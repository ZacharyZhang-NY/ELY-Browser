use ely_domain::SitePermissionDecision;
use ely_servo_host::{RenderedFrame, ServoHostError, WebViewSnapshot, WebViewState};
use gpui::NativeSurfaceHandle;
use serde::Serialize;
use thiserror::Error;

pub(crate) struct ServoLiveEnsureRequest {
    pub(crate) tab_id: String,
    pub(crate) profile_id: String,
    pub(crate) url: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) page_zoom_percent: u16,
    /// Display scale factor (1.0 standard, 2.0 Retina). Servo's
    /// WebView lays out CSS pixels = device pixels / hidpi factor.
    pub(crate) device_pixel_ratio: f32,
    pub(crate) native_surface: Option<NativeSurfaceHandle>,
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
    rgba_bytes: Option<Vec<u8>>,
}

impl ServoLiveFrame {
    pub(super) fn from_presented(
        snapshot: WebViewSnapshot,
        width: u32,
        height: u32,
        device_pixel_ratio: f32,
    ) -> Self {
        let (css_viewport_width, css_viewport_height) =
            css_viewport_size(width, height, device_pixel_ratio);
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            render_state: render_state_label(snapshot.state()).to_string(),
            width,
            height,
            device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: 1,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: 1,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: 0,
            rgba_bytes: None,
        }
    }

    pub(super) fn from_rendered(
        snapshot: WebViewSnapshot,
        rendered_frame: RenderedFrame,
        device_pixel_ratio: f32,
    ) -> Self {
        let width = rendered_frame.width();
        let height = rendered_frame.height();
        let (css_viewport_width, css_viewport_height) =
            css_viewport_size(width, height, device_pixel_ratio);
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            render_state: render_state_label(snapshot.state()).to_string(),
            width,
            height,
            device_pixel_ratio,
            css_viewport_width,
            css_viewport_height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: rendered_frame.non_white_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: rendered_frame.content_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: rendered_frame.sample_hash(),
            rgba_bytes: Some(rendered_frame.rgba_bytes().to_vec()),
        }
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
    pub fn into_rgba_bytes(self) -> Option<Vec<u8>> {
        self.rgba_bytes
    }

    #[cfg(test)]
    pub(crate) fn for_test(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Self {
        #[cfg(all(test, feature = "live-site-smoke"))]
        let summary =
            ely_servo_host::RenderedFrameSummary::from_rgba_bytes(width, height, &rgba_bytes);
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
            non_white_pixel_count: summary.non_white_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: summary.content_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: summary.sample_hash(),
            rgba_bytes: Some(rgba_bytes),
        }
    }
}

fn render_state_label(state: &WebViewState) -> &'static str {
    match state {
        WebViewState::Created => "created",
        WebViewState::Loading => "loading",
        WebViewState::Complete => "complete",
        WebViewState::Sleeping => "sleeping",
        WebViewState::Crashed => "crashed",
    }
}

fn css_viewport_size(width: u32, height: u32, device_pixel_ratio: f32) -> (u32, u32) {
    let scale = if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
        device_pixel_ratio
    } else {
        1.0
    };
    (
        ((width as f32) / scale).round().max(1.0) as u32,
        ((height as f32) / scale).round().max(1.0) as u32,
    )
}

#[derive(Debug, Error)]
pub(crate) enum ServoLiveError {
    #[error("servo native surface is unavailable")]
    NativeSurfaceUnavailable,

    #[error("servo scroll input is missing a viewport point")]
    MissingScrollPoint,

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),

    #[error(transparent)]
    Host(#[from] ServoHostError),
}

impl ServoLiveError {
    pub(crate) fn is_runtime_unavailable(&self) -> bool {
        matches!(self, Self::Host(ServoHostError::RuntimeAlreadyStarted))
    }
}
