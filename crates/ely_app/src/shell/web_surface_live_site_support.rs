use super::*;

#[derive(Clone, Copy)]
pub(super) struct ExpectedCssViewport {
    pub(super) physical_width: u32,
    pub(super) physical_height: u32,
    pub(super) css_width: u32,
    pub(super) css_height: u32,
    pub(super) dpr_percent: u16,
}

pub(super) fn validate_prd_frame(
    frame: &WebSurfaceFrame,
    case: &LiveSiteCase,
    expected_scroll_y: i32,
) -> Result<(), String> {
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
        frame.scroll_offset().y() == expected_scroll_y,
        format!("{} scroll: {:?}", case.url, frame.scroll_offset()),
    )?;
    require(
        frame.css_viewport_size() == (LIVE_SURFACE_WIDTH, LIVE_SURFACE_HEIGHT),
        format!("{} CSS viewport: {:?}", case.url, frame.css_viewport_size()),
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
    let expected_detail = if expected_scroll_y == 0 {
        format!("{} {}x{}", frame.render_state(), LIVE_SURFACE_WIDTH, LIVE_SURFACE_HEIGHT)
    } else {
        format!(
            "{} {}x{} y={expected_scroll_y}",
            frame.render_state(),
            LIVE_SURFACE_WIDTH,
            LIVE_SURFACE_HEIGHT,
        )
    };
    require(
        frame.detail_label() == expected_detail,
        format!("{} detail: {}", case.url, frame.detail_label()),
    )?;
    require(
        frame.non_white_pixel_count() > 0,
        format!("{} non-white pixels: {}", case.url, frame.non_white_pixel_count()),
    )?;
    require(
        frame.content_pixel_count() >= MINIMUM_CONTENT_PIXELS,
        format!("{} content pixels: {}", case.url, frame.content_pixel_count()),
    )?;
    require(frame.sample_hash() > 0, format!("{} sample hash: {}", case.url, frame.sample_hash()))
}

pub(super) fn validate_prd_frame_at_size(
    frame: &WebSurfaceFrame,
    case: &LiveSiteCase,
    expected_width: u32,
    expected_height: u32,
) -> Result<(), String> {
    require(
        frame.size()
            == WebSurfaceSize {
                width: expected_width,
                height: expected_height,
                device_pixel_ratio_percent: 100,
            },
        format!("{} size: {:?}", case.url, frame.size()),
    )?;
    require_render_state_is_open(frame.render_state(), case.url)?;
    require(
        frame.css_viewport_size() == (expected_width, expected_height),
        format!("{} CSS viewport: {:?}", case.url, frame.css_viewport_size()),
    )?;
    require(
        frame.url_label().contains(normalized_url(case.url)),
        format!("url: {}", frame.url_label()),
    )?;
    require(
        frame.title_label().contains(case.title_fragment),
        format!("title: {}", frame.title_label()),
    )?;
    require(
        frame.non_white_pixel_count() > 0,
        format!("{} non-white pixels: {}", case.url, frame.non_white_pixel_count()),
    )?;
    require(
        frame.content_pixel_count() >= MINIMUM_CONTENT_PIXELS,
        format!("{} content pixels: {}", case.url, frame.content_pixel_count()),
    )?;
    require(frame.sample_hash() > 0, format!("{} sample hash: {}", case.url, frame.sample_hash()))
}

pub(super) fn validate_prd_frame_at_css_size(
    frame: &WebSurfaceFrame,
    case: &LiveSiteCase,
    expected: ExpectedCssViewport,
) -> Result<(), String> {
    require(
        frame.size()
            == WebSurfaceSize {
                width: expected.physical_width,
                height: expected.physical_height,
                device_pixel_ratio_percent: expected.dpr_percent,
            },
        format!("{} size: {:?}", case.url, frame.size()),
    )?;
    require_render_state_is_open(frame.render_state(), case.url)?;
    require(
        frame.css_viewport_size() == (expected.css_width, expected.css_height),
        format!("{} CSS viewport: {:?}", case.url, frame.css_viewport_size()),
    )?;
    require(
        frame.url_label().contains(normalized_url(case.url)),
        format!("url: {}", frame.url_label()),
    )?;
    require(
        frame.title_label().contains(case.title_fragment),
        format!("title: {}", frame.title_label()),
    )?;
    Ok(())
}

pub(super) fn log_prd_frame(label: &str, frame: &WebSurfaceFrame, case: &LiveSiteCase) {
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

pub(super) fn require_render_state_is_open(state: &str, url: &str) -> Result<(), String> {
    require(matches!(state, "complete" | "loading"), format!("{url} state: {state}"))
}

pub(super) fn require(condition: bool, message: String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message) }
}

pub(super) fn live_surface_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(
        point(px(0.0), px(0.0)),
        size(px(LIVE_SURFACE_WIDTH as f32), px(LIVE_SURFACE_HEIGHT as f32)),
    )
}

pub(super) fn resized_live_surface_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(
        point(px(0.0), px(0.0)),
        size(px(RESIZED_LIVE_SURFACE_WIDTH as f32), px(RESIZED_LIVE_SURFACE_HEIGHT as f32)),
    )
}

pub(super) fn live_scroll_point() -> gpui::Point<gpui::Pixels> {
    point(px(LIVE_SITE_SCROLL_POINT_X), px(LIVE_SITE_SCROLL_POINT_Y))
}

pub(super) fn web_tab(profile_id: ProfileId, url: &str) -> Result<BrowserTab, Box<dyn Error>> {
    Ok(BrowserTab::new(TabId::new(), SpaceId::new(), profile_id, "Web", UrlText::parse(url)?))
}

pub(super) fn normalized_url(url: &str) -> &str {
    url.trim_end_matches('/')
}
