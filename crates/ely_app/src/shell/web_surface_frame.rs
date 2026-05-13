use std::cell::RefCell;
use std::hash::Hasher;
use std::sync::Arc;

use ahash::AHasher;
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA};
use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_live::ServoLiveFrame;

thread_local! {
    /// Single-slot cache of the most-recently-uploaded RGBA payload.
    /// At 60 fps on a 1080p canvas the previous `from_parts` was
    /// unconditionally building a fresh `Arc<RenderImage>` for every
    /// tick — even when the bytes were bit-identical to the last
    /// frame. The cache keys on a 64-bit hash of the raw bytes and
    /// reuses the existing `Arc<RenderImage>` whenever the hash
    /// matches, so steady-state idle pages no longer churn the GPUI
    /// texture pool.
    ///
    /// Uses `AHasher` instead of std's `DefaultHasher`. SipHash13
    /// (default) tops out around ~1.5 GB/s; an 8 MB 1080p frame
    /// hashes in ~5 ms on a modern CPU, which eats roughly 30 % of
    /// the 16 ms scroll budget on every cache-miss frame. AHash
    /// runs ~10 GB/s on the same hardware, dropping the per-frame
    /// hash cost to ~0.8 ms and giving the scroll path back most of
    /// that budget. Hash collisions remain ~1 in 2^64; if they ever
    /// matter we'll trade in length + first/last 32 bytes as a
    /// disambiguator before paying the full memcmp.
    static LAST_FRAME_IMAGE: RefCell<Option<(u64, Arc<RenderImage>)>> =
        const { RefCell::new(None) };
}

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
    /// Software-path image built from RGBA bytes when the sidecar runs
    /// without hardware surface publication.
    pub(super) image: Option<Arc<RenderImage>>,
    /// Hardware-path surface imported from the sidecar's IOSurface.
    /// GPUI is patched locally to present BGRA CVPixelBuffers through
    /// `surface(...)`, so hardware frames can skip the RGBA pipe.
    #[cfg(target_os = "macos")]
    pub(super) pixel_buffer: Option<CVPixelBuffer>,
}

impl WebSurfaceFrame {
    pub(super) fn from_live_frame(
        requested_url: String,
        scroll_offset: WebSurfaceScrollOffset,
        zoom_percent: u16,
        frame: ServoLiveFrame,
    ) -> Result<Self, WebSurfaceError> {
        #[cfg(target_os = "macos")]
        let pixel_buffer = frame.pixel_buffer().cloned();
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
            #[cfg(target_os = "macos")]
            pixel_buffer,
        })
    }

    fn from_parts(parts: WebSurfaceFrameParts) -> Result<Self, WebSurfaceError> {
        #[cfg(target_os = "macos")]
        let has_pixel_buffer = parts.pixel_buffer.is_some();
        #[cfg(not(target_os = "macos"))]
        let has_pixel_buffer = false;

        if parts.rgba_bytes.is_empty() && !has_pixel_buffer {
            return Err(WebSurfaceError::MissingRenderablePayload);
        }

        #[cfg(target_os = "macos")]
        if let Some(pixel_buffer) = parts.pixel_buffer.as_ref() {
            validate_hardware_pixel_buffer(pixel_buffer, parts.width, parts.height)?;
        }

        let image = if parts.rgba_bytes.is_empty() {
            None
        } else {
            // Servo's `read_pixels(gl::RGBA, gl::UNSIGNED_BYTE)` writes
            // R-G-B-A in memory order. GPUI's `RenderImage` is documented
            // as "in BGRA format" and uploads via
            // `MTLPixelFormat::BGRA8Unorm`, which reads B-G-R-A. Hand the bytes across
            // unchanged and the Metal sampler treats R as B (and vice
            // versa) — every coloured pixel renders with R and B swapped.
            // Swap once here so the rest of the pipeline (dedup hash,
            // image buffer, GPU upload) all operate on the same BGRA
            // representation.
            let mut bytes = parts.rgba_bytes;
            swap_red_blue_in_place(&mut bytes);
            let bytes_hash = rgba_hash(&bytes);
            Some(resolve_render_image(parts.width, parts.height, bytes, bytes_hash)?)
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
            image,
            #[cfg(target_os = "macos")]
            pixel_buffer: parts.pixel_buffer,
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
    #[cfg(target_os = "macos")]
    pixel_buffer: Option<CVPixelBuffer>,
}

#[derive(Debug, Error)]
pub(super) enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
    #[error("servo live frame did not include a software image or hardware IOSurface")]
    MissingRenderablePayload,
    #[cfg(target_os = "macos")]
    #[error(
        "servo hardware surface size {actual_width}x{actual_height} did not match frame report {expected_width}x{expected_height}"
    )]
    HardwareSurfaceSizeMismatch {
        expected_width: u32,
        expected_height: u32,
        actual_width: usize,
        actual_height: usize,
    },
    #[cfg(target_os = "macos")]
    #[error("servo hardware surface pixel format 0x{actual:x} is unsupported; expected 32BGRA")]
    UnsupportedHardwareSurfaceFormat { actual: u32 },
}

#[cfg(target_os = "macos")]
fn validate_hardware_pixel_buffer(
    pixel_buffer: &CVPixelBuffer,
    expected_width: u32,
    expected_height: u32,
) -> Result<(), WebSurfaceError> {
    let actual_width = pixel_buffer.get_width();
    let actual_height = pixel_buffer.get_height();
    if actual_width != expected_width as usize || actual_height != expected_height as usize {
        return Err(WebSurfaceError::HardwareSurfaceSizeMismatch {
            expected_width,
            expected_height,
            actual_width,
            actual_height,
        });
    }

    let actual_format = pixel_buffer.get_pixel_format();
    if actual_format != kCVPixelFormatType_32BGRA {
        return Err(WebSurfaceError::UnsupportedHardwareSurfaceFormat { actual: actual_format });
    }

    Ok(())
}

/// Swap byte 0 and byte 2 of every 4-byte pixel, converting Servo's
/// RGBA8 output into GPUI's expected BGRA8 in place. ~0.8 ms on a
/// 1080p frame; cheap relative to the AHash pass that follows.
fn swap_red_blue_in_place(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

fn rgba_hash(bytes: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(bytes);
    hasher.finish()
}

fn resolve_render_image(
    width: u32,
    height: u32,
    rgba_bytes: Vec<u8>,
    bytes_hash: u64,
) -> Result<Arc<RenderImage>, WebSurfaceError> {
    LAST_FRAME_IMAGE.with(|cache| -> Result<Arc<RenderImage>, WebSurfaceError> {
        let mut cache = cache.borrow_mut();
        if let Some((cached_hash, cached_image)) = cache.as_ref()
            && *cached_hash == bytes_hash
        {
            return Ok(cached_image.clone());
        }

        let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_bytes)
            .ok_or(WebSurfaceError::InvalidFrameBuffer { width, height })?;
        let new_image = Arc::new(RenderImage::new([image::Frame::new(image_buffer)]));
        *cache = Some((bytes_hash, new_image.clone()));
        Ok(new_image)
    })
}
