use std::{
    env,
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Stdio},
};

/// Environment variable that lets the user pick the rendering context
/// kind used by the spawned sidecar. Accepted values: `software`
/// (default — bit-identical to pre-flag builds) and `hardware` (real
/// GPU adapter via the vendored `HardwareOffscreenContext`; requires
/// the sidecar binary to be compiled with the `hardware-render`
/// feature). Anything else is silently dropped and the sidecar
/// defaults to software so a typo'd value never blocks the browser
/// from starting; the sidecar's own arg parser still errors loudly on
/// an unrecognised value when set explicitly via the flag.
const RENDERING_CONTEXT_ENV: &str = "ELY_SERVO_RENDERING_CONTEXT";

use ely_domain::SitePermissionDecision;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::servo_sidecar_command::{SidecarCommandError, default_sidecar_command};

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
}

impl ServoLiveClient {
    pub fn new(profile_data_dir: PathBuf) -> Result<Self, ServoLiveError> {
        let command_target = default_sidecar_command()?;
        if let Some(path) = command_target.missing_binary_path() {
            return Err(ServoLiveError::SidecarBinaryUnavailable { path: path.to_path_buf() });
        }

        let mut command = command_target.command();
        command.arg("live").arg("--profile-data-dir").arg(profile_data_dir);
        if let Some(rendering_context) = rendering_context_from_env() {
            command.arg("--rendering-context").arg(rendering_context);
        }
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
            scroll_delta_x: request.scroll_delta_x,
            scroll_delta_y: request.scroll_delta_y,
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
            self.import_iosurface_handle(handle);
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
        // upper limit is `width * height * 4` (RGBA8); anything
        // larger is a protocol violation and we fail the request
        // instead of allocating against it.
        let pixel_byte_count = (report.width as u64)
            .saturating_mul(report.height as u64)
            .saturating_mul(4);
        if (report.rgba_byte_count as u64) != pixel_byte_count {
            return Err(ServoLiveError::FrameBudgetExceeded {
                advertised: report.rgba_byte_count,
                pixel_budget: pixel_byte_count,
                width: report.width,
                height: report.height,
            });
        }

        // Raw frame bytes follow the JSON header on the same pipe.
        // `read_exact` drains BufReader's buffer first (the line read
        // never crosses the `\n` boundary) and then pulls the rest
        // straight from the child's stdout — no fs::read, no temp file.
        let mut rgba_bytes = vec![0u8; report.rgba_byte_count];
        self.stdout
            .read_exact(&mut rgba_bytes)
            .map_err(ServoLiveError::FrameRead)?;

        let mut frame = ServoLiveFrame::from_parts(report, rgba_bytes);

        #[cfg(target_os = "macos")]
        if let Some(surface_id) = response.current_surface_id {
            frame.pixel_buffer = self.iosurface_cache.pixel_buffer_for(surface_id);
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
    /// in the local cache. Failures are logged but don't error the
    /// request — the renderer falls back to the existing software
    /// `Arc<RenderImage>` path when no pixel buffer is available, so
    /// the user always sees a frame.
    fn import_iosurface_handle(&mut self, handle: &LiveSurfaceHandle) {
        match self.iosurface_cache.import(handle.mach_port_name, handle.surface_id) {
            Ok(()) => tracing::info!(
                target: "ely::servo::iosurface",
                surface_id = handle.surface_id,
                width = handle.width,
                height = handle.height,
                "imported IOSurface into CVPixelBuffer cache",
            ),
            Err(error) => tracing::warn!(
                target: "ely::servo::iosurface",
                error = %error,
                surface_id = handle.surface_id,
                "IOSurface→CVPixelBuffer import failed; subsequent samples will miss",
            ),
        }
    }
}

pub(crate) struct ServoLiveEnsureRequest {
    pub(crate) tab_id: String,
    pub(crate) profile_id: String,
    pub(crate) url: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) page_zoom_percent: u16,
    pub(crate) scroll_delta_x: i32,
    pub(crate) scroll_delta_y: i32,
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
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    rgba_bytes: Vec<u8>,
    /// Hardware-path companion: when present, the renderer can hand
    /// the underlying IOSurface straight to GPUI's Metal pipeline via
    /// `gpui::surface(...)` and skip the RGBA upload entirely. Always
    /// `None` on the software path and on non-macOS hosts.
    #[cfg(target_os = "macos")]
    pixel_buffer: Option<CVPixelBuffer>,
}

impl ServoLiveFrame {
    fn from_parts(report: LiveFrameReport, rgba_bytes: Vec<u8>) -> Self {
        Self {
            loaded_url: report.loaded_url,
            title: report.title,
            render_state: report.state,
            width: report.width,
            height: report.height,
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
    /// current hardware surface, if any. The renderer hands this to
    /// `gpui::surface(...)` to skip the RGBA→texture upload path.
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

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    SidecarCommand(#[from] SidecarCommandError),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LiveRequest {
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        click_x: Option<u32>,
        click_y: Option<u32>,
        hover_x: Option<u32>,
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permissions: Vec<ServoLiveSitePermission>,
    },
    Poll {
        tab_id: String,
    },
}

#[derive(Deserialize)]
struct LiveResponse {
    error: Option<String>,
    frame: Option<LiveFrameReport>,
    #[serde(default)]
    perf: Option<LiveFramePerfSummary>,
    /// Hardware path only: present on the first frame after a new
    /// IOSurface is bound (initial paint, resize, surfman swap chain
    /// rotation). T10.4 will turn this into an imported Metal texture;
    /// for now we log it on the `ely::servo::iosurface` target so the
    /// pipeline is observable end-to-end without yet wiring it into
    /// the renderer.
    #[serde(default)]
    surface_handle: Option<LiveSurfaceHandle>,
    /// Hardware path only: which previously-imported IOSurface to
    /// sample this frame. surfman's attached swap chain rotates the
    /// bound surface, so this id alternates between the values the
    /// receiver has already imported via `surface_handle`.
    #[serde(default)]
    current_surface_id: Option<u64>,
}

/// Wire mirror of `ely_servo_host::IOSurfaceHandle`. Duplicated rather
/// than imported because `ely_app` only talks to the sidecar via
/// stdin/stdout JSON — it has no crate dependency on `ely_servo_host`
/// and adding one just to share a four-field struct would pull the
/// Servo dep tree into the renderer process.
#[derive(Clone, Copy, Debug, Deserialize)]
struct LiveSurfaceHandle {
    mach_port_name: u32,
    surface_id: u64,
    width: u32,
    height: u32,
}

/// Aggregated frame-stage timings rolled up every N frames by the
/// sidecar. We accept anything matching the wire shape and let the
/// `tracing` event echo the percentiles verbatim — the sidecar is
/// the source of truth for histogram boundaries.
#[derive(Deserialize)]
struct LiveFramePerfSummary {
    window: u32,
    context: String,
    paint_p50_us: u64,
    paint_p95_us: u64,
    paint_p99_us: u64,
    encode_p50_us: u64,
    encode_p95_us: u64,
    encode_p99_us: u64,
    write_p50_us: u64,
    write_p95_us: u64,
    write_p99_us: u64,
    total_p50_us: u64,
    total_p95_us: u64,
    total_p99_us: u64,
}

/// Per-frame tag that tells the renderer which already-imported
/// `MTLTexture` to sample. Emitted at `trace` instead of `info` because
/// it fires every frame on the hardware path; the import event above
/// is the rare `info` and this trace is the steady-state breadcrumb.
fn log_iosurface_current(surface_id: u64) {
    tracing::trace!(
        target: "ely::servo::iosurface",
        surface_id,
        "iosurface_current",
    );
}

/// Emit one structured `tracing` event per IOSurface handover, on a
/// dedicated target so `RUST_LOG=ely::servo::iosurface=info` lights up
/// the cross-process surface pipeline without pulling in everything
/// else. The renderer (T10.4) will turn the same handle into an
/// imported Metal texture; today the event is the observable contract
/// that T10.3 plumbing is alive.
fn log_iosurface_handle(handle: &LiveSurfaceHandle) {
    tracing::info!(
        target: "ely::servo::iosurface",
        mach_port_name = handle.mach_port_name,
        surface_id = handle.surface_id,
        width = handle.width,
        height = handle.height,
        "iosurface_handle",
    );
}

/// Emit one structured `tracing` event per perf summary, on a
/// dedicated target so `RUST_LOG=ely::servo::perf=info` flips the
/// stream on without dragging the rest of the app along. Filtering
/// happens upstream in the subscriber — this call is a single
/// pointer + integer push.
fn log_frame_perf(summary: &LiveFramePerfSummary) {
    tracing::info!(
        target: "ely::servo::perf",
        window = summary.window,
        context = %summary.context,
        paint_p50_us = summary.paint_p50_us,
        paint_p95_us = summary.paint_p95_us,
        paint_p99_us = summary.paint_p99_us,
        encode_p50_us = summary.encode_p50_us,
        encode_p95_us = summary.encode_p95_us,
        encode_p99_us = summary.encode_p99_us,
        write_p50_us = summary.write_p50_us,
        write_p95_us = summary.write_p95_us,
        write_p99_us = summary.write_p99_us,
        total_p50_us = summary.total_p50_us,
        total_p95_us = summary.total_p95_us,
        total_p99_us = summary.total_p99_us,
        "frame_perf",
    );
}

#[derive(Deserialize)]
struct LiveFrameReport {
    loaded_url: Option<String>,
    title: Option<String>,
    state: String,
    width: u32,
    height: u32,
    rgba_byte_count: usize,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
}

/// Map `ELY_SERVO_RENDERING_CONTEXT` to a CLI argument value if it's
/// one we recognise. Unset variable returns `None` (sidecar uses its
/// own default of software); unknown value also returns `None`
/// rather than `Some("garbage")` so a stale env var doesn't fail the
/// sidecar startup. The sidecar's own arg parser is the source of
/// truth for what values are valid — we just gate which ones we
/// forward.
fn rendering_context_from_env() -> Option<&'static str> {
    match env::var(RENDERING_CONTEXT_ENV).ok()?.to_lowercase().as_str() {
        "software" => Some("software"),
        "hardware" => Some("hardware"),
        _ => None,
    }
}
