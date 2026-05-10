use std::error::Error;

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
        1.0,
    ));
    assert_applied(store.record_scroll_delta(
        tab.id(),
        tab.url().as_str(),
        point(px(0.0), px(60.0)),
        1.0,
    ));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.scroll_offset.y(), 200);
    assert_eq!(input.scroll_delta.map(|delta| (delta.x(), delta.y())), Some((0, 200)));
    Ok(())
}

#[test]
fn viewport_size_changes_after_stable_second_measurement() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/resize")?;

    assert_applied(store.record_viewport_size(tab.id(), web_bounds(), 1.0));
    assert_eq!(
        store.record_viewport_size(tab.id(), resized_once_bounds(), 1.0),
        WebSurfaceInputOutcome::Buffered,
        "first sighting of a new size must wait for a confirming second measurement",
    );
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

    assert_applied(store.record_scroll_delta(tab.id(), url, point(px(0.0), px(140.0)), 1.0));

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
    assert_applied(store.record_scroll_delta(tab.id(), url, point(px(0.0), px(140.0)), 2.0));

    let input = store.take_pending_input(tab.id(), url);

    assert_eq!(
        input.scroll_delta.map(|delta| (delta.x(), delta.y())),
        Some((0, 280)),
        "wheel delta of 140 logical px must be 280 device px on Retina",
    );
    assert_eq!(input.scroll_offset.y(), 280, "scroll offset accumulates in device px");
    assert_eq!(
        input.click_point,
        None,
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
        store.record_click_point(
            tab.id(),
            tab.url().as_str(),
            point(px(160.0), px(120.0)),
            1.0,
        ),
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
            1.0,
        ),
        WebSurfaceInputOutcome::DroppedZeroDelta,
    );
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
