use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use ely_domain::{DEFAULT_ZOOM_PERCENT, ProfileId, TabId, UrlText};
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, MouseHoverRequest, NavigationRequest, PageZoomRequest,
    PermissionDecision, PermissionRequest, RenderedFrame, ResizeRequest, ScrollRequest, ServoHost,
    ServoHostError, ServoSurfaceSize, SoftwareServoHost, WebViewSnapshot, WebViewState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::args::LiveArgs;

/// Per-`Ensure` budget the sidecar waits for Servo to paint a frame
/// after input dispatch. The original 60 ms was tuned for navigation
/// alone — too tight for click + paint round trips on the software
/// renderer. With 250 ms, a click dispatched into an already-loaded
/// page (the common case for input dispatch) paints within the same
/// `Ensure` so the user sees the page react instead of waiting for
/// the next 16 ms `Poll` from the GPUI shell.
const LIVE_FRAME_WAIT_TIMEOUT: Duration = Duration::from_millis(250);
const LIVE_FRAME_WAIT_INTERVAL: Duration = Duration::from_millis(2);

pub(super) fn run_live(args: LiveArgs) -> Result<(), LiveSidecarError> {
    fs::create_dir_all(&args.profile_data_dir)?;
    let mut host = SoftwareServoHost::new_with_config_dir(
        ServoSurfaceSize::new(1, 1),
        Some(args.profile_data_dir),
    )?;
    let mut sessions = HashMap::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<LiveRequest>(&line) {
            Ok(request) => handle_request(&mut host, &mut sessions, request),
            Err(error) => Err(LiveSidecarError::Json(error)),
        };
        write_response(&mut stdout, response)?;
    }

    Ok(())
}

fn handle_request(
    host: &mut SoftwareServoHost,
    sessions: &mut HashMap<String, LiveSession>,
    request: LiveRequest,
) -> Result<LiveResponse, LiveSidecarError> {
    match request {
        LiveRequest::Ensure {
            tab_id,
            profile_id,
            url,
            width,
            height,
            page_zoom_percent,
            scroll_delta_x,
            scroll_delta_y,
            click_x,
            click_y,
            hover_x,
            hover_y,
            typed_text,
            site_permissions,
            rgba_out,
        } => {
            let tab = TabId::parse(tab_id.clone())?;
            let profile = ProfileId::parse(profile_id)?;
            let url = UrlText::parse(url)?;
            let session = ensure_session(host, sessions, tab_id, &tab, &profile, width, height)?;

            if apply_layout(host, session, width, height, page_zoom_percent)? {
                session.awaiting_visible_frame = true;
            }
            apply_permissions(host, session, &profile, site_permissions)?;
            if session.requested_url != url.as_str() {
                host.navigate(NavigationRequest {
                    webview_id: session.webview_id.clone(),
                    tab_id: tab,
                    url: url.clone(),
                })?;
                session.requested_url = url.as_str().to_string();
                session.scroll_x = 0;
                session.scroll_y = 0;
                session.awaiting_visible_frame = true;
            }
            if apply_input(
                host,
                session,
                scroll_delta_x,
                scroll_delta_y,
                click_x,
                click_y,
                hover_x,
                hover_y,
                typed_text,
            )? {
                session.awaiting_visible_frame = true;
            }
            poll_frame(host, session, rgba_out.into())
        }
        LiveRequest::Poll { tab_id, rgba_out } => {
            let Some(session) = sessions.get_mut(&tab_id) else {
                return Ok(LiveResponse::empty());
            };
            poll_frame(host, session, rgba_out.into())
        }
    }
}

fn write_response(
    stdout: &mut impl Write,
    response: Result<LiveResponse, LiveSidecarError>,
) -> Result<(), LiveSidecarError> {
    let response = match response {
        Ok(response) => response,
        Err(error) => LiveResponse::error(error.to_string()),
    };
    serde_json::to_writer(&mut *stdout, &response)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn ensure_session<'a>(
    host: &mut SoftwareServoHost,
    sessions: &'a mut HashMap<String, LiveSession>,
    key: String,
    tab_id: &TabId,
    profile_id: &ProfileId,
    width: u32,
    height: u32,
) -> Result<&'a mut LiveSession, LiveSidecarError> {
    if !sessions.contains_key(&key) {
        let webview_id = host.create_webview_with_size(
            tab_id.clone(),
            profile_id.clone(),
            ServoSurfaceSize::new(width, height),
        )?;
        sessions.insert(key.clone(), LiveSession::new(webview_id, width, height));
    }

    sessions.get_mut(&key).ok_or(LiveSidecarError::SessionUnavailable)
}

fn apply_layout(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    if session.width != width || session.height != height {
        host.resize(ResizeRequest { webview_id: session.webview_id.clone(), width, height })?;
        session.width = width;
        session.height = height;
        changed = true;
    }

    if session.page_zoom_percent != page_zoom_percent {
        host.set_page_zoom(PageZoomRequest {
            webview_id: session.webview_id.clone(),
            zoom_factor: f32::from(page_zoom_percent) / 100.0,
        })?;
        session.page_zoom_percent = page_zoom_percent;
        changed = true;
    }

    Ok(changed)
}

fn apply_permissions(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    profile_id: &ProfileId,
    permissions: Vec<LiveSitePermission>,
) -> Result<(), LiveSidecarError> {
    for permission in permissions {
        host.set_permission(
            PermissionRequest {
                webview_id: session.webview_id.clone(),
                profile_id: profile_id.clone(),
                origin: ely_domain::SiteOrigin::parse(permission.origin)?,
                feature: ely_domain::SitePermissionFeature::parse(permission.feature.as_str())?,
            },
            PermissionDecision::from(ely_domain::SitePermissionDecision::parse(
                permission.decision.as_str(),
            )?),
        )?;
    }

    Ok(())
}

fn apply_input(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    scroll_delta_x: i32,
    scroll_delta_y: i32,
    click_x: Option<u32>,
    click_y: Option<u32>,
    hover_x: Option<u32>,
    hover_y: Option<u32>,
    typed_text: Option<String>,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    if scroll_delta_x != 0 || scroll_delta_y != 0 {
        host.scroll(ScrollRequest {
            webview_id: session.webview_id.clone(),
            delta_x: scroll_delta_x,
            delta_y: scroll_delta_y,
        })?;
        session.scroll_x = positive_scroll_component(session.scroll_x, scroll_delta_x);
        session.scroll_y = positive_scroll_component(session.scroll_y, scroll_delta_y);
        changed = true;
    }

    if let (Some(x), Some(y)) = (hover_x, hover_y) {
        host.hover(MouseHoverRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let (Some(x), Some(y)) = (click_x, click_y) {
        host.click(MouseClickRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let Some(text) = typed_text {
        host.type_text(KeyboardTextRequest { webview_id: session.webview_id.clone(), text })?;
        changed = true;
    }

    Ok(changed)
}

fn poll_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rgba_out: PathBuf,
) -> Result<LiveResponse, LiveSidecarError> {
    let started_at = Instant::now();
    let mut latest_frame = None;

    loop {
        host.tick();
        let snapshot = host.snapshot(&session.webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(&session.webview_id)?;
            let snapshot = host.snapshot(&session.webview_id)?;
            let frame = host.last_rendered_frame()?;
            let has_visible_content =
                frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0;
            fs::write(&rgba_out, frame.rgba_bytes())?;
            let response =
                LiveResponse::frame(LiveFrameReport::new(&snapshot, &frame, rgba_out.clone()));
            if has_visible_content {
                session.awaiting_visible_frame = false;
                return Ok(response);
            }
            if !session.awaiting_visible_frame {
                return Ok(response);
            }
            latest_frame = Some(response);
        }

        if !session.awaiting_visible_frame {
            return Ok(LiveResponse::empty());
        }
        if started_at.elapsed() >= LIVE_FRAME_WAIT_TIMEOUT {
            return Ok(latest_frame.unwrap_or_else(LiveResponse::empty));
        }

        thread::sleep(LIVE_FRAME_WAIT_INTERVAL);
    }
}

#[derive(Clone)]
struct LiveSession {
    webview_id: ely_domain::WebViewId,
    requested_url: String,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    scroll_x: i32,
    scroll_y: i32,
    awaiting_visible_frame: bool,
}

impl LiveSession {
    fn new(webview_id: ely_domain::WebViewId, width: u32, height: u32) -> Self {
        Self {
            webview_id,
            requested_url: String::new(),
            width: width.max(1),
            height: height.max(1),
            page_zoom_percent: DEFAULT_ZOOM_PERCENT,
            scroll_x: 0,
            scroll_y: 0,
            awaiting_visible_frame: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LiveRequest {
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        click_x: Option<u32>,
        click_y: Option<u32>,
        #[serde(default)]
        hover_x: Option<u32>,
        #[serde(default)]
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permissions: Vec<LiveSitePermission>,
        rgba_out: String,
    },
    Poll {
        tab_id: String,
        rgba_out: String,
    },
}

#[derive(Deserialize)]
struct LiveSitePermission {
    origin: String,
    feature: String,
    decision: String,
}

#[derive(Serialize)]
struct LiveResponse {
    error: Option<String>,
    frame: Option<LiveFrameReport>,
}

impl LiveResponse {
    fn empty() -> Self {
        Self { error: None, frame: None }
    }

    fn frame(frame: LiveFrameReport) -> Self {
        Self { error: None, frame: Some(frame) }
    }

    fn error(message: String) -> Self {
        Self { error: Some(message), frame: None }
    }
}

#[derive(Serialize)]
struct LiveFrameReport {
    loaded_url: Option<String>,
    title: Option<String>,
    state: &'static str,
    width: u32,
    height: u32,
    rgba_path: PathBuf,
    rgba_byte_count: usize,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    sample_hash: u64,
}

impl LiveFrameReport {
    fn new(snapshot: &WebViewSnapshot, frame: &RenderedFrame, rgba_path: PathBuf) -> Self {
        Self {
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            rgba_path,
            rgba_byte_count: frame.rgba_bytes().len(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum LiveSidecarError {
    #[error("live session is unavailable after creation")]
    SessionUnavailable,

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),

    #[error(transparent)]
    Host(#[from] ServoHostError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn state_label(state: &WebViewState) -> &'static str {
    match state {
        WebViewState::Created => "created",
        WebViewState::Loading => "loading",
        WebViewState::Complete => "complete",
        WebViewState::Sleeping => "sleeping",
        WebViewState::Crashed => "crashed",
    }
}

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    value.clamp(0, i64::from(i32::MAX)) as i32
}
