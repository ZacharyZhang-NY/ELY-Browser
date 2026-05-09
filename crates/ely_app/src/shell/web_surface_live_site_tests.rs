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
const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const LIVE_SITE_RENDER_ATTEMPTS: usize = 3;

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
    for case in cases {
        let frame = render_web_surface_frame(case)?;
        log_prd_frame("web-surface", &frame, case);
    }
    Ok(())
}

fn render_web_surface_frame(case: &LiveSiteCase) -> Result<WebSurfaceFrame, Box<dyn Error>> {
    let mut last_error = String::new();

    for attempt in 0..LIVE_SITE_RENDER_ATTEMPTS {
        let mut store = WebSurfaceStore::new();
        let tab = web_tab(case.url)?;
        assert!(store.record_viewport_size(tab.id(), live_surface_bounds()), "{}", case.url);
        let request = store
            .prepare_request(&tab, ProfileDataMode::Persistent)
            .ok_or_else(|| format!("missing web surface request for {}", case.url))?;
        let tab_id = request.tab_id.clone();
        let snapshot = request.client.snapshot(request.snapshot_request)?;
        let frame = WebSurfaceFrame::from_snapshot(
            request.requested_url,
            request.scroll_offset,
            request.zoom_percent,
            request.click_point,
            request.typed_text,
            snapshot,
        )?;

        match validate_prd_frame(&frame, case) {
            Ok(()) => {
                store.finish(tab_id, WebSurfaceState::Ready(frame.clone()));
                let Some(WebSurfaceState::Ready(stored_frame)) = store.state(tab.id()) else {
                    return Err(format!("web surface state is not ready for {}", case.url).into());
                };
                validate_prd_frame(stored_frame, case)?;
                return Ok(frame);
            }
            Err(error) => last_error = error,
        }

        if attempt + 1 < LIVE_SITE_RENDER_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    Err(last_error.into())
}

fn validate_prd_frame(frame: &WebSurfaceFrame, case: &LiveSiteCase) -> Result<(), String> {
    require(
        frame.size() == WebSurfaceSize { width: LIVE_SURFACE_WIDTH, height: LIVE_SURFACE_HEIGHT },
        format!("{} size: {:?}", case.url, frame.size()),
    )?;
    require(
        frame.scroll_offset() == WebSurfaceScrollOffset::default(),
        format!("{} scroll: {:?}", case.url, frame.scroll_offset()),
    )?;
    require_render_state_is_open(frame.render_state(), case.url)?;
    require(
        frame.url_label().contains(normalized_url(case.url)),
        format!("url: {}", frame.url_label()),
    )?;
    require(
        frame.title_label().contains(case.title_fragment),
        format!("title: {}", frame.title_label()),
    )?;
    require(
        frame.detail_label() == format!("{} 934x657", frame.render_state()),
        format!("{} detail: {}", case.url, frame.detail_label()),
    )?;
    require(frame.non_white_pixel_count() > 0, case.url.to_string())?;
    require(
        frame.content_pixel_count() >= MINIMUM_CONTENT_PIXELS,
        format!("{} content pixels: {}", case.url, frame.content_pixel_count()),
    )?;
    require(frame.sample_hash() > 0, case.url.to_string())
}

fn log_prd_frame(label: &str, frame: &WebSurfaceFrame, case: &LiveSiteCase) {
    eprintln!(
        "prd-live-site {label} url={} loaded={} title={} state={} size={}x{} content_pixels={} non_white_pixels={} sample_hash={}",
        case.url,
        frame.url_label(),
        frame.title_label(),
        frame.render_state(),
        frame.size().width,
        frame.size().height,
        frame.content_pixel_count(),
        frame.non_white_pixel_count(),
        frame.sample_hash()
    );
}

fn require_render_state_is_open(state: &str, url: &str) -> Result<(), String> {
    require(matches!(state, "complete" | "loading"), format!("{url} state: {state}"))
}

fn require(condition: bool, message: String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message) }
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
