use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Stdio},
    time::Duration,
};

/// Environment variable that lets the user pick the rendering context
/// kind used by the spawned sidecar. Accepted values: `software`
/// and `hardware`. macOS defaults to the hardware path and receives
/// IOSurface mach send rights over a side Mach channel.
use ely_domain::SitePermissionDecision;
use serde::Serialize;
use thiserror::Error;

#[path = "servo_live_wire.rs"]
mod wire;

use super::servo_sidecar_command::{
    SidecarCommandError, SidecarRenderingContext, default_sidecar_command,
    rendering_context_from_env,
};
use wire::{
    LiveFrameReport, LiveRequest, LiveResponse, LiveSurfaceHandle, log_frame_perf,
    log_iosurface_current, log_iosurface_handle,
};

#[cfg(target_os = "macos")]
use super::iosurface_mach::{IOSurfaceMachError, IOSurfaceMachReceiver};
#[cfg(target_os = "macos")]
use super::iosurface_metal::IOSurfaceCache;
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;

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
    iosurface_receiver: Option<IOSurfaceMachReceiver>,
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
        let iosurface_receiver = if rendering_context == SidecarRenderingContext::Hardware {
            let receiver = IOSurfaceMachReceiver::new()?;
            command.arg("--iosurface-mach-service").arg(receiver.service_name());
            Some(receiver)
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
            iosurface_receiver,
        })
    }

    pub fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
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
        })
    }

    pub fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        self.request(LiveRequest::Poll { tab_id })
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

        if let Some(perf) = response.perf.as_ref() {
            log_frame_perf(perf);
        }

        if let Some(handle) = response.surface_handle.as_ref() {
            log_iosurface_handle(handle);
            #[cfg(target_os = "macos")]
            self.import_iosurface_handle(handle)?;
        }
        if let Some(surface_id) = response.current_surface_id {
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
        if let Some(surface_id) = response.current_surface_id {
            let pixel_buffer = self.iosurface_cache.pixel_buffer_for(surface_id);
            if pixel_buffer.is_none() && !has_software_payload {
                return Err(ServoLiveError::IOSurfacePixelBufferMissing { surface_id });
            }
            frame.pixel_buffer = pixel_buffer;
        }

        Ok(Some(frame))
    }
}

impl Drop for ServoLiveClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(target_os = "macos")]
impl ServoLiveClient {
    /// Convert the sidecar's `surface_handle` into a `CVPixelBuffer`
    /// in the local cache. A later frame with a missing pixel buffer
    /// becomes a web-surface error instead of a blank ready frame.
    fn import_iosurface_handle(
        &mut self,
        handle: &LiveSurfaceHandle,
    ) -> Result<(), ServoLiveError> {
        let mach_port_name = match self.iosurface_receiver.as_mut() {
            Some(receiver) => {
                receiver.receive_port_for_surface(handle.surface_id, Duration::from_secs(1))?
            }
            None => handle.mach_port_name,
        };
        match self.iosurface_cache.import(mach_port_name, handle.surface_id) {
            Ok(()) => tracing::info!(
                target: "ely::servo::iosurface",
                surface_id = handle.surface_id,
                width = handle.width,
                height = handle.height,
                "imported IOSurface into CVPixelBuffer cache",
            ),
            Err(error) => {
                tracing::warn!(
                    target: "ely::servo::iosurface",
                    error = %error,
                    surface_id = handle.surface_id,
                    "IOSurface→CVPixelBuffer import failed",
                );
                return Err(ServoLiveError::IOSurfaceImportFailed {
                    surface_id: handle.surface_id,
                    mach_port_name,
                    message: error.to_string(),
                });
            }
        }
        Ok(())
    }
}

pub(crate) struct ServoLiveEnsureRequest {
    pub(crate) tab_id: String,
    pub(crate) profile_id: String,
    pub(crate) url: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) page_zoom_percent: u16,
    /// Display scale factor (1.0 standard, 2.0 Retina). Servo's
    /// WebView lays out CSS pixels = device pixels / hidpi factor;
    /// without this, a Retina viewport gets desktop-CSS-pixel layout
    /// and every visible element renders at half its expected size.
    pub(crate) device_pixel_ratio: f32,
    pub(crate) scroll_delta_x: i32,
    pub(crate) scroll_delta_y: i32,
    pub(crate) scroll_point_x: Option<u32>,
    pub(crate) scroll_point_y: Option<u32>,
    pub(crate) click_x: Option<u32>,
    pub(crate) click_y: Option<u32>,
    pub(crate) hover_x: Option<u32>,
    pub(crate) hover_y: Option<u32>,
    pub(crate) typed_text: Option<String>,
    pub(crate) site_permissions: Vec<ServoLiveSitePermission>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ServoLiveSitePermission {
    pub(crate) origin: String,
    pub(crate) feature: String,
    pub(crate) decision: String,
}

impl ServoLiveSitePermission {
    pub fn new(
        origin: impl Into<String>,
        feature: impl Into<String>,
        decision: SitePermissionDecision,
    ) -> Self {
        Self { origin: origin.into(), feature: feature.into(), decision: decision.as_str().into() }
    }
}

pub(crate) struct ServoLiveFrame {
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    device_pixel_ratio: f32,
    css_viewport_width: u32,
    css_viewport_height: u32,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    rgba_bytes: Vec<u8>,
    /// Hardware-path surface: the imported IOSurface published by the
    /// sidecar, wrapped as a CVPixelBuffer for GPUI's `surface(...)`.
    #[cfg(target_os = "macos")]
    pixel_buffer: Option<CVPixelBuffer>,
}

// SAFETY: CVPixelBuffer wraps CVPixelBufferRef, a CoreFoundation type
// Apple documents as safe to share across threads. The Rust core-video
// crate does not mark it Send, so the worker thread needs this opt-in
// to ship hardware frames back to the UI thread via mpsc::Sender.
#[cfg(target_os = "macos")]
#[expect(unsafe_code)]
unsafe impl Send for ServoLiveFrame {}

impl ServoLiveFrame {
    fn from_parts(report: LiveFrameReport, rgba_bytes: Vec<u8>) -> Self {
        let (css_viewport_width, css_viewport_height) = css_viewport_size_from_report(&report);
        Self {
            loaded_url: report.loaded_url,
            title: report.title,
            render_state: report.state,
            width: report.width,
            height: report.height,
            device_pixel_ratio: report.device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: report.non_white_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: report.content_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: report.sample_hash,
            rgba_bytes,
            #[cfg(target_os = "macos")]
            pixel_buffer: None,
        }
    }

    /// Returns the imported `CVPixelBuffer` matching the frame's
    /// current hardware surface.
    #[cfg(target_os = "macos")]
    #[must_use]
    pub fn pixel_buffer(&self) -> Option<&CVPixelBuffer> {
        self.pixel_buffer.as_ref()
    }

    #[must_use]
    pub fn loaded_url(&self) -> Option<&str> {
        self.loaded_url.as_deref()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn render_state(&self) -> &str {
        self.render_state.as_str()
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
    }

    #[must_use]
    pub fn css_viewport_width(&self) -> u32 {
        self.css_viewport_width
    }

    #[must_use]
    pub fn css_viewport_height(&self) -> u32 {
        self.css_viewport_height
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn non_white_pixel_count(&self) -> u64 {
        self.non_white_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn content_pixel_count(&self) -> u64 {
        self.content_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn sample_hash(&self) -> u64 {
        self.sample_hash
    }

    #[must_use]
    pub fn into_rgba_bytes(self) -> Vec<u8> {
        self.rgba_bytes
    }

    #[cfg(test)]
    pub(crate) fn for_test(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Self {
        Self {
            loaded_url: Some("https://example.com/".to_string()),
            title: Some("Example".to_string()),
            render_state: "complete".to_string(),
            width,
            height,
            device_pixel_ratio: 1.0,
            css_viewport_width: width,
            css_viewport_height: height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: 0,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: 0,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: 0,
            rgba_bytes,
            #[cfg(target_os = "macos")]
            pixel_buffer: None,
        }
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn for_test_with_pixel_buffer(
        width: u32,
        height: u32,
        pixel_buffer: CVPixelBuffer,
    ) -> Self {
        Self {
            loaded_url: Some("https://example.com/".to_string()),
            title: Some("Example".to_string()),
            render_state: "complete".to_string(),
            width,
            height,
            device_pixel_ratio: 1.0,
            css_viewport_width: width,
            css_viewport_height: height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: 0,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: 0,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: 0,
            rgba_bytes: Vec::new(),
            pixel_buffer: Some(pixel_buffer),
        }
    }
}

fn css_viewport_size_from_report(report: &LiveFrameReport) -> (u32, u32) {
    let dpr = if report.device_pixel_ratio.is_finite() && report.device_pixel_ratio > 0.0 {
        report.device_pixel_ratio
    } else {
        1.0
    };
    let fallback_width = ((report.width as f32) / dpr).round().max(1.0) as u32;
    let fallback_height = ((report.height as f32) / dpr).round().max(1.0) as u32;
    (
        if report.css_viewport_width > 0 { report.css_viewport_width } else { fallback_width },
        if report.css_viewport_height > 0 { report.css_viewport_height } else { fallback_height },
    )
}

#[derive(Debug, Error)]
pub(crate) enum ServoLiveError {
    #[error("servo sidecar binary is unavailable at {path}")]
    SidecarBinaryUnavailable { path: PathBuf },

    #[error("failed to run servo live sidecar: {0}")]
    Command(#[source] io::Error),

    #[error("servo live sidecar pipe is unavailable: {name}")]
    PipeUnavailable { name: &'static str },

    #[error("servo live sidecar exited")]
    SidecarExited,

    #[error("servo live sidecar failed: {message}")]
    SidecarFailed { message: String },

    #[error("failed to read servo live frame bytes: {0}")]
    FrameRead(#[source] io::Error),

    #[error(
        "servo live sidecar advertised {advertised} frame bytes which exceeds \
         the {width}x{height} pixel budget ({pixel_budget} bytes)"
    )]
    FrameBudgetExceeded { advertised: usize, pixel_budget: u64, width: u32, height: u32 },

    #[cfg(target_os = "macos")]
    #[error(
        "servo live IOSurface import failed for surface {surface_id:#x} \
         mach port 0x{mach_port_name:x}: {message}"
    )]
    IOSurfaceImportFailed { surface_id: u64, mach_port_name: u32, message: String },

    #[cfg(target_os = "macos")]
    #[error("servo live IOSurface {surface_id:#x} was selected before its pixel buffer import")]
    IOSurfacePixelBufferMissing { surface_id: u64 },

    #[cfg(target_os = "macos")]
    #[error(transparent)]
    IOSurfaceMach(#[from] IOSurfaceMachError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    SidecarCommand(#[from] SidecarCommandError),
}

impl ServoLiveError {
    pub(crate) fn is_sidecar_process_unusable(&self) -> bool {
        match self {
            Self::SidecarExited => true,
            Self::Command(error) | Self::FrameRead(error) => matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
            ),
            _ => false,
        }
    }
}
