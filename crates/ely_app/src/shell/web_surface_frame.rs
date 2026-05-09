use std::sync::Arc;

use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_live::ServoLiveFrame;

#[cfg(all(test, feature = "live-site-smoke"))]
use super::web_surface_geometry::WebSurfaceSize;
use super::web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollOffset};

#[derive(Clone)]
#[cfg_attr(not(all(test, feature = "live-site-smoke")), allow(dead_code))]
pub(super) struct WebSurfaceFrame {
    pub(super) requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    zoom_percent: u16,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    pub(super) image: Arc<RenderImage>,
}

impl WebSurfaceFrame {
    pub(super) fn from_live_frame(
        requested_url: String,
        scroll_offset: WebSurfaceScrollOffset,
        zoom_percent: u16,
        frame: ServoLiveFrame,
    ) -> Result<Self, WebSurfaceError> {
        Self::from_parts(WebSurfaceFrameParts {
            requested_url,
            loaded_url: frame.loaded_url().map(str::to_string),
            title: frame.title().map(str::to_string),
            render_state: frame.render_state().to_string(),
            width: frame.width(),
            height: frame.height(),
            scroll_offset,
            zoom_percent,
            click_point: None,
            typed_text: None,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: frame.non_white_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: frame.content_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: frame.sample_hash(),
            rgba_bytes: frame.into_rgba_bytes(),
        })
    }

    fn from_parts(parts: WebSurfaceFrameParts) -> Result<Self, WebSurfaceError> {
        let Some(image_buffer) =
            ImageBuffer::<Rgba<u8>, _>::from_raw(parts.width, parts.height, parts.rgba_bytes)
        else {
            return Err(WebSurfaceError::InvalidFrameBuffer {
                width: parts.width,
                height: parts.height,
            });
        };

        Ok(Self {
            requested_url: parts.requested_url,
            loaded_url: parts.loaded_url,
            title: parts.title,
            render_state: parts.render_state,
            width: parts.width,
            height: parts.height,
            scroll_offset: parts.scroll_offset,
            zoom_percent: parts.zoom_percent,
            click_point: parts.click_point,
            typed_text: parts.typed_text,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: parts.non_white_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: parts.content_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: parts.sample_hash,
            image: Arc::new(RenderImage::new([image::Frame::new(image_buffer)])),
        })
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn title_label(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.requested_url.clone())
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn url_label(&self) -> &str {
        self.loaded_url.as_deref().unwrap_or(self.requested_url.as_str())
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn detail_label(&self) -> String {
        let mut detail =
            format!("{} {}", self.render_state(), self.scroll_offset.detail_label(self.size()));
        if self.zoom_percent != ely_domain::DEFAULT_ZOOM_PERCENT {
            detail = format!("{detail} zoom={}%", self.zoom_percent);
        }
        if let Some(click_point) = self.click_point {
            detail = format!("{detail} {}", click_point.detail_label());
        }
        match self.typed_text.as_ref() {
            Some(typed_text) => format!("{detail} text={}b", typed_text.len()),
            None => detail,
        }
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn size(&self) -> WebSurfaceSize {
        WebSurfaceSize { width: self.width, height: self.height }
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn render_state(&self) -> &str {
        self.render_state.as_str()
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn scroll_offset(&self) -> WebSurfaceScrollOffset {
        self.scroll_offset
    }

    pub(super) fn zoom_percent(&self) -> u16 {
        self.zoom_percent
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn click_point(&self) -> Option<WebSurfaceClickPoint> {
        self.click_point
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn typed_text(&self) -> Option<&str> {
        self.typed_text.as_deref()
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn non_white_pixel_count(&self) -> u64 {
        self.non_white_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn content_pixel_count(&self) -> u64 {
        self.content_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn sample_hash(&self) -> u64 {
        self.sample_hash
    }
}

struct WebSurfaceFrameParts {
    requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    zoom_percent: u16,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    rgba_bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub(super) enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
}
