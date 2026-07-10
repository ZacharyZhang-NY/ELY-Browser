#[cfg(target_os = "macos")]
use std::sync::Arc;
use std::{collections::TryReserveError, io, path::PathBuf};

use ely_domain::SitePermissionDecision;
use serde::Serialize;
use thiserror::Error;

use super::wire::LiveFrameReport;
use crate::services::servo_sidecar_command::SidecarCommandError;

#[cfg(target_os = "macos")]
use crate::services::iosurface_mach::IOSurfaceMachError;
#[cfg(target_os = "macos")]
use crate::services::iosurface_metal::HardwareSurfaceBacking;
#[cfg(all(test, target_os = "macos"))]
use core_video::pixel_buffer::CVPixelBuffer;

pub(crate) struct ServoLiveEnsureRequest {
    pub(crate) tab_id: String,
    pub(crate) profile_id: String,
    pub(crate) url: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) page_zoom_percent: u16,
    /// Display scale factor used to derive Servo's CSS viewport.
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
    pixels_changed: bool,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    rgba_bytes: Option<Vec<u8>>,
    #[cfg(target_os = "macos")]
    hardware_surface: Option<Arc<HardwareSurfaceBacking>>,
    #[cfg(target_os = "macos")]
    hardware_surface_id: Option<u64>,
}

impl ServoLiveFrame {
    pub(super) fn from_parts(report: LiveFrameReport, rgba_bytes: Option<Vec<u8>>) -> Self {
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
            pixels_changed: report.pixels_changed,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: report.non_white_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: report.content_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: report.sample_hash,
            rgba_bytes,
            #[cfg(target_os = "macos")]
            hardware_surface: None,
            #[cfg(target_os = "macos")]
            hardware_surface_id: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn set_hardware_surface(
        &mut self,
        surface_id: u64,
        surface: Option<Arc<HardwareSurfaceBacking>>,
    ) {
        self.hardware_surface_id = surface.as_ref().map(|_| surface_id);
        self.hardware_surface = surface;
    }

    #[cfg(target_os = "macos")]
    #[must_use]
    pub fn hardware_surface_id(&self) -> Option<u64> {
        self.hardware_surface_id
    }

    #[cfg(target_os = "macos")]
    #[must_use]
    pub(crate) fn hardware_surface(&self) -> Option<&Arc<HardwareSurfaceBacking>> {
        self.hardware_surface.as_ref()
    }

    pub(super) fn has_software_payload(&self) -> bool {
        self.rgba_bytes.is_some()
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

    #[must_use]
    pub fn pixels_changed(&self) -> bool {
        self.pixels_changed
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
    pub fn into_rgba_bytes(self) -> Option<Vec<u8>> {
        self.rgba_bytes
    }

    #[cfg(test)]
    pub(crate) fn for_test(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Self {
        Self::for_test_with_render_state(width, height, rgba_bytes, "complete")
    }

    #[cfg(test)]
    pub(crate) fn for_test_with_render_state(
        width: u32,
        height: u32,
        rgba_bytes: Vec<u8>,
        render_state: &str,
    ) -> Self {
        #[cfg(all(test, feature = "live-site-smoke"))]
        let summary =
            ely_servo_host::RenderedFrameSummary::from_rgba_bytes(width, height, &rgba_bytes);
        Self {
            loaded_url: Some("https://example.com/".to_string()),
            title: Some("Example".to_string()),
            render_state: render_state.to_string(),
            width,
            height,
            device_pixel_ratio: 1.0,
            css_viewport_width: width,
            css_viewport_height: height,
            pixels_changed: true,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: summary.non_white_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: summary.content_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: summary.sample_hash(),
            rgba_bytes: Some(rgba_bytes),
            #[cfg(target_os = "macos")]
            hardware_surface: None,
            #[cfg(target_os = "macos")]
            hardware_surface_id: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_with_title(
        width: u32,
        height: u32,
        rgba_bytes: Vec<u8>,
        render_state: &str,
        title: &str,
    ) -> Self {
        let mut frame = Self::for_test_with_render_state(width, height, rgba_bytes, render_state);
        frame.title = Some(title.to_string());
        frame
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn for_test_with_pixel_buffer(
        width: u32,
        height: u32,
        surface_id: u64,
        pixel_buffer: CVPixelBuffer,
    ) -> Self {
        Self::for_test_with_hardware_change(width, height, surface_id, pixel_buffer, true)
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn for_test_with_hardware_change(
        width: u32,
        height: u32,
        surface_id: u64,
        pixel_buffer: CVPixelBuffer,
        pixels_changed: bool,
    ) -> Self {
        let mut frame = Self::for_test(width, height, Vec::new());
        frame.rgba_bytes = None;
        frame.hardware_surface = Some(HardwareSurfaceBacking::new(pixel_buffer));
        frame.hardware_surface_id = Some(surface_id);
        frame.pixels_changed = pixels_changed;
        frame
    }
}

fn css_viewport_size_from_report(report: &LiveFrameReport) -> (u32, u32) {
    let scale = if report.device_pixel_ratio.is_finite() && report.device_pixel_ratio > 0.0 {
        report.device_pixel_ratio
    } else {
        1.0
    };
    let fallback_width = ((report.width as f32) / scale).round().max(1.0) as u32;
    let fallback_height = ((report.height as f32) / scale).round().max(1.0) as u32;
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

    #[error("servo live sidecar protocol mismatch: expected {expected}, received {actual:?}")]
    ProtocolVersionMismatch { expected: u32, actual: Option<u32> },

    #[error("servo live sidecar {operation} timed out after {timeout_millis} ms")]
    RequestTimedOut { operation: &'static str, timeout_millis: u128 },

    #[error("servo live sidecar failed: {message}")]
    SidecarFailed { message: String },

    #[error("servo live sidecar response header exceeded {limit} bytes")]
    ResponseHeaderTooLarge { limit: usize },

    #[error("failed to read servo live frame bytes: {0}")]
    FrameRead(#[source] io::Error),

    #[error(
        "servo live frame dimensions {width}x{height} exceed the {max_dimension}px dimension limit"
    )]
    InvalidFrameDimensions { width: u32, height: u32, max_dimension: u32 },

    #[error("servo live frame requires {bytes} bytes; the limit is {limit}")]
    FrameByteLimitExceeded { bytes: u64, limit: usize },

    #[error(
        "servo live sidecar advertised {advertised} frame bytes for {width}x{height}; expected {expected}"
    )]
    InvalidFrameByteCount { advertised: usize, expected: u64, width: u32, height: u32 },

    #[error("failed to reserve {bytes} bytes for a servo live frame: {source}")]
    FrameAllocation {
        bytes: usize,
        #[source]
        source: TryReserveError,
    },

    #[error("invalid servo live sidecar response: {message}")]
    InvalidResponse { message: &'static str },

    #[cfg(target_os = "macos")]
    #[error(
        "servo live IOSurface import failed for surface {surface_id:#x} mach port 0x{mach_port_name:x}: {message}"
    )]
    IOSurfaceImportFailed { surface_id: u64, mach_port_name: u32, message: String },

    #[cfg(target_os = "macos")]
    #[error("servo live IOSurface backing failed for surface {surface_id:#x}: {message}")]
    IOSurfaceBackingFailed { surface_id: u64, message: String },

    #[cfg(target_os = "macos")]
    #[error("failed to spawn servo live IOSurface importer: {0}")]
    IOSurfaceImportWorker(#[source] io::Error),

    #[cfg(target_os = "macos")]
    #[error(transparent)]
    IOSurfaceMach(#[from] IOSurfaceMachError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    SidecarCommand(#[from] SidecarCommandError),
}

impl ServoLiveError {
    pub(crate) fn is_runtime_unavailable(&self) -> bool {
        match self {
            Self::SidecarExited
            | Self::ProtocolVersionMismatch { .. }
            | Self::RequestTimedOut { .. }
            | Self::ResponseHeaderTooLarge { .. }
            | Self::InvalidFrameDimensions { .. }
            | Self::FrameByteLimitExceeded { .. }
            | Self::InvalidFrameByteCount { .. }
            | Self::FrameAllocation { .. }
            | Self::InvalidResponse { .. }
            | Self::Json(_) => true,
            #[cfg(target_os = "macos")]
            Self::IOSurfaceImportFailed { .. }
            | Self::IOSurfaceBackingFailed { .. }
            | Self::IOSurfaceImportWorker(_)
            | Self::IOSurfaceMach(_) => true,
            Self::Command(error) | Self::FrameRead(error) => matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
            ),
            Self::SidecarBinaryUnavailable { .. }
            | Self::PipeUnavailable { .. }
            | Self::SidecarFailed { .. }
            | Self::SidecarCommand(_) => false,
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn iosurface_transport_failures_invalidate_runtime() {
        let errors = [
            ServoLiveError::IOSurfaceImportFailed {
                surface_id: 7,
                mach_port_name: 11,
                message: "injected".to_string(),
            },
            ServoLiveError::IOSurfaceImportWorker(io::Error::other("injected")),
            ServoLiveError::IOSurfaceMach(IOSurfaceMachError::InvalidMessage),
            ServoLiveError::IOSurfaceBackingFailed {
                surface_id: 7,
                message: "injected".to_string(),
            },
        ];

        assert!(errors.iter().all(ServoLiveError::is_runtime_unavailable));
    }
}
