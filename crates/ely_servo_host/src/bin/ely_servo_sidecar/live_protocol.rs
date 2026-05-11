//! Wire types for the sidecar live loop. Split out of `live.rs` to
//! keep the hot loop and protocol surface in separate files.

use std::io;

use ely_servo_host::{IOSurfaceHandle, RenderedFrame, ServoHostError, WebViewSnapshot, WebViewState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        click_x: Option<u32>,
        click_y: Option<u32>,
        #[serde(default)]
        hover_x: Option<u32>,
        #[serde(default)]
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permissions: Vec<LiveSitePermission>,
    },
    Poll {
        tab_id: String,
    },
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

/// A handle plus an optional rendered frame and partial stage
/// timings. We hold the `RenderedFrame` so the write step can stream
/// its existing rgba slice straight onto the pipe — no extra
/// `to_vec()`.
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
    pub rgba_byte_count: usize,
    pub non_white_pixel_count: u64,
    pub content_pixel_count: u64,
    pub sample_hash: u64,
}

impl LiveFrameReport {
    pub fn new(snapshot: &WebViewSnapshot, frame: &RenderedFrame) -> Self {
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            rgba_byte_count: frame.rgba_bytes().len(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
        }
    }
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

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),

    #[error(transparent)]
    Host(#[from] ServoHostError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
