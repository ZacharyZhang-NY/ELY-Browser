use std::{io, path::PathBuf};

use ely_servo_host::{
    ConsumedPermission, IOSurfaceHandle, RenderedFrame, ServoHostError, WebViewSnapshot,
    WebViewState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use super::iosurface_mach::IOSurfaceMachError;

pub(super) const LIVE_PROTOCOL_VERSION: u32 = 3;
pub(super) const MAX_FRAME_DIMENSION: u32 = 16_384;
pub(super) const MAX_FRAME_BYTE_COUNT: usize = 256 * 1024 * 1024;
pub(super) const MAX_RESPONSE_HEADER_BYTES: usize = 256 * 1024;
pub(super) const MAX_TITLE_BYTES: usize = ely_servo_host::MAX_PAGE_TITLE_BYTES;
pub(super) const MAX_LOADED_URL_BYTES: usize = 32 * 1024;
pub(super) const MAX_ERROR_BYTES: usize = 32 * 1024;
pub(super) const MAX_PERMISSION_CONSUMPTIONS_PER_RESPONSE: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum LiveRequest {
    Handshake {
        protocol_version: u32,
    },
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        #[serde(default = "default_zoom_percent")]
        page_zoom_percent: u16,
        #[serde(default = "default_device_pixel_ratio")]
        device_pixel_ratio: f32,
        #[serde(default)]
        scroll_delta_x: i32,
        #[serde(default)]
        scroll_delta_y: i32,
        #[serde(default)]
        scroll_point_x: Option<u32>,
        #[serde(default)]
        scroll_point_y: Option<u32>,
        #[serde(default)]
        click_x: Option<u32>,
        #[serde(default)]
        click_y: Option<u32>,
        #[serde(default)]
        hover_x: Option<u32>,
        #[serde(default)]
        hover_y: Option<u32>,
        #[serde(default)]
        typed_text: Option<String>,
        site_permission_generation: u64,
        #[serde(default)]
        site_permissions: Vec<LiveSitePermission>,
        #[serde(default)]
        ready_surface_ids: Vec<u64>,
        #[serde(default)]
        pending_surface_ids: Vec<u64>,
    },
    Poll {
        tab_id: String,
        #[serde(default)]
        ready_surface_ids: Vec<u64>,
        #[serde(default)]
        pending_surface_ids: Vec<u64>,
    },
    Close {
        tab_id: String,
    },
    Shutdown,
}

const fn default_zoom_percent() -> u16 {
    ely_domain::DEFAULT_ZOOM_PERCENT
}

const fn default_device_pixel_ratio() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveSitePermission {
    pub(super) origin: String,
    pub(super) feature: String,
    pub(super) state: String,
    pub(super) revision: u64,
}

pub(super) struct LiveOutcome {
    pub(super) response: LiveResponse,
    pub(super) frame: Option<RenderedFrame>,
}

impl LiveOutcome {
    pub(super) fn empty() -> Self {
        Self { response: LiveResponse::empty(), frame: None }
    }

    pub(super) fn error(message: String) -> Self {
        Self { response: LiveResponse::error(message), frame: None }
    }

    pub(super) fn frame(report: LiveFrameReport, frame: RenderedFrame) -> Self {
        Self { response: LiveResponse::frame(report), frame: Some(frame) }
    }

    pub(super) fn with_permission_consumptions(
        mut self,
        consumptions: Vec<ConsumedPermission>,
    ) -> Self {
        self.response.permission_consumptions =
            consumptions.into_iter().map(LivePermissionConsumption::from).collect();
        self
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) fn surface(report: LiveFrameReport) -> Self {
        Self { response: LiveResponse::frame(report), frame: None }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct LiveResponse {
    pub(super) protocol_version: u32,
    pub(super) error: Option<String>,
    pub(super) frame: Option<LiveFrameReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) surface_handle: Option<IOSurfaceHandle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) current_surface_id: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) permission_consumptions: Vec<LivePermissionConsumption>,
}

#[derive(Debug, Serialize)]
pub(super) struct LivePermissionConsumption {
    pub(super) profile_id: String,
    pub(super) origin: String,
    pub(super) feature: String,
    pub(super) grant_revision: u64,
}

impl From<ConsumedPermission> for LivePermissionConsumption {
    fn from(consumed: ConsumedPermission) -> Self {
        Self {
            profile_id: consumed.profile_id.as_str().to_string(),
            origin: consumed.origin.as_str().to_string(),
            feature: consumed.feature.as_str().to_string(),
            grant_revision: consumed.grant_revision,
        }
    }
}

impl LiveResponse {
    fn empty() -> Self {
        Self {
            protocol_version: LIVE_PROTOCOL_VERSION,
            error: None,
            frame: None,
            surface_handle: None,
            current_surface_id: None,
            permission_consumptions: Vec::new(),
        }
    }

    fn error(message: String) -> Self {
        Self {
            protocol_version: LIVE_PROTOCOL_VERSION,
            error: Some(message),
            frame: None,
            surface_handle: None,
            current_surface_id: None,
            permission_consumptions: Vec::new(),
        }
    }

    fn frame(frame: LiveFrameReport) -> Self {
        Self {
            protocol_version: LIVE_PROTOCOL_VERSION,
            error: None,
            frame: Some(frame),
            surface_handle: None,
            current_surface_id: None,
            permission_consumptions: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct LiveFrameReport {
    pub(super) loaded_url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: &'static str,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) device_pixel_ratio: f32,
    pub(super) css_viewport_width: u32,
    pub(super) css_viewport_height: u32,
    pub(super) rgba_byte_count: usize,
    pub(super) pixels_changed: bool,
    pub(super) non_white_pixel_count: u64,
    pub(super) content_pixel_count: u64,
    pub(super) sample_hash: u64,
}

impl LiveFrameReport {
    pub(super) fn new(
        snapshot: &WebViewSnapshot,
        frame: &RenderedFrame,
        device_pixel_ratio: f32,
        pixels_changed: bool,
    ) -> Result<Self, LiveSidecarError> {
        let device_pixel_ratio = normalized_device_pixel_ratio(device_pixel_ratio);
        let css_viewport_width = css_dimension(frame.width(), device_pixel_ratio);
        let css_viewport_height = css_dimension(frame.height(), device_pixel_ratio);
        Ok(Self {
            loaded_url: loaded_url_for_response(snapshot.url())?,
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            rgba_byte_count: frame.rgba_bytes().len(),
            pixels_changed,
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
        })
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) fn from_surface(
        snapshot: &WebViewSnapshot,
        width: u32,
        height: u32,
        device_pixel_ratio: f32,
        pixels_changed: bool,
    ) -> Result<Self, LiveSidecarError> {
        let device_pixel_ratio = normalized_device_pixel_ratio(device_pixel_ratio);
        Ok(Self {
            loaded_url: loaded_url_for_response(snapshot.url())?,
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width,
            height,
            device_pixel_ratio,
            css_viewport_width: css_dimension(width, device_pixel_ratio),
            css_viewport_height: css_dimension(height, device_pixel_ratio),
            rgba_byte_count: 0,
            pixels_changed,
            non_white_pixel_count: 0,
            content_pixel_count: 0,
            sample_hash: 0,
        })
    }
}

fn loaded_url_for_response(value: Option<&str>) -> Result<Option<String>, LiveSidecarError> {
    match value {
        Some(url) if url.len() > MAX_LOADED_URL_BYTES => {
            Err(LiveSidecarError::LoadedUrlTooLong { limit: MAX_LOADED_URL_BYTES })
        }
        Some(url) => Ok(Some(url.to_string())),
        None => Ok(None),
    }
}

fn normalized_device_pixel_ratio(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 { value.clamp(0.5, 5.0) } else { 1.0 }
}

fn css_dimension(value: u32, device_pixel_ratio: f32) -> u32 {
    ((value as f32) / device_pixel_ratio).round().max(1.0) as u32
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
    #[error("live protocol handshake is required before sidecar requests")]
    ProtocolHandshakeRequired,

    #[error("live protocol mismatch: expected {expected}, received {actual}")]
    ProtocolVersionMismatch { expected: u32, actual: u32 },

    #[error("live request line exceeds the {limit}-byte protocol limit")]
    RequestLineTooLarge { limit: usize },

    #[error("request URL exceeds the {limit}-byte live protocol limit")]
    RequestUrlTooLong { limit: usize },

    #[error("loaded URL exceeds the {limit}-byte live protocol limit")]
    LoadedUrlTooLong { limit: usize },

    #[error("response header requires {bytes} bytes; the live protocol limit is {limit}")]
    ResponseHeaderTooLarge { bytes: usize, limit: usize },

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[error("hardware rendering requires --iosurface-mach-service")]
    IOSurfaceMachServiceRequired,

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[error(
        "hardware surface {surface_id} size {surface_width}x{surface_height} does not match frame report {frame_width}x{frame_height}"
    )]
    HardwareSurfaceReportMismatch {
        surface_id: u64,
        surface_width: u32,
        surface_height: u32,
        frame_width: u32,
        frame_height: u32,
    },

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[error("hardware IOSurface handle does not match the presented surface")]
    HardwareSurfaceHandleMismatch,

    #[error("sidecar process is already bound to profile {expected}; received {actual}")]
    ProfileMismatch { expected: String, actual: String },

    #[error("servo profile data directory is already in use: {path}")]
    ProfileDataDirectoryInUse { path: PathBuf },

    #[error("{input} requires both x and y coordinates")]
    IncompletePoint { input: &'static str },

    #[error("frame dimensions overflow the protocol byte count: {width}x{height}")]
    FrameDimensionsOverflow { width: u32, height: u32 },

    #[error("frame dimensions {width}x{height} exceed the {max_dimension}px dimension limit")]
    InvalidFrameDimensions { width: u32, height: u32, max_dimension: u32 },

    #[error("frame requires {bytes} bytes; the protocol limit is {limit}")]
    FrameByteLimitExceeded { bytes: u64, limit: usize },

    #[error("frame byte count mismatch: expected {expected}, received {actual}")]
    FrameByteCountMismatch { expected: usize, actual: usize },

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

pub(super) fn validated_frame_byte_count(
    width: u32,
    height: u32,
) -> Result<usize, LiveSidecarError> {
    if width == 0 || height == 0 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return Err(LiveSidecarError::InvalidFrameDimensions {
            width,
            height,
            max_dimension: MAX_FRAME_DIMENSION,
        });
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(LiveSidecarError::FrameDimensionsOverflow { width, height })?;
    if bytes > MAX_FRAME_BYTE_COUNT as u64 {
        return Err(LiveSidecarError::FrameByteLimitExceeded {
            bytes,
            limit: MAX_FRAME_BYTE_COUNT,
        });
    }
    Ok(bytes as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_defaults_optional_input_fields() -> Result<(), serde_json::Error> {
        let request = serde_json::from_str::<LiveRequest>(
            r#"{"type":"ensure","tab_id":"tab","profile_id":"profile","url":"https://example.com","width":800,"height":600,"site_permission_generation":0}"#,
        )?;

        assert!(matches!(
            request,
            LiveRequest::Ensure {
                page_zoom_percent: 100,
                device_pixel_ratio: 1.0,
                scroll_delta_x: 0,
                scroll_delta_y: 0,
                ready_surface_ids,
                pending_surface_ids,
                ..
            } if ready_surface_ids.is_empty() && pending_surface_ids.is_empty()
        ));
        Ok(())
    }

    #[test]
    fn handshake_deserializes_protocol_version() -> Result<(), serde_json::Error> {
        let request =
            serde_json::from_str::<LiveRequest>(r#"{"type":"handshake","protocol_version":3}"#)?;

        assert!(matches!(request, LiveRequest::Handshake { protocol_version: 3 }));
        Ok(())
    }

    #[test]
    fn site_permission_requires_revision() {
        let request = serde_json::from_str::<LiveRequest>(
            r#"{"type":"ensure","tab_id":"tab","profile_id":"profile","url":"https://example.com","width":800,"height":600,"site_permission_generation":0,"site_permissions":[{"origin":"https://example.com","feature":"camera","state":"allow-once"}]}"#,
        );

        assert!(request.is_err());
    }

    #[test]
    fn frame_layout_enforces_dimension_and_byte_limits() {
        assert!(matches!(
            validated_frame_byte_count(MAX_FRAME_DIMENSION + 1, 1),
            Err(LiveSidecarError::InvalidFrameDimensions { .. })
        ));
        assert!(matches!(
            validated_frame_byte_count(MAX_FRAME_DIMENSION, MAX_FRAME_DIMENSION),
            Err(LiveSidecarError::FrameByteLimitExceeded { .. })
        ));
    }

    #[test]
    fn shutdown_deserializes_from_wire() -> Result<(), serde_json::Error> {
        let request = serde_json::from_str::<LiveRequest>(r#"{"type":"shutdown"}"#)?;

        assert!(matches!(request, LiveRequest::Shutdown));
        Ok(())
    }
}
