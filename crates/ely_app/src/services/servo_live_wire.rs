use serde::{Deserialize, Serialize};

use super::ServoLiveSitePermission;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum LiveRequest {
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        device_pixel_ratio: f32,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        scroll_point_x: Option<u32>,
        scroll_point_y: Option<u32>,
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
pub(super) struct LiveResponse {
    pub(super) error: Option<String>,
    pub(super) frame: Option<LiveFrameReport>,
    #[serde(default)]
    pub(super) perf: Option<LiveFramePerfSummary>,
    /// Hardware path only: present on the first frame after a new
    /// IOSurface is bound (initial paint, resize, surfman swap chain
    /// rotation). T10.4 will turn this into an imported Metal texture;
    /// for now we log it on the `ely::servo::iosurface` target so the
    /// pipeline is observable end-to-end without yet wiring it into
    /// the renderer.
    #[serde(default)]
    pub(super) surface_handle: Option<LiveSurfaceHandle>,
    /// Hardware path only: which previously-imported IOSurface to
    /// sample this frame. surfman's attached swap chain rotates the
    /// bound surface, so this id alternates between the values the
    /// receiver has already imported via `surface_handle`.
    #[serde(default)]
    pub(super) current_surface_id: Option<u64>,
}

/// Wire mirror of `ely_servo_host::IOSurfaceHandle`. Duplicated rather
/// than imported because `ely_app` only talks to the sidecar via
/// stdin/stdout JSON — it has no crate dependency on `ely_servo_host`
/// and adding one just to share a four-field struct would pull the
/// Servo dep tree into the renderer process.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) struct LiveSurfaceHandle {
    pub(super) mach_port_name: u32,
    pub(super) surface_id: u64,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Aggregated frame-stage timings rolled up every N frames by the
/// sidecar. We accept anything matching the wire shape and let the
/// `tracing` event echo the percentiles verbatim — the sidecar is
/// the source of truth for histogram boundaries.
#[derive(Deserialize)]
pub(super) struct LiveFramePerfSummary {
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

#[derive(Deserialize)]
pub(super) struct LiveFrameReport {
    pub(super) loaded_url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: String,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba_byte_count: usize,
    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) sample_hash: u64,
}

/// Per-frame tag that tells the renderer which already-imported
/// `MTLTexture` to sample. Emitted at `trace` instead of `info` because
/// it fires every frame on the hardware path; the import event above
/// is the rare `info` and this trace is the steady-state breadcrumb.
pub(super) fn log_iosurface_current(surface_id: u64) {
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
pub(super) fn log_iosurface_handle(handle: &LiveSurfaceHandle) {
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
pub(super) fn log_frame_perf(summary: &LiveFramePerfSummary) {
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
