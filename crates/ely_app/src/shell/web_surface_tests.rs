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

fn web_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)))
}

fn resized_once_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(934.0), px(657.0)))
}

fn web_tab(url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), ProfileId::new(), "Web", UrlText::parse(url)?))
}
