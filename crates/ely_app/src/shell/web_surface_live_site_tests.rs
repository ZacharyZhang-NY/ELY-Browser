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
        web_surface_geometry::WebSurfaceSize,
        web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
    },
};

use super::WebSurfaceStore;

const LIVE_SURFACE_WIDTH: u32 = 934;
const LIVE_SURFACE_HEIGHT: u32 = 657;
const RESIZED_LIVE_SURFACE_WIDTH: u32 = LIVE_SURFACE_WIDTH + 12;
const RESIZED_LIVE_SURFACE_HEIGHT: u32 = LIVE_SURFACE_HEIGHT + 20;
const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const LIVE_SITE_RENDER_ATTEMPTS: usize = 3;
const LIVE_SITE_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const LIVE_SITE_WAIT_INTERVAL: Duration = Duration::from_millis(2);
const LIVE_SITE_CHILD_ENV: &str = "ELY_APP_WEB_SURFACE_LIVE_CHILD";
const LIVE_SITE_SCROLL_DOWN_Y: i32 = 360;
const LIVE_SITE_SCROLL_UP_Y: i32 = -240;
const LIVE_SITE_SCROLL_POINT_X: f32 = 320.0;
const LIVE_SITE_SCROLL_POINT_Y: f32 = 320.0;
const RETINA_SCALE_FACTOR: f32 = 2.0;

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

#[test]
fn web_surface_scrolls_prd_site_down_and_up() -> Result<(), Box<dyn Error>> {
    run_isolated_live_site_test("web_surface_scrolls_prd_site_down_and_up", || {
        assert_web_surface_scrolls_prd_site()
    })
}

#[test]
fn web_surface_resizes_prd_site_without_failed_state() -> Result<(), Box<dyn Error>> {
    run_isolated_live_site_test("web_surface_resizes_prd_site_without_failed_state", || {
        assert_web_surface_resizes_prd_site()
    })
}

#[test]
fn web_surface_reports_css_viewport_at_retina_scale() -> Result<(), Box<dyn Error>> {
    run_isolated_live_site_test("web_surface_reports_css_viewport_at_retina_scale", || {
        assert_web_surface_reports_retina_css_viewport()
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

fn assert_web_surface_scrolls_prd_site() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let profile_id = ProfileId::new();
    let case = PRD_TOP_SITE_CASES
        .iter()
        .find(|case| case.url == "https://github.com")
        .ok_or("missing github.com live-site case")?;
    let tab = web_tab(profile_id, case.url)?;

    assert_eq!(
        store.record_viewport_size(tab.id(), live_surface_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    let initial = wait_for_ready_frame_at_scroll(&mut store, tab.id(), case, 0, None)?;

    assert_eq!(
        store.record_scroll_delta(
            tab.id(),
            case.url,
            point(px(0.0), px(LIVE_SITE_SCROLL_DOWN_Y as f32)),
            live_scroll_point(),
            1.0,
        ),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    let down = wait_for_ready_frame_at_scroll(
        &mut store,
        tab.id(),
        case,
        LIVE_SITE_SCROLL_DOWN_Y,
        Some(initial.sample_hash()),
    )?;

    assert_eq!(
        store.record_scroll_delta(
            tab.id(),
            case.url,
            point(px(0.0), px(LIVE_SITE_SCROLL_UP_Y as f32)),
            live_scroll_point(),
            1.0,
        ),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    wait_for_ready_frame_at_scroll(
        &mut store,
        tab.id(),
        case,
        LIVE_SITE_SCROLL_DOWN_Y + LIVE_SITE_SCROLL_UP_Y,
        Some(down.sample_hash()),
    )?;
    store.close_surface(tab.id());
    Ok(())
}

fn assert_web_surface_resizes_prd_site() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let profile_id = ProfileId::new();
    let case = PRD_TOP_SITE_CASES
        .iter()
        .find(|case| case.url == "https://servo.org/")
        .ok_or("missing servo.org live-site case")?;
    let tab = web_tab(profile_id, case.url)?;

    assert_eq!(
        store.record_viewport_size(tab.id(), live_surface_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    let _ = wait_for_ready_frame_at_size(
        &mut store,
        &tab,
        case,
        LIVE_SURFACE_WIDTH,
        LIVE_SURFACE_HEIGHT,
    )?;

    assert_eq!(
        store.record_viewport_size(tab.id(), resized_live_surface_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    wait_for_ready_frame_at_size(
        &mut store,
        &tab,
        case,
        RESIZED_LIVE_SURFACE_WIDTH,
        RESIZED_LIVE_SURFACE_HEIGHT,
    )?;
    store.close_surface(tab.id());
    Ok(())
}

fn assert_web_surface_reports_retina_css_viewport() -> Result<(), Box<dyn Error>> {
    let mut store = WebSurfaceStore::new();
    let profile_id = ProfileId::new();
    let case = PRD_TOP_SITE_CASES
        .iter()
        .find(|case| case.url == "https://servo.org/")
        .ok_or("missing servo.org live-site case")?;
    let tab = web_tab(profile_id, case.url)?;

    assert_eq!(
        store.record_viewport_size(tab.id(), live_surface_bounds(), RETINA_SCALE_FACTOR),
        WebSurfaceInputOutcome::Applied,
        "{}",
        case.url,
    );
    store.ensure_surface(&tab, ProfileDataMode::Transient, &[]);
    wait_for_ready_frame_at_css_size(
        &mut store,
        tab.id(),
        case,
        ExpectedCssViewport {
            physical_width: LIVE_SURFACE_WIDTH * RETINA_SCALE_FACTOR as u32,
            physical_height: LIVE_SURFACE_HEIGHT * RETINA_SCALE_FACTOR as u32,
            css_width: LIVE_SURFACE_WIDTH,
            css_height: LIVE_SURFACE_HEIGHT,
            dpr_percent: (RETINA_SCALE_FACTOR * 100.0).round() as u16,
        },
    )?;
    store.close_surface(tab.id());
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

        let result = wait_for_ready_frame(store, tab.id(), case);
        store.close_surface(tab.id());

        match result {
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
    let mut last_error = None;

    loop {
        store.tick(std::slice::from_ref(tab_id));
        match store.state(tab_id) {
            Some(WebSurfaceState::Ready(frame)) => {
                if let Err(error) = validate_prd_frame(frame, case, 0) {
                    last_error = Some(error);
                } else {
                    return Ok(frame.clone());
                }
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                return Err(format!("{} failed: {message}", case.url));
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        if started_at.elapsed() >= LIVE_SITE_WAIT_TIMEOUT {
            return Err(last_error.unwrap_or_else(|| format!("timed out rendering {}", case.url)));
        }

        thread::sleep(LIVE_SITE_WAIT_INTERVAL);
    }
}

fn wait_for_ready_frame_at_scroll(
    store: &mut WebSurfaceStore,
    tab_id: &TabId,
    case: &LiveSiteCase,
    expected_scroll_y: i32,
    previous_sample_hash: Option<u64>,
) -> Result<WebSurfaceFrame, String> {
    let started_at = Instant::now();
    let mut last_error = None;

    loop {
        store.tick(std::slice::from_ref(tab_id));
        match store.state(tab_id) {
            Some(WebSurfaceState::Ready(frame))
                if frame.scroll_offset().y() == expected_scroll_y =>
            {
                if let Err(error) = validate_prd_frame(frame, case, expected_scroll_y) {
                    last_error = Some(error);
                } else if !frame.has_hardware_surface()
                    && let Some(previous_sample_hash) = previous_sample_hash
                    && frame.sample_hash() == previous_sample_hash
                {
                    last_error = Some(format!(
                        "{} scroll y={expected_scroll_y} sample hash unchanged",
                        case.url
                    ));
                } else {
                    return Ok(frame.clone());
                }
            }
            Some(WebSurfaceState::Ready(frame)) => {
                last_error = Some(format!(
                    "{} expected scroll y={expected_scroll_y}, observed {}",
                    case.url,
                    frame.detail_label()
                ));
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                return Err(format!("{} failed: {message}", case.url));
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        if started_at.elapsed() >= LIVE_SITE_WAIT_TIMEOUT {
            return Err(last_error.unwrap_or_else(|| {
                format!("timed out rendering {} at scroll y={expected_scroll_y}", case.url)
            }));
        }

        thread::sleep(LIVE_SITE_WAIT_INTERVAL);
    }
}

fn wait_for_ready_frame_at_size(
    store: &mut WebSurfaceStore,
    tab: &BrowserTab,
    case: &LiveSiteCase,
    expected_width: u32,
    expected_height: u32,
) -> Result<WebSurfaceFrame, String> {
    let started_at = Instant::now();
    let mut last_error = None;

    loop {
        store.ensure_surface(tab, ProfileDataMode::Transient, &[]);
        store.tick(std::slice::from_ref(tab.id()));
        match store.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame))
                if frame.size().width == expected_width
                    && frame.size().height == expected_height =>
            {
                if let Err(error) =
                    validate_prd_frame_at_size(frame, case, expected_width, expected_height)
                {
                    last_error = Some(error);
                } else {
                    return Ok(frame.clone());
                }
            }
            Some(WebSurfaceState::Ready(_)) => {}
            Some(WebSurfaceState::Failed { message, .. }) => {
                return Err(format!("{} failed: {message}", case.url));
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        if started_at.elapsed() >= LIVE_SITE_WAIT_TIMEOUT {
            return Err(last_error.unwrap_or_else(|| {
                format!("timed out rendering {} at {expected_width}x{expected_height}", case.url)
            }));
        }

        thread::sleep(LIVE_SITE_WAIT_INTERVAL);
    }
}

fn wait_for_ready_frame_at_css_size(
    store: &mut WebSurfaceStore,
    tab_id: &TabId,
    case: &LiveSiteCase,
    expected: ExpectedCssViewport,
) -> Result<WebSurfaceFrame, String> {
    let started_at = Instant::now();
    let mut last_error = None;

    loop {
        store.tick(std::slice::from_ref(tab_id));
        match store.state(tab_id) {
            Some(WebSurfaceState::Ready(frame))
                if frame.size().width == expected.physical_width
                    && frame.size().height == expected.physical_height =>
            {
                if let Err(error) = validate_prd_frame_at_css_size(frame, case, expected) {
                    last_error = Some(error);
                } else {
                    return Ok(frame.clone());
                }
            }
            Some(WebSurfaceState::Ready(_)) => {}
            Some(WebSurfaceState::Failed { message, .. }) => {
                return Err(format!("{} failed: {message}", case.url));
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        if started_at.elapsed() >= LIVE_SITE_WAIT_TIMEOUT {
            return Err(last_error.unwrap_or_else(|| {
                format!(
                    "timed out rendering {} at {}x{} css {}x{}",
                    case.url,
                    expected.physical_width,
                    expected.physical_height,
                    expected.css_width,
                    expected.css_height,
                )
            }));
        }

        thread::sleep(LIVE_SITE_WAIT_INTERVAL);
    }
}


#[path = "web_surface_live_site_support.rs"]
mod web_surface_live_site_support;
use web_surface_live_site_support::*;
