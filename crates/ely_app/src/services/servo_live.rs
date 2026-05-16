use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Stdio},
};

#[cfg(target_os = "macos")]
#[path = "servo_live_iosurface_importer.rs"]
mod iosurface_importer;
#[path = "servo_live_types.rs"]
mod types;
/// Environment variable that lets the user pick the rendering context
/// kind used by the spawned sidecar. Accepted values: `software`
/// and `hardware`. macOS defaults to the hardware path and receives
/// IOSurface mach send rights over a side Mach channel.
#[path = "servo_live_wire.rs"]
mod wire;

pub(crate) use types::{
    ServoLiveEnsureRequest, ServoLiveError, ServoLiveFrame, ServoLiveSitePermission,
};

use super::servo_sidecar_command::{
    SidecarRenderingContext, default_sidecar_command, rendering_context_from_env,
};
use wire::{
    LiveRequest, LiveResponse, LiveSurfaceHandle, log_frame_perf, log_iosurface_current,
    log_iosurface_handle,
};

#[cfg(target_os = "macos")]
use super::iosurface_metal::IOSurfaceCache;
#[cfg(target_os = "macos")]
use iosurface_importer::{IOSurfaceImportResult, IOSurfaceImportWorker};

pub(crate) struct ServoLiveClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Cache of imported `CVPixelBuffer`s keyed by surface_id. Built
    /// lazily on the first `surface_handle` the sidecar publishes —
    /// software-path tabs never trigger construction.
    #[cfg(target_os = "macos")]
    iosurface_cache: IOSurfaceCache,
    #[cfg(target_os = "macos")]
    iosurface_importer: Option<IOSurfaceImportWorker>,
}

impl ServoLiveClient {
    pub fn new(profile_data_dir: PathBuf) -> Result<Self, ServoLiveError> {
        let command_target = default_sidecar_command()?;
        if let Some(path) = command_target.missing_binary_path() {
            return Err(ServoLiveError::SidecarBinaryUnavailable { path: path.to_path_buf() });
        }

        let rendering_context = rendering_context_from_env();
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
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ServoLiveError::Command)?;
        let stdin = child.stdin.take().ok_or(ServoLiveError::PipeUnavailable { name: "stdin" })?;
        let stdout =
            child.stdout.take().ok_or(ServoLiveError::PipeUnavailable { name: "stdout" })?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            #[cfg(target_os = "macos")]
            iosurface_cache: IOSurfaceCache::new(),
            #[cfg(target_os = "macos")]
            iosurface_importer,
        })
    }

    pub fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        #[cfg(target_os = "macos")]
        self.drain_iosurface_imports()?;
        let ready_surface_ids = self.ready_surface_ids();
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
        })
    }

    pub fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        #[cfg(target_os = "macos")]
        self.drain_iosurface_imports()?;
        let ready_surface_ids = self.ready_surface_ids();
        self.request(LiveRequest::Poll { tab_id, ready_surface_ids })
    }

    pub fn close(&mut self, tab_id: String) -> Result<(), ServoLiveError> {
        self.request(LiveRequest::Close { tab_id }).map(|_| ())
    }

    fn request(&mut self, request: LiveRequest) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        serde_json::to_writer(&mut self.stdin, &request)?;
        self.stdin.write_all(b"\n").map_err(ServoLiveError::Command)?;
        self.stdin.flush().map_err(ServoLiveError::Command)?;

        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line).map_err(ServoLiveError::Command)?;
        if bytes == 0 {
            return Err(ServoLiveError::SidecarExited);
        }

        let response: LiveResponse = serde_json::from_str(&line)?;
        if let Some(error) = response.error {
            return Err(ServoLiveError::SidecarFailed { message: error });
        }
        let surface_handle = response.surface_handle;
        let current_surface_id = response.current_surface_id;

        if let Some(perf) = response.perf.as_ref() {
            log_frame_perf(perf);
        }

        if let Some(handle) = surface_handle.as_ref() {
            log_iosurface_handle(handle);
        }
        if let Some(surface_id) = current_surface_id {
            log_iosurface_current(surface_id);
        }

        let Some(report) = response.frame else {
            return Ok(None);
        };

        // Sanity bound the byte count advertised by the sidecar
        // header so a buggy or hostile sidecar can't park us on
        // `read_exact` for an arbitrarily-sized buffer. The honest
        // upper limit is `width * height * 4` (RGBA8); `0` is the
        // explicit "hardware path active, sample the IOSurface"
        // signal; any other byte count is a protocol violation.
        let pixel_byte_count =
            (report.width as u64).saturating_mul(report.height as u64).saturating_mul(4);
        let advertised = report.rgba_byte_count as u64;
        if advertised != 0 && advertised != pixel_byte_count {
            return Err(ServoLiveError::FrameBudgetExceeded {
                advertised: report.rgba_byte_count,
                pixel_budget: pixel_byte_count,
                width: report.width,
                height: report.height,
            });
        }

        // Raw frame bytes follow the JSON header on the same pipe for
        // software frames. `read_exact` drains BufReader's buffer first
        // (the line read never crosses the `\n` boundary) and then
        // pulls the rest straight from the child's stdout.
        let mut rgba_bytes = vec![0u8; report.rgba_byte_count];
        if report.rgba_byte_count > 0 {
            self.stdout.read_exact(&mut rgba_bytes).map_err(ServoLiveError::FrameRead)?;
        }

        let has_software_payload = report.rgba_byte_count > 0;
        let mut frame = ServoLiveFrame::from_parts(report, rgba_bytes);

        #[cfg(target_os = "macos")]
        if let Some(handle) = surface_handle.as_ref() {
            self.queue_iosurface_handle(*handle)?;
            self.drain_iosurface_imports()?;
        }

        #[cfg(target_os = "macos")]
        if let Some(surface_id) = current_surface_id {
            let pixel_buffer = self.iosurface_cache.pixel_buffer_for(surface_id);
            if pixel_buffer.is_none() && !has_software_payload {
                return Ok(None);
            }
            frame.set_pixel_buffer(pixel_buffer);
        }

        Ok(Some(frame))
    }
}

#[cfg(not(target_os = "macos"))]
impl ServoLiveClient {
    fn ready_surface_ids(&self) -> Vec<u64> {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
impl ServoLiveClient {
    fn ready_surface_ids(&self) -> Vec<u64> {
        self.iosurface_cache.surface_ids()
    }

    fn queue_iosurface_handle(&mut self, handle: LiveSurfaceHandle) -> Result<(), ServoLiveError> {
        let Some(importer) = self.iosurface_importer.as_ref() else {
            return Err(ServoLiveError::IOSurfaceImportFailed {
                surface_id: handle.surface_id,
                mach_port_name: handle.mach_port_name,
                message: "IOSurface import worker is unavailable".to_string(),
            });
        };
        importer.submit(handle).map_err(|failure| ServoLiveError::IOSurfaceImportFailed {
            surface_id: failure.surface_id,
            mach_port_name: failure.mach_port_name,
            message: failure.message,
        })
    }

    fn drain_iosurface_imports(&mut self) -> Result<(), ServoLiveError> {
        let Some(importer) = self.iosurface_importer.as_ref() else {
            return Ok(());
        };
        for result in importer.drain() {
            match result {
                IOSurfaceImportResult::Imported(imported) => {
                    self.iosurface_cache
                        .insert_pixel_buffer(imported.surface_id, imported.pixel_buffer);
                    tracing::info!(
                        target: "ely::servo::iosurface",
                        surface_id = imported.surface_id,
                        width = imported.width,
                        height = imported.height,
                        "imported IOSurface into CVPixelBuffer cache",
                    );
                }
                IOSurfaceImportResult::Failed(failure) => {
                    tracing::warn!(
                        target: "ely::servo::iosurface",
                        surface_id = failure.surface_id,
                        width = failure.width,
                        height = failure.height,
                        mach_port_name = failure.mach_port_name,
                        message = %failure.message,
                        "IOSurface import worker failed",
                    );
                    return Err(ServoLiveError::IOSurfaceImportFailed {
                        surface_id: failure.surface_id,
                        mach_port_name: failure.mach_port_name,
                        message: failure.message,
                    });
                }
            }
        }
        Ok(())
    }
}

impl Drop for ServoLiveClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
