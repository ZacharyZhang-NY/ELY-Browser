use std::error::Error;

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use crate::{
    services::ProfileDataMode,
    services::prd_live_sites::{
        LiveSiteCase, PRD_REFERENCE_SITE_CASES, PRD_TOP_SITE_CASES,
        assert_prd_reference_urls_are_covered,
    },
    shell::{
        web_surface_frame::WebSurfaceFrame,
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::WebSurfaceState,
    },
};

use super::WebSurfaceStore;

const LIVE_SURFACE_WIDTH: u32 = 934;
const LIVE_SURFACE_HEIGHT: u32 = 657;

#[test]
fn web_surface_cases_cover_prd_reference_urls() -> Result<(), Box<dyn Error>> {
    assert_prd_reference_urls_are_covered()
}

#[test]
fn web_surface_opens_and_renders_prd_top_sites() -> Result<(), Box<dyn Error>> {
    assert_web_surfaces_render(PRD_TOP_SITE_CASES)
}

#[test]
fn web_surface_opens_and_renders_prd_reference_sites() -> Result<(), Box<dyn Error>> {
    assert_web_surfaces_render(PRD_REFERENCE_SITE_CASES)
}

fn assert_web_surfaces_render(cases: &[LiveSiteCase]) -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    for case in cases {
        let tab = web_tab(case.url)?;
        let bounds = live_surface_bounds();

        assert!(store.record_viewport_size(tab.id(), bounds), "{}", case.url);
        let request = store
            .prepare_request(&tab, ProfileDataMode::Persistent)
            .ok_or_else(|| format!("missing web surface request for {}", case.url))?;
        let tab_id = request.tab_id.clone();
        let snapshot = request.client.snapshot(request.snapshot_request)?;
        let frame = WebSurfaceFrame::from_snapshot(
            request.requested_url,
            request.scroll_offset,
            request.click_point,
            request.typed_text,
            snapshot,
        )?;

        assert_prd_frame_is_ready(&frame, case);
        store.finish(tab_id, WebSurfaceState::Ready(frame));
        let Some(WebSurfaceState::Ready(frame)) = store.state(tab.id()) else {
            return Err(format!("web surface state is not ready for {}", case.url).into());
        };
        assert_prd_frame_is_ready(frame, case);
    }
    Ok(())
}

fn assert_prd_frame_is_ready(frame: &WebSurfaceFrame, case: &LiveSiteCase) {
    assert_eq!(
        frame.size(),
        WebSurfaceSize { width: LIVE_SURFACE_WIDTH, height: LIVE_SURFACE_HEIGHT },
        "{}",
        case.url
    );
    assert_eq!(frame.scroll_offset(), WebSurfaceScrollOffset::default(), "{}", case.url);
    assert!(frame.url_label().contains(normalized_url(case.url)), "{}", frame.url_label());
    assert!(frame.title_label().contains(case.title_fragment), "{}", frame.title_label());
    assert_eq!(frame.detail_label(), "934x657", "{}", case.url);
}

fn live_surface_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(
        point(px(0.0), px(0.0)),
        size(px(LIVE_SURFACE_WIDTH as f32), px(LIVE_SURFACE_HEIGHT as f32)),
    )
}

fn web_tab(url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), ProfileId::new(), "Web", UrlText::parse(url)?))
}

fn normalized_url(url: &str) -> &str {
    url.trim_end_matches('/')
}
