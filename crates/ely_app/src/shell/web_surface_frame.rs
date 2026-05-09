use std::sync::Arc;

use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_sidecar::SidecarSnapshot;

use super::{
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_image::renderable_image_buffer,
};

#[derive(Clone)]
pub(super) struct WebSurfaceFrame {
    pub(super) requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
    pub(super) image: Arc<RenderImage>,
}

impl WebSurfaceFrame {
    pub(super) fn from_snapshot(
        requested_url: String,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
        typed_text: Option<String>,
        snapshot: SidecarSnapshot,
    ) -> Result<Self, WebSurfaceError> {
        let width = snapshot.width();
        let height = snapshot.height();
        let loaded_url = snapshot.loaded_url().map(str::to_string);
        let title = snapshot.title().map(str::to_string);
        let render_state = snapshot.render_state().to_string();
        let rgba_bytes = snapshot.into_rgba_bytes();

        let Some(buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_bytes) else {
            return Err(WebSurfaceError::InvalidFrameBuffer { width, height });
        };

        let image_buffer = renderable_image_buffer(buffer);

        Ok(Self {
            requested_url,
            loaded_url,
            title,
            render_state,
            width,
            height,
            scroll_offset,
            click_point,
            typed_text,
            image: Arc::new(RenderImage::new([image::Frame::new(image_buffer)])),
        })
    }

    pub(super) fn title_label(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.requested_url.clone())
    }

    pub(super) fn url_label(&self) -> &str {
        self.loaded_url.as_deref().unwrap_or(self.requested_url.as_str())
    }

    pub(super) fn detail_label(&self) -> String {
        let mut detail =
            format!("{} {}", self.render_state(), self.scroll_offset.detail_label(self.size()));
        if let Some(click_point) = self.click_point {
            detail = format!("{detail} {}", click_point.detail_label());
        }
        match self.typed_text.as_ref() {
            Some(typed_text) => format!("{detail} text={}b", typed_text.len()),
            None => detail,
        }
    }

    pub(super) fn size(&self) -> WebSurfaceSize {
        WebSurfaceSize { width: self.width, height: self.height }
    }

    pub(super) fn render_state(&self) -> &str {
        self.render_state.as_str()
    }

    pub(super) fn scroll_offset(&self) -> WebSurfaceScrollOffset {
        self.scroll_offset
    }

    pub(super) fn click_point(&self) -> Option<WebSurfaceClickPoint> {
        self.click_point
    }

    pub(super) fn typed_text(&self) -> Option<&str> {
        self.typed_text.as_deref()
    }
}

#[derive(Debug, Error)]
pub(super) enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
}
