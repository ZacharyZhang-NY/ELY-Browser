use std::sync::Arc;

use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_sidecar::SidecarSnapshot;

use super::{
    web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_image::renderable_image_buffer,
};

#[derive(Clone)]
pub(super) struct WebSurfaceFrame {
    pub(super) requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    width: u32,
    height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    pub(super) image: Arc<RenderImage>,
}

impl WebSurfaceFrame {
    pub(super) fn from_snapshot(
        requested_url: String,
        scroll_offset: WebSurfaceScrollOffset,
        snapshot: SidecarSnapshot,
    ) -> Result<Self, WebSurfaceError> {
        let width = snapshot.width();
        let height = snapshot.height();
        let loaded_url = snapshot.loaded_url().map(str::to_string);
        let title = snapshot.title().map(str::to_string);
        let rgba_bytes = snapshot.into_rgba_bytes();

        let Some(buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_bytes) else {
            return Err(WebSurfaceError::InvalidFrameBuffer { width, height });
        };

        let image_buffer = renderable_image_buffer(buffer);

        Ok(Self {
            requested_url,
            loaded_url,
            title,
            width,
            height,
            scroll_offset,
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
        self.scroll_offset.detail_label(self.size())
    }

    pub(super) fn size(&self) -> WebSurfaceSize {
        WebSurfaceSize { width: self.width, height: self.height }
    }

    pub(super) fn scroll_offset(&self) -> WebSurfaceScrollOffset {
        self.scroll_offset
    }
}

#[derive(Debug, Error)]
pub(super) enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
}
