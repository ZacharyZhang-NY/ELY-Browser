use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead, Write},
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
use super::live_protocol::{
    LiveFrameReport, LiveOutcome, LiveRequest, LiveSitePermission, PartialFrameTimings,
};
pub(super) use super::live_protocol::LiveSidecarError;
use super::perf::{FramePerfAggregator, FramePerfSummary, FrameStageTimings, elapsed_ns};

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
        write_outcome(
            &mut stdout,
            &mut perf,
            &mut pending_summary,
            outcome,
            frame_started_at,
        )?;
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
            scroll_delta_x,
            scroll_delta_y,
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
            populate_surface_fields(host, &webview_id, &tab_id, published_surface_ids, &mut outcome);
            Ok(outcome)
        }
        LiveRequest::Poll { tab_id } => {
            let Some(session) = sessions.get_mut(&tab_id) else {
                return Ok(LiveOutcome::empty());
            };
            let webview_id = session.webview_id.clone();
            let mut outcome = poll_frame(host, session, rendering_context_kind)?;
            populate_surface_fields(host, &webview_id, &tab_id, published_surface_ids, &mut outcome);
            Ok(outcome)
        }
    }
}

/// Populate the hardware surface protocol fields on `outcome`. Two
/// pieces of state ride out together:
///
///   * `current_surface_id` — set on every payload-bearing hardware
///     frame so the receiver knows which previously-imported
///     `MTLTexture` to sample THIS frame. surfman's attached swap
///     chain rotates front/back surfaces, so this alternates between
///     a small set of ids.
///   * `surface_handle` — populated only the first time the sidecar
///     sees a given `surface_id`; the receiver imports the IOSurface
///     once and caches the resulting Metal texture. Minting a fresh
///     mach port per frame would leak ports — `IOSurfaceCreateMachPort`
///     hands out a new send right each call and they don't free
///     automatically until the receiver `mach_port_deallocate`s.
fn populate_surface_fields(
    host: &SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    tab_id: &str,
    published_surface_ids: &mut HashMap<String, HashSet<u64>>,
    outcome: &mut LiveOutcome,
) {
    if outcome.frame.is_none() {
        return;
    }
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    {
        let Ok(Some(identity)) = host.peek_iosurface_identity(webview_id) else {
            return;
        };
        outcome.response.current_surface_id = Some(identity.surface_id);
        let seen = published_surface_ids.entry(tab_id.to_string()).or_default();
        if seen.contains(&identity.surface_id) {
            return;
        }
        let Ok(Some(handle)) = host.current_iosurface_handle(webview_id) else {
            return;
        };
        seen.insert(handle.surface_id);
        outcome.response.surface_handle = Some(handle);
    }
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    {
        let _ = (host, webview_id, tab_id, published_surface_ids);
    }
}

/// Serialise the response then stream the optional raw RGBA frame on
/// the same stdout pipe. The client reads the JSON line, takes
/// `rgba_byte_count` from the report, then reads that many bytes
/// from the same stream — no temp file round-trip.
///
/// After the bytes hit the pipe we fold paint/encode/write/total
/// timings into the aggregator. `total_ns` is the wall-clock span
/// from `frame_started_at` (request arrival) to the stdout flush
/// returning, so it captures every per-frame cost outside the three
/// measured stages. Any summary the aggregator emits is stashed on
/// `pending_summary` and rides out on the *next* response, because
/// the protocol is one-line-per-response and an unsolicited summary
/// line would desync the main process's read loop.
fn write_outcome(
    stdout: &mut impl Write,
    perf: &mut FramePerfAggregator,
    pending_summary: &mut Option<FramePerfSummary>,
    outcome: Result<LiveOutcome, LiveSidecarError>,
    frame_started_at: Instant,
) -> Result<(), LiveSidecarError> {
    let mut outcome = outcome.unwrap_or_else(|error| LiveOutcome::error(error.to_string()));
    let partial_timings = outcome.partial_timings.take();
    let frame_present = outcome.frame.is_some();
    if let Some(summary) = pending_summary.take() {
        outcome.response.perf = Some(summary);
    }
    // Hardware path: receiver samples the IOSurface directly through
    // its CVPixelBuffer cache, so the raw RGBA payload is dead
    // weight. Drop it from the wire (and zero the byte count in the
    // header so the client knows nothing follows). At 1080p × 60 fps
    // that's 8 MB × 60 = ~480 MB/s of pipe traffic eliminated.
    let drop_rgba_payload = outcome.response.current_surface_id.is_some();
    if drop_rgba_payload {
        if let Some(report) = outcome.response.frame.as_mut() {
            report.rgba_byte_count = 0;
        }
    }
    let write_started_at = Instant::now();
    serde_json::to_writer(&mut *stdout, &outcome.response)?;
    stdout.write_all(b"\n")?;
    if !drop_rgba_payload {
        if let Some(frame) = outcome.frame.as_ref() {
            stdout.write_all(frame.rgba_bytes())?;
        }
    }
    stdout.flush()?;
    if frame_present {
        let write_ns = elapsed_ns(write_started_at);
        let total_ns = elapsed_ns(frame_started_at);
        let partial = partial_timings.unwrap_or(PartialFrameTimings { paint_ns: 0, encode_ns: 0 });
        let timings = FrameStageTimings::from_durations(
            Duration::from_nanos(partial.paint_ns),
            Duration::from_nanos(partial.encode_ns),
            Duration::from_nanos(write_ns),
            Duration::from_nanos(total_ns),
        );
        if let Some(summary) = perf.record(timings) {
            *pending_summary = Some(summary);
        }
    }
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
    rendering_context_kind: RenderingContextKind,
) -> Result<LiveOutcome, LiveSidecarError> {
    let started_at = Instant::now();
    let mut latest = None;

    loop {
        host.tick();
        let snapshot = host.snapshot(&session.webview_id)?;
        if snapshot.has_pending_frame() {
            let paint_started_at = Instant::now();
            host.paint(&session.webview_id)?;
            let snapshot = host.snapshot(&session.webview_id)?;
            let frame = host.last_rendered_frame()?;
            let paint_ns = elapsed_ns(paint_started_at);
            let encode_started_at = Instant::now();
            // `non_white`/`content_pixel_count` come from a CPU
            // readback of the bound framebuffer. On the software
            // path that's the source of truth: Servo's compositor
            // returns a white framebuffer until layout completes, so
            // the threshold check is how we skip blank loading
            // frames. On the hardware path the readback goes through
            // `glReadPixels` on the surfman swap-chain surface, which
            // can return content that fails the threshold even when
            // Servo painted real pixels (the IOSurface itself is
            // valid for GPUI to sample directly). `has_pending_frame`
            // already encodes "Servo finished painting" — trust it
            // for the hardware path instead of double-checking via a
            // readback we know to be unreliable.
            let has_visible_content = match rendering_context_kind {
                RenderingContextKind::Software => {
                    frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0
                }
                #[cfg(feature = "hardware-render")]
                RenderingContextKind::Hardware => true,
                #[cfg(not(feature = "hardware-render"))]
                RenderingContextKind::Hardware => {
                    frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0
                }
            };
            let report = LiveFrameReport::new(&snapshot, &frame);
            let encode_ns = elapsed_ns(encode_started_at);
            let timings = PartialFrameTimings { paint_ns, encode_ns };
            let outcome = LiveOutcome::from_frame(report, frame, timings);
            if has_visible_content {
                session.awaiting_visible_frame = false;
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

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    value.clamp(0, i64::from(i32::MAX)) as i32
}
