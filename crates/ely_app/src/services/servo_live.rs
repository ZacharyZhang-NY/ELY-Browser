use std::{
    path::PathBuf,
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use std::collections::BTreeSet;

#[cfg(target_os = "macos")]
#[path = "servo_live_iosurface_importer.rs"]
mod iosurface_importer;
#[path = "servo_live_ipc.rs"]
mod ipc;
#[path = "servo_live_types.rs"]
mod types;
#[path = "servo_live_wire.rs"]
mod wire;

pub(crate) use types::{
    ServoLiveEnsureRequest, ServoLiveError, ServoLiveFrame, ServoLiveSitePermission,
};

use self::{
    ipc::{IpcReply, ServoLiveIpc, validate_frame_layout},
    wire::{LIVE_PROTOCOL_VERSION, LiveRequest, LiveSurfaceHandle},
};
use super::servo_sidecar_command::{
    SidecarRenderingContext, default_sidecar_command, rendering_context_from_env,
};

#[cfg(target_os = "macos")]
use super::iosurface_metal::IOSurfaceCache;
#[cfg(target_os = "macos")]
use iosurface_importer::{IOSurfaceImportResult, IOSurfaceImportWorker};

const SIDECAR_TIMEOUTS: SidecarTimeouts = SidecarTimeouts {
    request: Duration::from_secs(10),
    shutdown: Duration::from_secs(2),
    exit: Duration::from_secs(2),
};

#[derive(Clone, Copy)]
struct SidecarTimeouts {
    request: Duration,
    shutdown: Duration,
    exit: Duration,
}

pub(crate) struct ServoLiveClient {
    child: Child,
    ipc: ServoLiveIpc,
    timeouts: SidecarTimeouts,
    active: bool,
    #[cfg(target_os = "macos")]
    iosurface_cache: IOSurfaceCache,
    #[cfg(target_os = "macos")]
    iosurface_importer: Option<IOSurfaceImportWorker>,
    #[cfg(target_os = "macos")]
    pending_surface_ids: BTreeSet<u64>,
}

impl ServoLiveClient {
    pub fn new(profile_data_dir: PathBuf) -> Result<Self, ServoLiveError> {
        let rendering_context = rendering_context_from_env();
        let command_target = default_sidecar_command()?;
        if let Some(path) = command_target.missing_binary_path() {
            return Err(ServoLiveError::SidecarBinaryUnavailable { path: path.to_path_buf() });
        }

        let mut command = command_target.command();
        command.arg("live").arg("--profile-data-dir").arg(profile_data_dir);
        command.arg("--rendering-context").arg(rendering_context.cli_arg());
        #[cfg(target_os = "macos")]
        let iosurface_importer = if rendering_context == SidecarRenderingContext::Hardware {
            let receiver = super::iosurface_mach::IOSurfaceMachReceiver::new()?;
            command.arg("--iosurface-mach-service").arg(receiver.service_name());
            Some(
                IOSurfaceImportWorker::new(receiver)
                    .map_err(ServoLiveError::IOSurfaceImportWorker)?,
            )
        } else {
            None
        };
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ServoLiveError::Command)?;
        let mut client = Self::from_spawned_child(child, SIDECAR_TIMEOUTS)?;
        #[cfg(target_os = "macos")]
        {
            client.iosurface_importer = iosurface_importer;
        }
        Ok(client)
    }

    fn from_spawned_child(
        mut child: Child,
        timeouts: SidecarTimeouts,
    ) -> Result<Self, ServoLiveError> {
        let Some(stdin) = child.stdin.take() else {
            terminate_child(&mut child, timeouts.exit);
            return Err(ServoLiveError::PipeUnavailable { name: "stdin" });
        };
        let Some(stdout) = child.stdout.take() else {
            drop(stdin);
            terminate_child(&mut child, timeouts.exit);
            return Err(ServoLiveError::PipeUnavailable { name: "stdout" });
        };
        let mut client = Self {
            child,
            ipc: ServoLiveIpc::spawn(stdin, stdout),
            timeouts,
            active: true,
            #[cfg(target_os = "macos")]
            iosurface_cache: IOSurfaceCache::new(),
            #[cfg(target_os = "macos")]
            iosurface_importer: None,
            #[cfg(target_os = "macos")]
            pending_surface_ids: BTreeSet::new(),
        };
        if let Err(error) = client.handshake() {
            client.terminate();
            return Err(error);
        }
        Ok(client)
    }

    pub fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        #[cfg(target_os = "macos")]
        self.drain_iosurface_imports()?;
        validate_frame_layout(request.width, request.height)?;
        let ready_surface_ids = self.ready_surface_ids();
        let pending_surface_ids = self.pending_surface_ids();
        self.request(LiveRequest::Ensure {
            tab_id: request.tab_id,
            profile_id: request.profile_id,
            url: request.url,
            width: request.width,
            height: request.height,
            page_zoom_percent: request.page_zoom_percent,
            device_pixel_ratio: request.device_pixel_ratio,
            scroll_delta_x: request.scroll_delta_x,
            scroll_delta_y: request.scroll_delta_y,
            scroll_point_x: request.scroll_point_x,
            scroll_point_y: request.scroll_point_y,
            click_x: request.click_x,
            click_y: request.click_y,
            hover_x: request.hover_x,
            hover_y: request.hover_y,
            typed_text: request.typed_text,
            site_permissions: request.site_permissions,
            ready_surface_ids,
            pending_surface_ids,
        })
    }

    pub fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        #[cfg(target_os = "macos")]
        self.drain_iosurface_imports()?;
        self.request(LiveRequest::Poll {
            tab_id,
            ready_surface_ids: self.ready_surface_ids(),
            pending_surface_ids: self.pending_surface_ids(),
        })
    }

    pub fn close(&mut self, tab_id: String) -> Result<(), ServoLiveError> {
        self.request(LiveRequest::Close { tab_id }).map(|_| ())
    }

    fn handshake(&mut self) -> Result<(), ServoLiveError> {
        let reply = self.exchange(
            LiveRequest::Handshake { protocol_version: LIVE_PROTOCOL_VERSION },
            self.timeouts.request,
            "handshake",
        )?;
        if reply.frame.is_some()
            || reply.surface_handle.is_some()
            || reply.current_surface_id.is_some()
        {
            return Err(ServoLiveError::InvalidResponse {
                message: "handshake response contains a frame",
            });
        }
        match reply.error {
            Some(message) => Err(ServoLiveError::SidecarFailed { message }),
            None => Ok(()),
        }
    }

    fn request(&mut self, request: LiveRequest) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        let reply = self.exchange(request, self.timeouts.request, "request")?;
        if let Some(message) = reply.error {
            return Err(ServoLiveError::SidecarFailed { message });
        }
        #[cfg(target_os = "macos")]
        if let Some(handle) = reply.surface_handle {
            self.queue_iosurface_handle(handle)?;
            self.drain_iosurface_imports()?;
        }

        let mut frame = reply.frame;
        #[cfg(target_os = "macos")]
        if let (Some(frame), Some(surface_id)) = (frame.as_mut(), reply.current_surface_id) {
            let hardware_surface =
                self.iosurface_cache.hardware_surface_for(surface_id).map_err(|error| {
                    ServoLiveError::IOSurfaceBackingFailed {
                        surface_id,
                        message: error.to_string(),
                    }
                })?;
            if hardware_surface.is_none() && !frame.has_software_payload() {
                return Ok(None);
            }
            frame.set_hardware_surface(surface_id, hardware_surface);
        }
        Ok(frame)
    }

    fn exchange(
        &mut self,
        request: LiveRequest,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<IpcReply, ServoLiveError> {
        if !self.active {
            return Err(ServoLiveError::SidecarExited);
        }
        let reply = match self.ipc.exchange(request, timeout, operation) {
            Ok(reply) => reply,
            Err(error) => {
                self.terminate();
                return Err(error);
            }
        };
        if reply.protocol_version != Some(LIVE_PROTOCOL_VERSION) {
            let error = ServoLiveError::ProtocolVersionMismatch {
                expected: LIVE_PROTOCOL_VERSION,
                actual: reply.protocol_version,
            };
            self.terminate();
            return Err(error);
        }
        Ok(reply)
    }

    fn shutdown(&mut self) {
        if !self.active {
            return;
        }
        let acknowledged = self
            .ipc
            .exchange(LiveRequest::Shutdown, self.timeouts.shutdown, "shutdown")
            .ok()
            .is_some_and(|reply| {
                reply.protocol_version == Some(LIVE_PROTOCOL_VERSION)
                    && reply.error.is_none()
                    && reply.frame.is_none()
                    && reply.surface_handle.is_none()
                    && reply.current_surface_id.is_none()
            });
        if acknowledged && wait_for_exit(&mut self.child, self.timeouts.exit) {
            self.active = false;
            self.ipc.close_and_join(self.timeouts.exit);
            return;
        }
        self.terminate();
    }

    fn terminate(&mut self) {
        if self.active {
            terminate_child(&mut self.child, self.timeouts.exit);
            self.active = false;
        }
        self.ipc.close_and_join(self.timeouts.exit);
    }
}

#[cfg(not(target_os = "macos"))]
impl ServoLiveClient {
    fn ready_surface_ids(&self) -> Vec<u64> {
        Vec::new()
    }

    fn pending_surface_ids(&self) -> Vec<u64> {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
impl ServoLiveClient {
    fn ready_surface_ids(&self) -> Vec<u64> {
        self.iosurface_cache.surface_ids()
    }

    fn pending_surface_ids(&self) -> Vec<u64> {
        self.pending_surface_ids.iter().copied().collect()
    }

    fn queue_iosurface_handle(&mut self, handle: LiveSurfaceHandle) -> Result<(), ServoLiveError> {
        let Some(importer) = self.iosurface_importer.as_ref() else {
            return Err(ServoLiveError::IOSurfaceImportFailed {
                surface_id: handle.surface_id,
                mach_port_name: handle.mach_port_name,
                message: "IOSurface import worker is unavailable".to_string(),
            });
        };
        let surface_id = handle.surface_id;
        importer.submit(handle).map_err(|failure| ServoLiveError::IOSurfaceImportFailed {
            surface_id: failure.surface_id,
            mach_port_name: failure.mach_port_name,
            message: failure.message,
        })?;
        self.pending_surface_ids.insert(surface_id);
        Ok(())
    }

    fn drain_iosurface_imports(&mut self) -> Result<(), ServoLiveError> {
        let Some(importer) = self.iosurface_importer.as_ref() else {
            return Ok(());
        };
        apply_iosurface_import_results(
            &mut self.iosurface_cache,
            &mut self.pending_surface_ids,
            importer.drain(),
        )
    }
}

#[cfg(target_os = "macos")]
fn apply_iosurface_import_results(
    cache: &mut IOSurfaceCache,
    pending_surface_ids: &mut BTreeSet<u64>,
    results: Vec<IOSurfaceImportResult>,
) -> Result<(), ServoLiveError> {
    let mut first_failure = None;
    for result in results {
        let surface_id = match &result {
            IOSurfaceImportResult::Imported(imported) => imported.surface_id,
            IOSurfaceImportResult::Failed(failure) => failure.surface_id,
        };
        pending_surface_ids.remove(&surface_id);
        match result {
            IOSurfaceImportResult::Imported(imported) => {
                cache.insert_imported(imported.surface_id, imported.imported);
            }
            IOSurfaceImportResult::Failed(failure) if first_failure.is_none() => {
                first_failure = Some(ServoLiveError::IOSurfaceImportFailed {
                    surface_id: failure.surface_id,
                    mach_port_name: failure.mach_port_name,
                    message: failure.message,
                });
            }
            IOSurfaceImportResult::Failed(_) => {}
        }
    }
    first_failure.map_or(Ok(()), Err)
}

impl Drop for ServoLiveClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return false,
        }
        if started_at.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn terminate_child(child: &mut Child, timeout: Duration) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = wait_for_exit(child, timeout);
}

#[cfg(all(test, unix))]
mod tests {
    use std::{error::Error, process::Command};

    use super::*;

    const TEST_TIMEOUTS: SidecarTimeouts = SidecarTimeouts {
        request: Duration::from_millis(50),
        shutdown: Duration::from_millis(50),
        exit: Duration::from_millis(250),
    };

    #[test]
    fn startup_rejects_a_sidecar_without_the_current_protocol() -> Result<(), Box<dyn Error>> {
        let child = spawn_script(
            "IFS= read -r request; printf '%s\\n' '{\"error\":\"unknown request\",\"frame\":null}'",
        )?;

        let result = ServoLiveClient::from_spawned_child(child, TEST_TIMEOUTS);

        assert!(matches!(result, Err(ServoLiveError::ProtocolVersionMismatch { .. })));
        Ok(())
    }

    #[test]
    fn request_timeout_kills_and_reaps_a_hung_sidecar() -> Result<(), Box<dyn Error>> {
        let mut client = hung_sidecar_client()?;
        let started_at = Instant::now();

        let result = client.poll("tab-hung".to_string());

        assert!(matches!(result, Err(ServoLiveError::RequestTimedOut { .. })));
        assert!(started_at.elapsed() < Duration::from_secs(1));
        assert!(client.child.try_wait()?.is_some());
        Ok(())
    }

    #[test]
    fn shutdown_timeout_kills_and_reaps_a_hung_sidecar() -> Result<(), Box<dyn Error>> {
        let mut client = hung_sidecar_client()?;
        let started_at = Instant::now();

        client.shutdown();

        assert!(started_at.elapsed() < Duration::from_secs(1));
        assert!(client.child.try_wait()?.is_some());
        Ok(())
    }

    fn hung_sidecar_client() -> Result<ServoLiveClient, ServoLiveError> {
        let script = format!(
            "IFS= read -r request; printf '%s\\n' '{{\"protocol_version\":{LIVE_PROTOCOL_VERSION},\"error\":null,\"frame\":null}}'; while IFS= read -r request; do :; done"
        );
        let child = spawn_script(&script).map_err(ServoLiveError::Command)?;
        ServoLiveClient::from_spawned_child(child, TEST_TIMEOUTS)
    }

    fn spawn_script(script: &str) -> Result<Child, std::io::Error> {
        Command::new("sh")
            .arg("-c")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "servo_live_iosurface_tests.rs"]
mod iosurface_tests;
