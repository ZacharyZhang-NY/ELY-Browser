use std::{io, path::PathBuf};

use ely_domain::SitePermissionDecision;
use serde::Serialize;
use thiserror::Error;

use super::wire::LiveFrameReport;
use crate::services::servo_sidecar_command::SidecarCommandError;

#[cfg(target_os = "macos")]
use crate::services::iosurface_mach::IOSurfaceMachError;
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;

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
    pub(super) fn from_parts(report: LiveFrameReport, rgba_bytes: Vec<u8>) -> Self {
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

    #[cfg(target_os = "macos")]
    pub(super) fn set_pixel_buffer(&mut self, pixel_buffer: Option<CVPixelBuffer>) {
        self.pixel_buffer = pixel_buffer;
    }

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
