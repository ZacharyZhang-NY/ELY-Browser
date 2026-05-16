//! Wire types for the sidecar live loop. Split out of `live.rs` to
//! keep the hot loop and protocol surface in separate files.

use std::io;

use ely_servo_host::{
    IOSurfaceHandle, RenderedFrame, ServoHostError, WebViewSnapshot, WebViewState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use super::iosurface_mach::IOSurfaceMachError;
use super::perf::FramePerfSummary;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum LiveRequest {
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        /// Display scale factor reported by the host's window
        /// (1.0 standard, 2.0 Retina). The sidecar plumbs this into
        /// Servo's `WebView::set_hidpi_scale_factor` so CSS layout
        /// happens at logical-pixel dimensions instead of physical.
        /// Defaults to 1.0 for backward compatibility if a client
        /// (e.g. the live perf bench) omits the field.
        #[serde(default = "default_device_pixel_ratio")]
        device_pixel_ratio: f32,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        scroll_point_x: Option<u32>,
        scroll_point_y: Option<u32>,
        click_x: Option<u32>,
        click_y: Option<u32>,
        #[serde(default)]
        hover_x: Option<u32>,
        #[serde(default)]
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permissions: Vec<LiveSitePermission>,
        #[serde(default)]
        ready_surface_ids: Vec<u64>,
    },
    Poll {
        tab_id: String,
        #[serde(default)]
        ready_surface_ids: Vec<u64>,
    },
    Close {
        tab_id: String,
    },
}

fn default_device_pixel_ratio() -> f32 {
    1.0
}

#[derive(Deserialize)]
pub(super) struct LiveSitePermission {
    pub origin: String,
    pub feature: String,
    pub decision: String,
}

/// Partial stage timings captured inside `poll_frame` before the
/// write phase. Combined with the write-stage duration measured by
/// `write_outcome` to form a full set of frame timings.
#[derive(Clone, Copy, Debug)]
pub(super) struct PartialFrameTimings {
    pub paint_ns: u64,
    pub encode_ns: u64,
}

/// A response plus an optional software RGBA payload and partial stage
/// timings. Software frames carry `RenderedFrame` so the write step
/// can stream its existing rgba slice straight onto the pipe; hardware
/// surface frames carry only a `LiveFrameReport`.
pub(super) struct LiveOutcome {
    pub response: LiveResponse,
    pub frame: Option<RenderedFrame>,
    pub partial_timings: Option<PartialFrameTimings>,
}

impl LiveOutcome {
    pub fn empty() -> Self {
        Self { response: LiveResponse::empty(), frame: None, partial_timings: None }
    }

    pub fn error(message: String) -> Self {
        Self { response: LiveResponse::error(message), frame: None, partial_timings: None }
    }

    pub fn from_frame(
        report: LiveFrameReport,
        frame: RenderedFrame,
        partial_timings: PartialFrameTimings,
    ) -> Self {
        Self {
            response: LiveResponse::frame(report),
            frame: Some(frame),
            partial_timings: Some(partial_timings),
        }
    }

    #[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
    pub fn from_report(report: LiveFrameReport, partial_timings: PartialFrameTimings) -> Self {
        Self {
            response: LiveResponse::frame(report),
            frame: None,
            partial_timings: Some(partial_timings),
        }
    }
}

#[derive(Serialize)]
pub(super) struct LiveResponse {
    pub error: Option<String>,
    pub frame: Option<LiveFrameReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perf: Option<FramePerfSummary>,
    /// Populated on the first frame the sidecar emits for a given
    /// surface — initial paint, after a resize, or whenever surfman
    /// rotates its swap chain to a surface we haven't seen yet. The
    /// receiver imports the IOSurface (via
    /// `IOSurfaceLookupFromMachPort`) once per `surface_id` and caches
    /// the resulting Metal texture. Always `None` on the software
    /// path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_handle: Option<IOSurfaceHandle>,
    /// Populated on every hardware paint frame. Tells the receiver
    /// which previously-imported IOSurface to sample THIS frame. The
    /// surfman attached swap chain rotates between front/back
    /// surfaces, so this id alternates between the values the receiver
    /// has already imported. Always `None` on the software path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_surface_id: Option<u64>,
}

impl LiveResponse {
    fn empty() -> Self {
        Self {
            error: None,
            frame: None,
            perf: None,
            surface_handle: None,
            current_surface_id: None,
        }
    }

    fn frame(frame: LiveFrameReport) -> Self {
        Self {
            error: None,
            frame: Some(frame),
            perf: None,
            surface_handle: None,
            current_surface_id: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            error: Some(message),
            frame: None,
            perf: None,
            surface_handle: None,
            current_surface_id: None,
        }
    }
}

#[derive(Serialize)]
pub(super) struct LiveFrameReport {
    pub loaded_url: Option<String>,
    pub title: Option<String>,
    pub state: &'static str,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub css_viewport_width: u32,
    pub css_viewport_height: u32,
    pub rgba_byte_count: usize,
    pub non_white_pixel_count: u64,
    pub content_pixel_count: u64,
    pub sample_hash: u64,
}

impl LiveFrameReport {
    pub fn new(snapshot: &WebViewSnapshot, frame: &RenderedFrame, device_pixel_ratio: f32) -> Self {
        let (css_viewport_width, css_viewport_height) =
            css_viewport_size(frame.width(), frame.height(), device_pixel_ratio);
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            rgba_byte_count: frame.rgba_bytes().len(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
        }
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub fn from_surface(
        snapshot: &WebViewSnapshot,
        width: u32,
        height: u32,
        device_pixel_ratio: f32,
    ) -> Self {
        let (css_viewport_width, css_viewport_height) =
            css_viewport_size(width, height, device_pixel_ratio);
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width,
            height,
            device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            rgba_byte_count: 0,
            non_white_pixel_count: 0,
            content_pixel_count: 0,
            sample_hash: 0,
        }
    }
}

fn css_viewport_size(width: u32, height: u32, device_pixel_ratio: f32) -> (u32, u32) {
    let dpr = if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
        device_pixel_ratio
    } else {
        1.0
    };
    (
        ((width as f32) / dpr).round().max(1.0) as u32,
        ((height as f32) / dpr).round().max(1.0) as u32,
    )
}

fn state_label(state: &WebViewState) -> &'static str {
    match state {
        WebViewState::Created => "created",
        WebViewState::Loading => "loading",
        WebViewState::Complete => "complete",
        WebViewState::Sleeping => "sleeping",
        WebViewState::Crashed => "crashed",
    }
}

#[derive(Debug, Error)]
pub(super) enum LiveSidecarError {
    #[error("live session is unavailable after creation")]
    SessionUnavailable,

    #[error("scroll input requires both scroll_point_x and scroll_point_y")]
    IncompleteScrollPoint,

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),

    #[error(transparent)]
    Host(#[from] ServoHostError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[error(transparent)]
    IOSurfaceMach(#[from] IOSurfaceMachError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_request_deserializes_from_wire() -> Result<(), serde_json::Error> {
        let request =
            serde_json::from_str::<LiveRequest>(r#"{"type":"close","tab_id":"tab-live-close"}"#)?;

        assert!(matches!(
            request,
            LiveRequest::Close { tab_id } if tab_id == "tab-live-close"
        ));
        Ok(())
    }
}
