use std::{error::Error, time::Duration};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use super::WebSurfaceStore;
use crate::shell::web_surface_state::WebSurfaceInputOutcome;

fn assert_applied(outcome: WebSurfaceInputOutcome) {
    assert_eq!(outcome, WebSurfaceInputOutcome::Applied);
}

#[test]
fn typed_text_enters_pending_input_after_clicked_viewport() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_click_point(
        tab.id(),
        tab.url().as_str(),
        point(px(160.0), px(120.0)),
        1.0,
    ));
    assert_applied(store.record_typed_text(tab.id(), tab.url().as_str(), "e"));
    assert_applied(store.record_typed_text(tab.id(), tab.url().as_str(), "l"));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.click_point.map(|point| (point.x(), point.y())), Some((160, 120)));
    assert_eq!(input.typed_text.as_deref(), Some("el"));
    Ok(())
}

#[test]
fn scroll_delta_enters_pending_input_after_wheel() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/list")?;

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_scroll_delta(
        tab.id(),
        tab.url().as_str(),
        point(px(0.0), px(140.0)),
        point(px(320.0), px(240.0)),
        1.0,
    ));
    assert_applied(store.record_scroll_delta(
        tab.id(),
        tab.url().as_str(),
        point(px(0.0), px(60.0)),
        point(px(300.0), px(220.0)),
        1.0,
    ));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.scroll_offset.y(), 200);
    assert_eq!(input.scroll_delta.map(|delta| (delta.x(), delta.y())), Some((0, 200)));
    assert_eq!(input.scroll_point.map(|point| (point.x(), point.y())), Some((300, 220)));
    Ok(())
}

#[test]
fn viewport_size_changes_on_first_clean_measurement() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/resize")?;

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_viewport_size(tab.id(), resized_once_bounds(), 1.0));
    Ok(())
}

/// Regression — a wheel scroll between focusing a field and typing
/// must not erase keyboard focus or buffered keystrokes.
///
/// The `record_scroll_delta` path drops `click_point` on purpose
/// (its viewport coordinates are stale once the page scrolls) but
/// must keep `keyboard_focus` and `typed_text`, so a focused input
/// keeps receiving the user's keystrokes after they wheel-scroll.
/// Locks the fix from commit fcac326-era input-loss work.
#[test]
fn scroll_after_click_keeps_keyboard_focus_and_typed_text() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;
    let url = tab.url().as_str();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_click_point(tab.id(), url, point(px(160.0), px(120.0)), 1.0));
    assert_applied(store.record_typed_text(tab.id(), url, "h"));

    assert_applied(store.record_scroll_delta(
        tab.id(),
        url,
        point(px(0.0), px(140.0)),
        point(px(160.0), px(120.0)),
        1.0,
    ));

    assert_eq!(
        store.record_typed_text(tab.id(), url, "i"),
        WebSurfaceInputOutcome::Applied,
        "scroll must not erase keyboard focus — typing after a scroll should still buffer",
    );

    let input = store.take_pending_input(tab.id(), url);

    assert_eq!(
        input.scroll_delta.map(|delta| (delta.x(), delta.y())),
        Some((0, 140)),
        "scroll delta should reach the sidecar",
    );
    assert_eq!(input.scroll_offset.y(), 140);
    assert!(
        input.click_point.is_none(),
        "post-scroll click coordinates would land on the wrong DOM node — they must be dropped",
    );
    assert_eq!(
        input.typed_text.as_deref(),
        Some("hi"),
        "buffered keystrokes from before AND after the scroll must reach the sidecar",
    );
    Ok(())
}

/// Locks the Retina (2x) scale path: GPUI delivers logical pixels but
/// Servo expects device pixels, so every coordinate that crosses the
/// boundary must be multiplied by `window.scale_factor()`. Without
/// this regression a future refactor could silently revert to the
/// 1.0-only behavior that left every Retina click landing in the
/// upper-left quadrant of the page.
#[test]
fn retina_scale_factor_doubles_every_input_coordinate() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;
    let url = tab.url().as_str();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 2.0));
    assert_applied(store.record_click_point(tab.id(), url, point(px(160.0), px(120.0)), 2.0));
    assert_applied(store.record_typed_text(tab.id(), url, "h"));
    assert_applied(store.record_scroll_delta(
        tab.id(),
        url,
        point(px(0.0), px(140.0)),
        point(px(160.0), px(120.0)),
        2.0,
    ));

    let input = store.take_pending_input(tab.id(), url);

    assert_eq!(
        input.scroll_delta.map(|delta| (delta.x(), delta.y())),
        Some((0, 280)),
        "wheel delta of 140 logical px must be 280 device px on Retina",
    );
    assert_eq!(input.scroll_offset.y(), 280, "scroll offset accumulates in device px");
    assert_eq!(
        input.click_point, None,
        "scroll drops the buffered click — its viewport coords are stale",
    );
    Ok(())
}

/// Locks the precondition that `record_typed_text` requires a prior
/// click to have established keyboard focus. Without this guard, a
/// future refactor could silently start buffering stray keystrokes
/// against an unfocused tab — and the user would see characters land
/// on whichever DOM node Servo last focused, with no visible cause.
#[test]
fn typing_without_a_prior_click_is_rejected() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;
    let url = tab.url().as_str();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_eq!(
        store.record_typed_text(tab.id(), url, "x"),
        WebSurfaceInputOutcome::DroppedNoKeyboardFocus,
        "typing must fail until a click establishes keyboard focus on this tab and url",
    );
    Ok(())
}

/// Negative-path coverage for `DroppedNoViewportBounds`: a click that
/// arrives before the viewport has reported its bounds (race during
/// first paint) must surface as a typed outcome, not a silent `false`.
#[test]
fn click_before_viewport_measured_reports_no_viewport_bounds() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;

    assert_eq!(
        store.record_click_point(tab.id(), tab.url().as_str(), point(px(160.0), px(120.0)), 1.0,),
        WebSurfaceInputOutcome::DroppedNoViewportBounds,
    );
    Ok(())
}

/// Negative-path coverage for `DroppedZeroDelta`: a wheel event whose
/// device-pixel delta rounds to zero must surface as the explicit
/// outcome so the renderer skips an unnecessary repaint.
#[test]
fn zero_wheel_delta_reports_zero_delta() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/list")?;

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_eq!(
        store.record_scroll_delta(
            tab.id(),
            tab.url().as_str(),
            point(px(0.0), px(0.0)),
            point(px(160.0), px(120.0)),
            1.0,
        ),
        WebSurfaceInputOutcome::DroppedZeroDelta,
    );
    Ok(())
}

#[test]
fn hover_input_is_rate_limited() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/hover")?;
    let start = std::time::Instant::now();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_hover_point_at(tab.id(), point(px(10.0), px(10.0)), 1.0, start));
    assert_eq!(
        store.record_hover_point_at(
            tab.id(),
            point(px(12.0), px(12.0)),
            1.0,
            start + Duration::from_millis(8),
        ),
        WebSurfaceInputOutcome::NoChange,
    );
    assert_applied(store.record_hover_point_at(
        tab.id(),
        point(px(44.0), px(45.0)),
        1.0,
        start + Duration::from_millis(33),
    ));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.hover_point.map(|point| (point.x(), point.y())), Some((44, 45)));
    Ok(())
}

/// Pinning the per-tab isolation invariant. A click recorded against
/// tab A must not be drained by, dropped by, or overwritten by any
/// state mutation routed to tab B. The store keys every click on its
/// owning `TabId` via `PerTabSurface`, but the singleton
/// `keyboard_focus` cross-cuts tabs — so a regression that
/// accidentally entangled them (e.g. dropping A's `click_point` when
/// B took focus) would surface here.
#[test]
fn click_on_tab_a_survives_click_on_tab_b() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab_a = web_tab("https://example.com/a")?;
    let tab_b = web_tab("https://example.com/b")?;

    assert_applied(store.record_viewport_size(tab_a.id(), web_bounds(), 1.0));
    assert_applied(store.record_viewport_size(tab_b.id(), web_bounds(), 1.0));

    assert_applied(store.record_click_point(
        tab_a.id(),
        tab_a.url().as_str(),
        point(px(40.0), px(40.0)),
        1.0,
    ));
    assert_applied(store.record_click_point(
        tab_b.id(),
        tab_b.url().as_str(),
        point(px(200.0), px(200.0)),
        1.0,
    ));

    let input_a = store.take_pending_input(tab_a.id(), tab_a.url().as_str());
    assert_eq!(
        input_a.click_point.map(|p| (p.x(), p.y())),
        Some((40, 40)),
        "tab A's click must survive a subsequent click on tab B — \
         per-tab surfaces are independent owners of `click_point`",
    );

    let input_b = store.take_pending_input(tab_b.id(), tab_b.url().as_str());
    assert_eq!(
        input_b.click_point.map(|p| (p.x(), p.y())),
        Some((200, 200)),
        "tab B's click must drain into B's pending input, not A's",
    );
    Ok(())
}

/// Pinning the zero-delta short-circuit's non-effect on a buffered
/// click. `record_scroll_delta` wipes `click_point` (post-scroll
/// coords would target the wrong DOM node), but the early `None`
/// return for `DroppedZeroDelta` must short-circuit *before* the wipe.
/// A future refactor that moved the wipe above the delta check would
/// silently eat clicks whenever a precision-mouse wheel reported a
/// sub-device-pixel delta.
#[test]
fn zero_wheel_delta_must_not_erase_buffered_click() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;
    let url = tab.url().as_str();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_click_point(tab.id(), url, point(px(160.0), px(120.0)), 1.0));

    assert_eq!(
        store.record_scroll_delta(
            tab.id(),
            url,
            point(px(0.0), px(0.0)),
            point(px(160.0), px(120.0)),
            1.0,
        ),
        WebSurfaceInputOutcome::DroppedZeroDelta,
    );

    let input = store.take_pending_input(tab.id(), url);
    assert_eq!(
        input.click_point.map(|p| (p.x(), p.y())),
        Some((160, 120)),
        "a zero-delta wheel event reports DroppedZeroDelta and must not \
         take the `click_point` wipe path — the early return guards it",
    );
    Ok(())
}

/// Pinning the bounds-vs-drain decoupling. A viewport resize between
/// click and drain leaves `viewport_bounds` mutated but does not
/// touch `click_point`, `scroll_offset`, or `requested_url` — the
/// three keys the drain filter checks. The click's stored device-px
/// coordinates remain valid against the new bounds because GPUI
/// re-renders before any new click can arrive.
///
/// If a future refactor stored raw window-relative coords on
/// `click_point` and converted them at drain time, a resize between
/// record and drain would shift the result; this test would still
/// drain "something" but the coordinates would change, surfacing the
/// drift.
#[test]
fn click_survives_viewport_bounds_change_before_drain() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/resize")?;
    let url = tab.url().as_str();

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_applied(store.record_click_point(tab.id(), url, point(px(160.0), px(120.0)), 1.0));

    assert_applied(store.record_viewport_size(tab.id(), resized_once_bounds(), 1.0));

    let input = store.take_pending_input(tab.id(), url);
    assert_eq!(
        input.click_point.map(|p| (p.x(), p.y())),
        Some((160, 120)),
        "a resize-in-progress must not steal the buffered click — \
         the drain filter checks url+scroll_offset, not bounds",
    );
    Ok(())
}

/// Servo's `read_pixels(gl::RGBA, gl::UNSIGNED_BYTE)` writes R-G-B-A
/// in memory order. GPUI's `RenderImage` is documented as "in BGRA
/// format" and the macOS Metal atlas uploads bytes as
/// `MTLPixelFormat::BGRA8Unorm` (B-G-R-A in memory). If the bytes
/// crossed the boundary unchanged, the Metal sampler would read the R
/// byte as B and vice versa — every coloured pixel would render with
/// R and B swapped. `WebSurfaceFrame::from_live_frame` is responsible
/// for swapping the two channels before handing the buffer to GPUI.
///
/// This test fed `(R=255, G=0, B=0, A=255)` — one red RGBA pixel —
/// through the live-frame conversion and asserts the resulting
/// `RenderImage` carries `(B=0, G=0, R=255, A=255)` byte-for-byte.
#[test]
fn live_frame_swaps_red_and_blue_bytes_for_gpui_bgra() -> Result<(), Box<dyn Error>> {
    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let rgba_red_pixel = vec![255u8, 0, 0, 255];
    let live = ServoLiveFrame::for_test(1, 1, rgba_red_pixel);
    let frame = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live,
    )?;

    let image = frame.image.as_ref().ok_or("software frame must produce a RenderImage")?;
    let bytes = image.as_bytes(0).ok_or("RenderImage must expose its first frame's bytes")?;
    assert_eq!(
        bytes,
        &[0u8, 0, 255, 255],
        "Servo's RGBA red pixel must be swapped to BGRA (B=0, G=0, R=255, A=255) \
         before GPUI uploads it as BGRA8Unorm. If this assert says \
         `[255, 0, 0, 255]`, the swap is missing and every coloured \
         pixel on the page renders with R and B exchanged.",
    );
    Ok(())
}

#[test]
fn empty_live_frame_payload_is_rejected() -> Result<(), String> {
    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let result = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        ServoLiveFrame::for_test(1, 1, Vec::new()),
    );

    let error = match result {
        Ok(_) => return Err("empty Servo frame payload reached Ready state".to_string()),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "servo live frame did not include a software image or hardware IOSurface",
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn hardware_live_frame_with_pixel_buffer_skips_software_image() -> Result<(), String> {
    use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA};

    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 1, 1, None)
        .map_err(|status| format!("CVPixelBufferCreate returned status {status}"))?;
    let live = ServoLiveFrame::for_test_with_pixel_buffer(1, 1, pixel_buffer);
    let frame = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live,
    )
    .map_err(|error| error.to_string())?;

    assert!(frame.image.is_none(), "hardware frame should use the CVPixelBuffer surface path");
    assert!(frame.pixel_buffer.is_some(), "hardware frame should carry the imported CVPixelBuffer");
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn hardware_live_frame_rejects_mismatched_surface_size() -> Result<(), String> {
    use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA};

    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 2, 1, None)
        .map_err(|status| format!("CVPixelBufferCreate returned status {status}"))?;
    let live = ServoLiveFrame::for_test_with_pixel_buffer(1, 1, pixel_buffer);
    let result = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live,
    );

    let error = match result {
        Ok(_) => return Err("mismatched hardware surface reached Ready state".to_string()),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "servo hardware surface size 2x1 did not match frame report 1x1",);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn hardware_live_frame_rejects_unsupported_surface_format() -> Result<(), String> {
    use core_video::pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange};

    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let pixel_buffer =
        CVPixelBuffer::new(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, 2, 2, None)
            .map_err(|status| format!("CVPixelBufferCreate returned status {status}"))?;
    let live = ServoLiveFrame::for_test_with_pixel_buffer(2, 2, pixel_buffer);
    let result = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live,
    );

    let error = match result {
        Ok(_) => return Err("unsupported hardware surface format reached Ready state".to_string()),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "servo hardware surface pixel format 0x34323066 is unsupported; expected 32BGRA",
    );
    Ok(())
}

#[cfg(all(target_os = "macos", feature = "live-site-smoke"))]
#[test]
fn hardware_live_frame_samples_bgra_surface_pixels() -> Result<(), String> {
    use core_video::{
        pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_32BGRA},
        r#return::kCVReturnSuccess,
    };

    use crate::services::servo_live::ServoLiveFrame;
    use crate::shell::web_surface_frame::WebSurfaceFrame;
    use crate::shell::web_surface_geometry::WebSurfaceScrollOffset;

    let pixel_buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, 2, 1, None)
        .map_err(|status| format!("CVPixelBufferCreate returned status {status}"))?;
    let lock_status = pixel_buffer.lock_base_address(0);
    if lock_status != kCVReturnSuccess {
        return Err(format!("CVPixelBufferLockBaseAddress returned status {lock_status}"));
    }
    let bytes_per_row = pixel_buffer.get_bytes_per_row();
    #[expect(unsafe_code)]
    unsafe {
        let base_address = pixel_buffer.get_base_address().cast::<u8>();
        let bytes = std::slice::from_raw_parts_mut(base_address, bytes_per_row);
        bytes[0..8].copy_from_slice(&[
            0, 0, 255, 255, // red in BGRA memory order
            255, 255, 255, 255,
        ]);
    }
    let unlock_status = pixel_buffer.unlock_base_address(0);
    if unlock_status != kCVReturnSuccess {
        return Err(format!("CVPixelBufferUnlockBaseAddress returned status {unlock_status}"));
    }

    let live = ServoLiveFrame::for_test_with_pixel_buffer(2, 1, pixel_buffer);
    let frame = WebSurfaceFrame::from_live_frame(
        "https://example.com/".to_string(),
        WebSurfaceScrollOffset::default(),
        100,
        live,
    )
    .map_err(|error| error.to_string())?;

    assert_eq!(frame.non_white_pixel_count(), 1);
    assert_eq!(frame.content_pixel_count(), 1);
    assert_ne!(frame.sample_hash(), 0);
    Ok(())
}

fn web_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)))
}

fn resized_once_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(934.0), px(657.0)))
}

fn web_tab(url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), ProfileId::new(), "Web", UrlText::parse(url)?))
}
