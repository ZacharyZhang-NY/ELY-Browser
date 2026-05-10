use std::error::Error;

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use super::WebSurfaceStore;

#[test]
fn typed_text_enters_pending_input_after_clicked_viewport() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;

    assert!(store.record_viewport_size(tab.id(), web_bounds()));
    assert!(store.record_click_point(tab.id(), tab.url().as_str(), point(px(160.0), px(120.0))));
    assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "e"));
    assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "l"));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.click_point.map(|point| (point.x(), point.y())), Some((160, 120)));
    assert_eq!(input.typed_text.as_deref(), Some("el"));
    Ok(())
}

#[test]
fn scroll_delta_enters_pending_input_after_wheel() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/list")?;

    assert!(store.record_viewport_size(tab.id(), web_bounds()));
    assert!(store.record_scroll_delta(tab.id(), tab.url().as_str(), point(px(0.0), px(140.0))));
    assert!(store.record_scroll_delta(tab.id(), tab.url().as_str(), point(px(0.0), px(60.0))));

    let input = store.take_pending_input(tab.id(), tab.url().as_str());

    assert_eq!(input.scroll_offset.y(), 200);
    assert_eq!(input.scroll_delta.map(|delta| (delta.x(), delta.y())), Some((0, 200)));
    Ok(())
}

#[test]
fn viewport_size_changes_after_stable_second_measurement() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/resize")?;

    assert!(store.record_viewport_size(tab.id(), web_bounds()));
    assert!(!store.record_viewport_size(tab.id(), resized_once_bounds()));
    assert!(store.record_viewport_size(tab.id(), resized_once_bounds()));
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

    assert!(store.record_viewport_size(tab.id(), web_bounds()));
    assert!(store.record_click_point(tab.id(), url, point(px(160.0), px(120.0))));
    assert!(store.record_typed_text(tab.id(), url, "h"));

    assert!(store.record_scroll_delta(tab.id(), url, point(px(0.0), px(140.0))));

    assert!(
        store.record_typed_text(tab.id(), url, "i"),
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

    assert!(store.record_viewport_size(tab.id(), web_bounds()));
    assert!(
        !store.record_typed_text(tab.id(), url, "x"),
        "typing must fail until a click establishes keyboard focus on this tab and url",
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
