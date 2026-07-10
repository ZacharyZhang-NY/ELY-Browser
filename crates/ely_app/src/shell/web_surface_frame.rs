use std::cell::RefCell;
use std::hash::Hasher;
use std::sync::Arc;

use ahash::AHasher;
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA};
use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

#[cfg(target_os = "macos")]
use crate::services::iosurface_metal::HardwareSurfaceBacking;
use crate::services::servo_live::ServoLiveFrame;

thread_local! {
    /// Single-slot cache for byte-identical software frames.
    /// AHash keeps the 1080p hash pass below the frame budget while
    /// avoiding repeated GPUI texture allocation for idle pages.
    static LAST_FRAME_IMAGE: RefCell<Option<CachedRenderImage>> =
        const { RefCell::new(None) };
}

struct CachedRenderImage {
    width: u32,
    height: u32,
    bytes_hash: u64,
    image: Arc<RenderImage>,
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
    device_pixel_ratio: f32,
    css_viewport_width: u32,
    css_viewport_height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    zoom_percent: u16,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
    pixels_changed: bool,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    pub(super) image: Option<Arc<RenderImage>>,
    #[cfg(target_os = "macos")]
    pub(super) hardware_surface: Option<Arc<HardwareSurfaceBacking>>,
    #[cfg(target_os = "macos")]
    hardware_surface_id: Option<u64>,
}

impl WebSurfaceFrame {
    pub(super) fn from_live_frame(
        requested_url: String,
        scroll_offset: WebSurfaceScrollOffset,
        zoom_percent: u16,
        frame: ServoLiveFrame,
    ) -> Result<Self, WebSurfaceError> {
        #[cfg(target_os = "macos")]
        let hardware_surface = frame.hardware_surface().cloned();
        #[cfg(target_os = "macos")]
        let hardware_surface_id = frame.hardware_surface_id();
        Self::from_parts(WebSurfaceFrameParts {
            requested_url,
            loaded_url: frame.loaded_url().map(str::to_string),
            title: frame.title().map(str::to_string),
            render_state: frame.render_state().to_string(),
            width: frame.width(),
            height: frame.height(),
            device_pixel_ratio: frame.device_pixel_ratio(),
            css_viewport_width: frame.css_viewport_width(),
            css_viewport_height: frame.css_viewport_height(),
            scroll_offset,
            zoom_percent,
            click_point: None,
            typed_text: None,
            pixels_changed: frame.pixels_changed(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: frame.non_white_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: frame.content_pixel_count(),
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: frame.sample_hash(),
            rgba_bytes: frame.into_rgba_bytes(),
            #[cfg(target_os = "macos")]
            hardware_surface,
            #[cfg(target_os = "macos")]
            hardware_surface_id,
        })
    }

    fn from_parts(parts: WebSurfaceFrameParts) -> Result<Self, WebSurfaceError> {
        #[cfg(target_os = "macos")]
        if let Some(surface) = parts.hardware_surface.as_ref() {
            validate_hardware_pixel_buffer(surface.pixel_buffer(), parts.width, parts.height)?;
        }
        #[cfg(target_os = "macos")]
        let has_hardware_surface = parts.hardware_surface.is_some();
        #[cfg(not(target_os = "macos"))]
        let has_hardware_surface = false;
        if parts.rgba_bytes.is_none() && !has_hardware_surface {
            return Err(WebSurfaceError::MissingRenderablePayload);
        }

        #[cfg(all(test, feature = "live-site-smoke"))]
        let pixel_sample = pixel_sample_for_parts(&parts)?;

        let image = match parts.rgba_bytes {
            Some(mut bytes) => {
                if bytes.is_empty() {
                    return Err(WebSurfaceError::MissingRenderablePayload);
                }
                // Servo's RGBA8 readback is converted once for GPUI's BGRA upload path.
                swap_red_blue_in_place(&mut bytes);
                let bytes_hash = rgba_hash(&bytes);
                Some(resolve_render_image(parts.width, parts.height, bytes, bytes_hash)?)
            }
            None => None,
        };

        Ok(Self {
            requested_url: parts.requested_url,
            loaded_url: parts.loaded_url,
            title: parts.title,
            render_state: parts.render_state,
            width: parts.width,
            height: parts.height,
            device_pixel_ratio: parts.device_pixel_ratio,
            css_viewport_width: parts.css_viewport_width,
            css_viewport_height: parts.css_viewport_height,
            scroll_offset: parts.scroll_offset,
            zoom_percent: parts.zoom_percent,
            click_point: parts.click_point,
            typed_text: parts.typed_text,
            pixels_changed: parts.pixels_changed,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: pixel_sample.non_white_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: pixel_sample.content_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            sample_hash: pixel_sample.sample_hash,
            image,
            #[cfg(target_os = "macos")]
            hardware_surface: parts.hardware_surface,
            #[cfg(target_os = "macos")]
            hardware_surface_id: parts.hardware_surface_id,
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
        WebSurfaceSize {
            width: self.width,
            height: self.height,
            device_pixel_ratio_percent: (self.device_pixel_ratio * 100.0).round() as u16,
        }
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn css_viewport_size(&self) -> (u32, u32) {
        (self.css_viewport_width, self.css_viewport_height)
    }

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

    pub(super) fn loaded_url(&self) -> Option<&str> {
        self.loaded_url.as_deref()
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn has_same_render_as(&self, other: &Self) -> bool {
        #[cfg(target_os = "macos")]
        if self.hardware_surface.is_some() || other.hardware_surface.is_some() {
            return self.hardware_surface.is_some()
                && other.hardware_surface.is_some()
                && self.hardware_surface_id == other.hardware_surface_id
                && !other.pixels_changed
                && self.metadata_matches(other);
        }
        let image_matches = match (self.image.as_ref(), other.image.as_ref()) {
            (Some(image), Some(other_image)) => Arc::ptr_eq(image, other_image),
            (None, None) => true,
            _ => false,
        };

        image_matches && self.metadata_matches(other)
    }

    fn metadata_matches(&self, other: &Self) -> bool {
        self.requested_url == other.requested_url
            && self.loaded_url == other.loaded_url
            && self.title == other.title
            && self.render_state == other.render_state
            && self.width == other.width
            && self.height == other.height
            && self.device_pixel_ratio == other.device_pixel_ratio
            && self.css_viewport_width == other.css_viewport_width
            && self.css_viewport_height == other.css_viewport_height
            && self.scroll_offset == other.scroll_offset
            && self.zoom_percent == other.zoom_percent
            && self.click_point == other.click_point
            && self.typed_text == other.typed_text
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

    #[cfg(test)]
    pub(super) fn has_hardware_surface(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.hardware_surface.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    #[cfg(all(test, feature = "live-site-smoke", target_os = "macos"))]
    pub(super) fn hardware_surface_id_for_test(&self) -> Option<u64> {
        self.hardware_surface_id
    }
}

struct WebSurfaceFrameParts {
    requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    device_pixel_ratio: f32,
    css_viewport_width: u32,
    css_viewport_height: u32,
    scroll_offset: WebSurfaceScrollOffset,
    zoom_percent: u16,
    click_point: Option<WebSurfaceClickPoint>,
    typed_text: Option<String>,
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

#[derive(Debug, Error)]
pub(super) enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
    #[error("servo live frame did not include renderable pixels")]
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

#[cfg(all(test, feature = "live-site-smoke"))]
struct WebSurfacePixelSample {
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    sample_hash: u64,
}

#[cfg(all(test, feature = "live-site-smoke"))]
fn pixel_sample_for_parts(
    parts: &WebSurfaceFrameParts,
) -> Result<WebSurfacePixelSample, WebSurfaceError> {
    Ok(WebSurfacePixelSample {
        non_white_pixel_count: parts.non_white_pixel_count,
        content_pixel_count: parts.content_pixel_count,
        sample_hash: parts.sample_hash,
    })
}

fn resolve_render_image(
    width: u32,
    height: u32,
    rgba_bytes: Vec<u8>,
    bytes_hash: u64,
) -> Result<Arc<RenderImage>, WebSurfaceError> {
    LAST_FRAME_IMAGE.with(|cache| -> Result<Arc<RenderImage>, WebSurfaceError> {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.as_ref()
            && cached.width == width
            && cached.height == height
            && cached.bytes_hash == bytes_hash
            && cached.image.as_bytes(0) == Some(rgba_bytes.as_slice())
        {
            return Ok(cached.image.clone());
        }

        let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_bytes)
            .ok_or(WebSurfaceError::InvalidFrameBuffer { width, height })?;
        let new_image = Arc::new(RenderImage::new([image::Frame::new(image_buffer)]));
        *cache = Some(CachedRenderImage { width, height, bytes_hash, image: new_image.clone() });
        Ok(new_image)
    })
}

#[cfg(test)]
#[path = "web_surface_frame_cache_tests.rs"]
mod cache_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "web_surface_frame_hardware_tests.rs"]
mod hardware_tests;
