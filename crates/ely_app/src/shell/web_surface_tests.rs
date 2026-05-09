use std::error::Error;

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use super::{ProfileDataMode, WebSurfaceStore};

#[test]
fn typed_text_enters_snapshot_request_after_clicked_viewport() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/form")?;

    let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)));
    assert!(store.record_viewport_size(tab.id(), bounds));
    assert!(store.record_click_point(tab.id(), tab.url().as_str(), point(px(160.0), px(120.0))));
    assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "e"));
    assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "l"));

    let request = store
        .prepare_request(&tab, ProfileDataMode::Persistent)
        .ok_or("missing web surface request")?;

    assert_eq!(request.typed_text.as_deref(), Some("el"));
    assert_eq!(request.snapshot_request.typed_text_for_test(), Some("el"));
    assert_eq!(request.snapshot_request.profile_id_for_test(), tab.profile_id());
    assert_eq!(request.zoom_percent, ely_domain::DEFAULT_ZOOM_PERCENT);
    assert_eq!(
        request.snapshot_request.page_zoom_percent_for_test(),
        ely_domain::DEFAULT_ZOOM_PERCENT
    );
    Ok(())
}

#[test]
fn private_profile_enters_transient_snapshot_request() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let tab = web_tab("https://example.com/private")?;

    let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)));
    assert!(store.record_viewport_size(tab.id(), bounds));

    let request = store
        .prepare_request(&tab, ProfileDataMode::Transient)
        .ok_or("missing web surface request")?;

    assert_eq!(request.snapshot_request.profile_data_mode_for_test(), ProfileDataMode::Transient);
    Ok(())
}

#[test]
fn tab_zoom_enters_snapshot_request() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let mut tab = web_tab("https://example.com/zoom")?;
    tab.set_zoom_percent(125)?;

    let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)));
    assert!(store.record_viewport_size(tab.id(), bounds));

    let request = store
        .prepare_request(&tab, ProfileDataMode::Persistent)
        .ok_or("missing web surface request")?;

    assert_eq!(request.zoom_percent, 125);
    assert_eq!(request.snapshot_request.page_zoom_percent_for_test(), 125);
    Ok(())
}

fn web_tab(url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), ProfileId::new(), "Web", UrlText::parse(url)?))
}
