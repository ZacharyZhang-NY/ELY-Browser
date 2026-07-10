use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA};

use crate::services::servo_live::ServoLiveFrame;

use super::{WebSurfaceError, WebSurfaceFrame};
use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

#[test]
fn hardware_frame_keeps_pixel_buffer_without_software_image() -> Result<(), WebSurfaceError> {
    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 64, 48, None)
        .map_err(|_| WebSurfaceError::MissingRenderablePayload)?;
    let live_frame = ServoLiveFrame::for_test_with_pixel_buffer(64, 48, 7, pixel_buffer);

    let frame = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live_frame,
    )?;

    assert!(frame.has_hardware_surface());
    assert!(frame.hardware_surface.is_some());
    assert!(frame.image.is_none());
    Ok(())
}

#[test]
fn hardware_frame_identity_participates_in_equality() -> Result<(), WebSurfaceError> {
    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 64, 48, None)
        .map_err(|_| WebSurfaceError::MissingRenderablePayload)?;
    let first = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_pixel_buffer(64, 48, 7, pixel_buffer.clone()),
    )?;
    let second = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_pixel_buffer(64, 48, 8, pixel_buffer),
    )?;

    assert!(!first.has_same_render_as(&second));
    Ok(())
}

#[test]
fn same_hardware_surface_with_new_pixels_triggers_repaint() -> Result<(), WebSurfaceError> {
    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 64, 48, None)
        .map_err(|_| WebSurfaceError::MissingRenderablePayload)?;
    let first = hardware_frame(7, pixel_buffer.clone(), false)?;
    let changed = hardware_frame(7, pixel_buffer, true)?;

    assert!(!first.has_same_render_as(&changed));
    Ok(())
}

#[test]
fn metadata_only_hardware_frame_reuses_same_surface() -> Result<(), WebSurfaceError> {
    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 64, 48, None)
        .map_err(|_| WebSurfaceError::MissingRenderablePayload)?;
    let first = hardware_frame(7, pixel_buffer.clone(), true)?;
    let metadata_only = hardware_frame(7, pixel_buffer, false)?;

    assert!(first.has_same_render_as(&metadata_only));
    Ok(())
}

#[test]
fn hardware_frame_validates_reported_dimensions() -> Result<(), String> {
    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 64, 48, None)
        .map_err(|status| format!("test pixel buffer creation failed: {status}"))?;
    let live_frame = ServoLiveFrame::for_test_with_pixel_buffer(96, 72, 7, pixel_buffer);

    assert!(matches!(
        WebSurfaceFrame::from_live_frame(
            "https://example.com/".to_string(),
            WebSurfaceScrollOffset::default(),
            100,
            live_frame,
        ),
        Err(WebSurfaceError::HardwareSurfaceSizeMismatch { .. })
    ));
    Ok(())
}

fn hardware_frame(
    surface_id: u64,
    pixel_buffer: CVPixelBuffer,
    pixels_changed: bool,
) -> Result<WebSurfaceFrame, WebSurfaceError> {
    WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test_with_hardware_change(
            64,
            48,
            surface_id,
            pixel_buffer,
            pixels_changed,
        ),
    )
}
