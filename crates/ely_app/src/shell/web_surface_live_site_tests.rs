use std::{
    env,
    error::Error,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use crate::{
    services::{
        ProfileDataMode,
        prd_live_sites::{
            LiveSiteCase, PRD_REFERENCE_SITE_CASES, PRD_TOP_SITE_CASES,
            assert_prd_reference_urls_are_covered,
        },
    },
    shell::{
        web_surface_frame::WebSurfaceFrame,
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
    },
};

use super::WebSurfaceStore;

const LIVE_SURFACE_WIDTH: u32 = 934;
const LIVE_SURFACE_HEIGHT: u32 = 657;
const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const LIVE_SITE_RENDER_ATTEMPTS: usize = 3;
const LIVE_SITE_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const LIVE_SITE_WAIT_INTERVAL: Duration = Duration::from_millis(2);
const LIVE_SITE_CHILD_ENV: &str = "ELY_APP_WEB_SURFACE_LIVE_CHILD";

#[test]
fn web_surface_cases_cover_prd_reference_urls() -> Result<(), Box<dyn Error>> {
    assert_prd_reference_urls_are_covered()
}

#[test]
fn web_surface_opens_and_renders_prd_top_sites() -> Result<(), Box<dyn Error>> {
    run_isolated_live_site_test("web_surface_opens_and_renders_prd_top_sites", || {
        assert_web_surfaces_render(PRD_TOP_SITE_CASES)
    })
}

#[test]
fn web_surface_opens_and_renders_prd_reference_sites() -> Result<(), Box<dyn Error>> {
    run_isolated_live_site_test("web_surface_opens_and_renders_prd_reference_sites", || {
        assert_web_surfaces_render(PRD_REFERENCE_SITE_CASES)
    })
}

fn run_isolated_live_site_test(
    test_name: &str,
    test: impl FnOnce() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    if env::var_os(LIVE_SITE_CHILD_ENV).is_some() {
        return test();
    }

    let output = Command::new(env::current_exe()?)
        .arg(test_name)
        .env(LIVE_SITE_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "isolated web surface live-site test failed\nstatus: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn assert_web_surfaces_render(cases: &[LiveSiteCase]) -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let profile_id = ProfileId::new();

    for case in cases {
        let frame = render_web_surface_frame(&mut store, &profile_id, case)?;
        log_prd_frame("web-surface", &frame, case);
    }
    Ok(())
}

fn render_web_surface_frame(
    store: &mut WebSurfaceStore,
    profile_id: &ProfileId,
    case: &LiveSiteCase,
) -> Result<WebSurfaceFrame, Box<dyn Error>> {
    let mut last_error = String::new();

    for attempt in 0..LIVE_SITE_RENDER_ATTEMPTS {
        let tab = web_tab(profile_id.clone(), case.url)?;
        assert_eq!(
            store.record_viewport_size(tab.id(), live_surface_bounds(), 1.0),
            WebSurfaceInputOutcome::Applied,
            "{}",
            case.url,
        );
        store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);

        match wait_for_ready_frame(store, tab.id(), case) {
            Ok(frame) => return Ok(frame),
            Err(error) => last_error = error,
        }

        if attempt + 1 < LIVE_SITE_RENDER_ATTEMPTS {
            thread::sleep(Duration::from_millis(250));
        }
    }

    Err(last_error.into())
}

fn wait_for_ready_frame(
    store: &mut WebSurfaceStore,
    tab_id: &TabId,
    case: &LiveSiteCase,
) -> Result<WebSurfaceFrame, String> {
    let started_at = Instant::now();

    loop {
        if started_at.elapsed() >= LIVE_SITE_WAIT_TIMEOUT {
            return Err(format!("timed out rendering {}", case.url));
        }

        store.tick(std::slice::from_ref(tab_id));
        match store.state(tab_id) {
            Some(WebSurfaceState::Ready(frame)) => {
                validate_prd_frame(frame, case)?;
                return Ok(frame.clone());
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                return Err(format!("{} failed: {message}", case.url));
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        thread::sleep(LIVE_SITE_WAIT_INTERVAL);
    }
}

fn validate_prd_frame(frame: &WebSurfaceFrame, case: &LiveSiteCase) -> Result<(), String> {
    require(
        frame.size()
            == WebSurfaceSize {
                width: LIVE_SURFACE_WIDTH,
                height: LIVE_SURFACE_HEIGHT,
                device_pixel_ratio_percent: 100,
            },
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
    if frame.has_hardware_surface() {
        return Ok(());
    }
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

fn web_tab(profile_id: ProfileId, url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), profile_id, "Web", UrlText::parse(url)?))
}

fn normalized_url(url: &str) -> &str {
    url.trim_end_matches('/')
}
