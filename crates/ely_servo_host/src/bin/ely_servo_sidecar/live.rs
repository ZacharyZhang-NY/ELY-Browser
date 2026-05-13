use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead},
    thread,
    time::{Duration, Instant},
};

use ely_domain::{DEFAULT_ZOOM_PERCENT, ProfileId, TabId, UrlText};
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, MouseHoverRequest, NavigationRequest, PageZoomRequest,
    PermissionDecision, PermissionRequest, RenderingContextKind, ResizeRequest, ScrollRequest,
    ServoHost, ServoSurfaceSize, SoftwareServoHost,
};

use super::args::LiveArgs;
use super::live_output::{populate_surface_fields, write_outcome};
pub(super) use super::live_protocol::LiveSidecarError;
use super::live_protocol::{
    LiveFrameReport, LiveOutcome, LiveRequest, LiveSitePermission, PartialFrameTimings,
};
use super::perf::{FramePerfAggregator, FramePerfSummary, elapsed_ns};

/// Per-`Ensure` budget for Servo to paint after input dispatch.
/// 250 ms catches the common click + paint round trip within the
/// same `Ensure` instead of waiting for the next 16 ms `Poll`.
const LIVE_FRAME_WAIT_TIMEOUT: Duration = Duration::from_millis(250);
const LIVE_FRAME_WAIT_INTERVAL: Duration = Duration::from_millis(2);

pub(super) fn run_live(args: LiveArgs) -> Result<(), LiveSidecarError> {
    fs::create_dir_all(&args.profile_data_dir)?;
    let rendering_context_kind = args.rendering_context_kind;
    let context_label = rendering_context_label(rendering_context_kind);
    let mut host = SoftwareServoHost::new_with_config_dir_and_kind(
        ServoSurfaceSize::new(1, 1),
        Some(args.profile_data_dir),
        rendering_context_kind,
    )?;
    let mut sessions = HashMap::new();
    let mut perf =
        FramePerfAggregator::new(context_label, FramePerfAggregator::DEFAULT_WINDOW_SIZE);
    let mut pending_summary: Option<FramePerfSummary> = None;
    let mut published_surface_ids: HashMap<String, HashSet<u64>> = HashMap::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // `frame_started_at` is the honest start of the end-to-end
        // frame: a request just arrived and we're about to do
        // everything required to put bytes back on the pipe. The
        // matching stop is the `stdout.flush()` inside
        // `write_outcome`.
        let frame_started_at = Instant::now();
        let outcome = match serde_json::from_str::<LiveRequest>(&line) {
            Ok(request) => handle_request(
                &mut host,
                &mut sessions,
                &mut published_surface_ids,
                rendering_context_kind,
                request,
            ),
            Err(error) => Err(LiveSidecarError::Json(error)),
        };
        write_outcome(&mut stdout, &mut perf, &mut pending_summary, outcome, frame_started_at)?;
    }

    Ok(())
}

const fn rendering_context_label(kind: RenderingContextKind) -> &'static str {
    match kind {
        RenderingContextKind::Software => "software",
        RenderingContextKind::Hardware => "hardware",
    }
}

fn handle_request(
    host: &mut SoftwareServoHost,
    sessions: &mut HashMap<String, LiveSession>,
    published_surface_ids: &mut HashMap<String, HashSet<u64>>,
    rendering_context_kind: RenderingContextKind,
    request: LiveRequest,
) -> Result<LiveOutcome, LiveSidecarError> {
    match request {
        LiveRequest::Ensure {
            tab_id,
            profile_id,
            url,
            width,
            height,
            page_zoom_percent,
            device_pixel_ratio,
            scroll_delta_x,
            scroll_delta_y,
            scroll_point_x,
            scroll_point_y,
            click_x,
            click_y,
            hover_x,
            hover_y,
            typed_text,
            site_permissions,
        } => {
            let tab = TabId::parse(tab_id.clone())?;
            let profile = ProfileId::parse(profile_id)?;
            let url = UrlText::parse(url)?;
            let session =
                ensure_session(host, sessions, tab_id.clone(), &tab, &profile, width, height)?;

            if apply_layout(host, session, width, height, page_zoom_percent, device_pixel_ratio)? {
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
                // New URL: the previous tab's pixels are no longer
                // valid evidence that "we have visible content"; let
                // the gate skip blank loading frames again.
                session.ever_visible_frame = false;
            }
            let input = LiveInput {
                scroll_delta_x,
                scroll_delta_y,
                scroll_point_x,
                scroll_point_y,
                click_x,
                click_y,
                hover_x,
                hover_y,
                typed_text,
            };
            if apply_input(host, session, input)? {
                // Tell poll_frame to actually wait for Servo to paint
                // a response to this input. The visible-content gate
                // is bypassed on the hardware path inside poll_frame,
                // so we return on the first `has_pending_frame=true`
                // (~3 ms in practice) rather than burning the full
                // LIVE_FRAME_WAIT_TIMEOUT.
                session.awaiting_visible_frame = true;
            }
            let webview_id = session.webview_id.clone();
            let mut outcome = poll_frame(host, session, rendering_context_kind)?;
            populate_surface_fields(
                host,
                &webview_id,
                &tab_id,
                published_surface_ids,
                &mut outcome,
            );
            Ok(outcome)
        }
        LiveRequest::Poll { tab_id } => {
            let Some(session) = sessions.get_mut(&tab_id) else {
                return Ok(LiveOutcome::empty());
            };
            let webview_id = session.webview_id.clone();
            let mut outcome = poll_frame(host, session, rendering_context_kind)?;
            populate_surface_fields(
                host,
                &webview_id,
                &tab_id,
                published_surface_ids,
                &mut outcome,
            );
            Ok(outcome)
        }
    }
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
    device_pixel_ratio: f32,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    // Push the device pixel ratio BEFORE resize. Servo's WebView
    // defaults hidpi to 1.0; without this the first layout treats
    // physical-pixel viewport widths as CSS-pixel widths and the page
    // lays out half the size you'd expect on a Retina display. The
    // sidecar-side `LiveSession::hidpi_scale_milli` keeps a u32 of
    // (scale × 1000) so f32 jitter from JSON parsing doesn't churn
    // the setter every frame.
    let hidpi_scale_milli = encode_hidpi_scale_milli(device_pixel_ratio);
    if session.hidpi_scale_milli != hidpi_scale_milli {
        host.set_hidpi_scale(ely_servo_host::HidpiScaleRequest {
            webview_id: session.webview_id.clone(),
            scale_factor: hidpi_scale_milli_to_f32(hidpi_scale_milli),
        })?;
        session.hidpi_scale_milli = hidpi_scale_milli;
        changed = true;
    }

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

fn encode_hidpi_scale_milli(scale: f32) -> u32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1_000;
    }
    let scaled = (scale * 1_000.0).round();
    scaled.clamp(500.0, 5_000.0) as u32
}

fn hidpi_scale_milli_to_f32(milli: u32) -> f32 {
    milli as f32 / 1_000.0
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
    input: LiveInput,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    if input.scroll_delta_x != 0 || input.scroll_delta_y != 0 {
        let (point_x, point_y) = input.scroll_point()?;
        host.scroll(ScrollRequest {
            webview_id: session.webview_id.clone(),
            delta_x: input.scroll_delta_x,
            delta_y: input.scroll_delta_y,
            point_x,
            point_y,
        })?;
        session.scroll_x = positive_scroll_component(session.scroll_x, input.scroll_delta_x);
        session.scroll_y = positive_scroll_component(session.scroll_y, input.scroll_delta_y);
        changed = true;
    }

    if let (Some(x), Some(y)) = (input.hover_x, input.hover_y) {
        host.hover(MouseHoverRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let (Some(x), Some(y)) = (input.click_x, input.click_y) {
        host.click(MouseClickRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let Some(text) = input.typed_text {
        host.type_text(KeyboardTextRequest { webview_id: session.webview_id.clone(), text })?;
        changed = true;
    }

    Ok(changed)
}

struct LiveInput {
    scroll_delta_x: i32,
    scroll_delta_y: i32,
    scroll_point_x: Option<u32>,
    scroll_point_y: Option<u32>,
    click_x: Option<u32>,
    click_y: Option<u32>,
    hover_x: Option<u32>,
    hover_y: Option<u32>,
    typed_text: Option<String>,
}

impl LiveInput {
    fn scroll_point(&self) -> Result<(u32, u32), LiveSidecarError> {
        let point = match (self.scroll_point_x, self.scroll_point_y) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(LiveSidecarError::IncompleteScrollPoint),
        };
        Ok(point)
    }
}

fn poll_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rendering_context_kind: RenderingContextKind,
) -> Result<LiveOutcome, LiveSidecarError> {
    let started_at = Instant::now();
    let mut latest = None;

    loop {
        host.tick();
        let snapshot = host.snapshot(&session.webview_id)?;
        if snapshot.has_pending_frame() {
            let (outcome, has_visible_content) =
                paint_pending_frame(host, session, rendering_context_kind)?;
            if has_visible_content {
                session.awaiting_visible_frame = false;
                session.ever_visible_frame = true;
                return Ok(outcome);
            }
            if !session.awaiting_visible_frame {
                return Ok(outcome);
            }
            latest = Some(outcome);
        }

        if !session.awaiting_visible_frame {
            return Ok(LiveOutcome::empty());
        }
        if started_at.elapsed() >= LIVE_FRAME_WAIT_TIMEOUT {
            return Ok(latest.unwrap_or_else(LiveOutcome::empty));
        }

        thread::sleep(LIVE_FRAME_WAIT_INTERVAL);
    }
}

fn paint_pending_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rendering_context_kind: RenderingContextKind,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    match rendering_context_kind {
        RenderingContextKind::Software => paint_readback_frame(host, session),
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        RenderingContextKind::Hardware => paint_hardware_surface_frame(host, session),
        #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
        RenderingContextKind::Hardware => paint_readback_frame(host, session),
    }
}

fn paint_readback_frame(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    let paint_started_at = Instant::now();
    host.paint(&session.webview_id)?;
    let snapshot = host.snapshot(&session.webview_id)?;
    let frame = host.last_rendered_frame()?;
    let paint_ns = elapsed_ns(paint_started_at);
    let encode_started_at = Instant::now();
    let has_visible_content = session.ever_visible_frame
        || (frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0);
    let report = LiveFrameReport::new(&snapshot, &frame);
    let encode_ns = elapsed_ns(encode_started_at);
    let timings = PartialFrameTimings { paint_ns, encode_ns };
    Ok((LiveOutcome::from_frame(report, frame, timings), has_visible_content))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn paint_hardware_surface_frame(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    let paint_started_at = Instant::now();
    host.paint_without_readback(&session.webview_id)?;
    let snapshot = host.snapshot(&session.webview_id)?;
    let paint_ns = elapsed_ns(paint_started_at);
    let encode_started_at = Instant::now();
    let report = LiveFrameReport::new_hardware_surface(&snapshot, session.width, session.height);
    let encode_ns = elapsed_ns(encode_started_at);
    let timings = PartialFrameTimings { paint_ns, encode_ns };
    Ok((LiveOutcome::from_report(report, timings), true))
}

#[derive(Clone)]
struct LiveSession {
    webview_id: ely_domain::WebViewId,
    requested_url: String,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    /// Last hidpi factor pushed to Servo, encoded as `(scale × 1000)`.
    /// Stored as a u32 so equality is cheap and stable across the
    /// f32 jitter that JSON parsing can introduce. Init to 0 so the
    /// first apply_layout call always pushes a real value.
    hidpi_scale_milli: u32,
    scroll_x: i32,
    scroll_y: i32,
    awaiting_visible_frame: bool,
    /// Sticky for the lifetime of a single URL: flipped to `true`
    /// the first time `poll_frame` sees a paint with real content
    /// (non-white, non-empty) and reset to `false` on every navigate.
    /// After it's `true`, the visible-content gate stops gating —
    /// scroll/click/hover/type all return on the first
    /// `has_pending_frame=true` (~3 ms) instead of waiting the full
    /// `LIVE_FRAME_WAIT_TIMEOUT`. The gate stays armed for the
    /// initial paint of each new URL so loading frames are still
    /// skipped.
    ever_visible_frame: bool,
}

impl LiveSession {
    fn new(webview_id: ely_domain::WebViewId, width: u32, height: u32) -> Self {
        Self {
            webview_id,
            requested_url: String::new(),
            width: width.max(1),
            height: height.max(1),
            page_zoom_percent: DEFAULT_ZOOM_PERCENT,
            hidpi_scale_milli: 0,
            scroll_x: 0,
            scroll_y: 0,
            awaiting_visible_frame: false,
            ever_visible_frame: false,
        }
    }
}

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    value.clamp(0, i64::from(i32::MAX)) as i32
}
