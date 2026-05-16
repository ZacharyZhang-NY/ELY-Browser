use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead},
    time::Instant,
};

use ely_domain::{ProfileId, TabId, UrlText};
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use ely_servo_host::ServoHostError;
use ely_servo_host::{
    IOSurfaceIdentity, NavigationRequest, RenderingContextKind, ServoHost, ServoSurfaceSize,
    SoftwareServoHost,
};

use super::args::LiveArgs;
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use super::iosurface_mach::{IOSurfaceMachSender, send_surface_port_if_needed};
use super::live_output::{populate_surface_fields, write_outcome};
pub(super) use super::live_protocol::LiveSidecarError;
use super::live_protocol::{LiveFrameReport, LiveOutcome, LiveRequest, PartialFrameTimings};
use super::live_session::{
    LiveInput, LiveSession, apply_input, apply_layout, apply_permissions, ensure_session,
};
use super::perf::{FramePerfAggregator, FramePerfSummary, elapsed_ns};

pub(super) fn run_live(args: LiveArgs) -> Result<(), LiveSidecarError> {
    let LiveArgs { profile_data_dir, iosurface_mach_service, rendering_context_kind } = args;
    let publish_readback_surface_fields = true;
    let require_client_ready_surfaces = iosurface_mach_service.is_some();
    fs::create_dir_all(&profile_data_dir)?;
    let context_label = rendering_context_label(rendering_context_kind);
    let mut host = SoftwareServoHost::new_with_config_dir_and_kind(
        ServoSurfaceSize::new(1, 1),
        Some(profile_data_dir),
        rendering_context_kind,
    )?;
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    let mut iosurface_mach_sender =
        iosurface_mach_service.as_deref().map(IOSurfaceMachSender::connect).transpose()?;
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    let _ = iosurface_mach_service;
    let mut sessions = HashMap::new();
    let mut perf =
        FramePerfAggregator::new(context_label, FramePerfAggregator::DEFAULT_WINDOW_SIZE);
    let mut pending_summary: Option<FramePerfSummary> = None;
    let mut published_surface_ids: HashMap<String, HashSet<IOSurfaceIdentity>> = HashMap::new();
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
                publish_readback_surface_fields,
                require_client_ready_surfaces,
                request,
            ),
            Err(error) => Err(LiveSidecarError::Json(error)),
        };
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        let outcome = {
            let mut outcome = outcome;
            send_surface_port_if_needed(iosurface_mach_sender.as_mut(), &mut outcome);
            outcome
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
    published_surface_ids: &mut HashMap<String, HashSet<IOSurfaceIdentity>>,
    rendering_context_kind: RenderingContextKind,
    publish_readback_surface_fields: bool,
    require_client_ready_surfaces: bool,
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
            ready_surface_ids,
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
                // The app tick calls this sidecar synchronously from
                // GPUI's update path. Mark that a fresh frame is
                // desired, then let poll_frame take one event-loop
                // step; a later 16 ms app tick will poll again if
                // Servo has not painted yet.
                session.awaiting_visible_frame = true;
            }
            let webview_id = session.webview_id.clone();
            let mut outcome = poll_frame(
                host,
                session,
                rendering_context_kind,
                payloadless_readiness(
                    &tab_id,
                    published_surface_ids,
                    &ready_surface_ids,
                    require_client_ready_surfaces,
                ),
            )?;
            populate_surface_fields(
                host,
                &webview_id,
                &tab_id,
                published_surface_ids,
                publish_readback_surface_fields,
                &mut outcome,
            );
            Ok(outcome)
        }
        LiveRequest::Poll { tab_id, ready_surface_ids } => {
            let Some(session) = sessions.get_mut(&tab_id) else {
                return Ok(LiveOutcome::empty());
            };
            let webview_id = session.webview_id.clone();
            let mut outcome = poll_frame(
                host,
                session,
                rendering_context_kind,
                payloadless_readiness(
                    &tab_id,
                    published_surface_ids,
                    &ready_surface_ids,
                    require_client_ready_surfaces,
                ),
            )?;
            populate_surface_fields(
                host,
                &webview_id,
                &tab_id,
                published_surface_ids,
                publish_readback_surface_fields,
                &mut outcome,
            );
            Ok(outcome)
        }
        LiveRequest::Close { tab_id } => {
            if let Some(session) = sessions.remove(&tab_id) {
                host.close_webview(&session.webview_id);
            }
            published_surface_ids.remove(&tab_id);
            Ok(LiveOutcome::empty())
        }
    }
}

fn poll_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rendering_context_kind: RenderingContextKind,
    readiness: PayloadlessReadiness<'_>,
) -> Result<LiveOutcome, LiveSidecarError> {
    host.tick();
    let snapshot = host.snapshot(&session.webview_id)?;
    let has_pending_frame = snapshot.has_pending_frame();
    if !should_paint_live_frame(has_pending_frame, session.awaiting_visible_frame) {
        return Ok(LiveOutcome::empty());
    }

    let (outcome, has_visible_content) =
        paint_pending_frame(host, session, rendering_context_kind, readiness, has_pending_frame)?;
    if has_visible_content {
        session.awaiting_visible_frame = false;
        session.ever_visible_frame = true;
        return Ok(outcome);
    }
    if !session.awaiting_visible_frame {
        return Ok(outcome);
    }

    Ok(LiveOutcome::empty())
}

fn should_paint_live_frame(has_pending_frame: bool, awaiting_visible_frame: bool) -> bool {
    has_pending_frame || awaiting_visible_frame
}

fn paint_pending_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rendering_context_kind: RenderingContextKind,
    readiness: PayloadlessReadiness<'_>,
    has_pending_frame: bool,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    let _ = readiness;

    match rendering_context_kind {
        RenderingContextKind::Software => paint_readback_frame(host, session, !has_pending_frame),
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        RenderingContextKind::Hardware => {
            paint_hardware_surface_frame(host, session, readiness, has_pending_frame)
        }
        #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
        RenderingContextKind::Hardware => paint_readback_frame(host, session, !has_pending_frame),
    }
}

fn paint_readback_frame(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    wait_for_completion: bool,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    let paint_started_at = Instant::now();
    host.paint_with_readback(&session.webview_id, wait_for_completion)?;
    let snapshot = host.snapshot(&session.webview_id)?;
    let frame = host.last_rendered_frame()?;
    let paint_ns = elapsed_ns(paint_started_at);
    let encode_started_at = Instant::now();
    let has_visible_content = session.ever_visible_frame
        || (frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0);
    let report = LiveFrameReport::new(&snapshot, &frame, session.device_pixel_ratio());
    let encode_ns = elapsed_ns(encode_started_at);
    let timings = PartialFrameTimings { paint_ns, encode_ns };
    Ok((LiveOutcome::from_frame(report, frame, timings), has_visible_content))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn paint_hardware_surface_frame(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    readiness: PayloadlessReadiness<'_>,
    has_pending_frame: bool,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    if !session.ever_visible_frame {
        return paint_initial_hardware_surface_frame(host, session, !has_pending_frame);
    }
    if !payloadless_surface_pool_ready(readiness, session.width, session.height) {
        return paint_readback_frame(host, session, !has_pending_frame);
    }
    let (outcome, identity) = paint_hardware_surface_report(host, session, !has_pending_frame)?;
    if !surface_has_been_published(readiness.published_surface_ids, readiness.tab_id, identity) {
        return paint_readback_frame(host, session, true);
    }
    Ok((outcome, true))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn paint_initial_hardware_surface_frame(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    wait_for_completion: bool,
) -> Result<(LiveOutcome, bool), LiveSidecarError> {
    let paint_started_at = Instant::now();
    host.paint_with_readback(&session.webview_id, wait_for_completion)?;
    let snapshot = host.snapshot(&session.webview_id)?;
    let frame = host.last_rendered_frame()?;
    let paint_ns = elapsed_ns(paint_started_at);
    let encode_started_at = Instant::now();
    let report = LiveFrameReport::new(&snapshot, &frame, session.device_pixel_ratio());
    let has_visible_content = frame.non_white_pixel_count() > 0 && frame.content_pixel_count() > 0;
    let encode_ns = elapsed_ns(encode_started_at);
    let timings = PartialFrameTimings { paint_ns, encode_ns };
    Ok((LiveOutcome::from_frame(report, frame, timings), has_visible_content))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn paint_hardware_surface_report(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    wait_for_completion: bool,
) -> Result<(LiveOutcome, IOSurfaceIdentity), LiveSidecarError> {
    let paint_started_at = Instant::now();
    host.paint_without_readback_with_completion(&session.webview_id, wait_for_completion)?;
    let snapshot = host.snapshot(&session.webview_id)?;
    let identity = host.peek_iosurface_identity(&session.webview_id)?.ok_or_else(|| {
        ServoHostError::HardwareSurfaceUnavailable { id: session.webview_id.clone() }
    })?;
    let paint_ns = elapsed_ns(paint_started_at);
    let encode_started_at = Instant::now();
    let report = LiveFrameReport::from_surface(
        &snapshot,
        identity.width,
        identity.height,
        session.device_pixel_ratio(),
    );
    let encode_ns = elapsed_ns(encode_started_at);
    let timings = PartialFrameTimings { paint_ns, encode_ns };
    Ok((LiveOutcome::from_report(report, timings), identity))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn payloadless_surface_pool_ready(
    readiness: PayloadlessReadiness<'_>,
    width: u32,
    height: u32,
) -> bool {
    let Some(published) = readiness.published_surface_ids.get(readiness.tab_id) else {
        return false;
    };
    let matching = published
        .iter()
        .filter(|identity| identity.width == width && identity.height == height)
        .take(2)
        .copied()
        .collect::<Vec<_>>();
    if matching.len() < 2 {
        return false;
    }
    !readiness.require_client_ready_surfaces
        || matching
            .iter()
            .all(|identity| readiness.ready_surface_ids.contains(&identity.surface_id))
}

#[derive(Clone, Copy)]
struct PayloadlessReadiness<'a> {
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    tab_id: &'a str,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    published_surface_ids: &'a HashMap<String, HashSet<IOSurfaceIdentity>>,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    ready_surface_ids: &'a [u64],
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    require_client_ready_surfaces: bool,
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    _marker: std::marker::PhantomData<&'a ()>,
}

fn payloadless_readiness<'a>(
    tab_id: &'a str,
    published_surface_ids: &'a HashMap<String, HashSet<IOSurfaceIdentity>>,
    ready_surface_ids: &'a [u64],
    require_client_ready_surfaces: bool,
) -> PayloadlessReadiness<'a> {
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    {
        PayloadlessReadiness {
            tab_id,
            published_surface_ids,
            ready_surface_ids,
            require_client_ready_surfaces,
        }
    }
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    {
        let _ = (tab_id, published_surface_ids, ready_surface_ids, require_client_ready_surfaces);
        PayloadlessReadiness { _marker: std::marker::PhantomData }
    }
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn surface_has_been_published(
    published_surface_ids: &HashMap<String, HashSet<IOSurfaceIdentity>>,
    tab_id: &str,
    identity: IOSurfaceIdentity,
) -> bool {
    published_surface_ids.get(tab_id).is_some_and(|published| published.contains(&identity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awaiting_visible_frame_forces_paint_without_pending_flag() {
        assert!(should_paint_live_frame(false, true));
    }

    #[test]
    fn idle_poll_waits_for_pending_frame() {
        assert!(!should_paint_live_frame(false, false));
        assert!(should_paint_live_frame(true, false));
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[test]
    fn payloadless_pool_waits_for_client_ready_surfaces_when_required() {
        let published = published_identities([identity(7, 800, 600), identity(8, 800, 600)]);

        assert!(!payloadless_surface_pool_ready(readiness(&published, &[7], true), 800, 600));
        assert!(payloadless_surface_pool_ready(readiness(&published, &[7, 8], true), 800, 600));
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    #[test]
    fn payloadless_pool_uses_published_surfaces_for_no_mach_clients() {
        let published = published_identities([identity(7, 800, 600), identity(8, 800, 600)]);

        assert!(payloadless_surface_pool_ready(readiness(&published, &[], false), 800, 600));
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    fn readiness<'a>(
        published_surface_ids: &'a HashMap<String, HashSet<IOSurfaceIdentity>>,
        ready_surface_ids: &'a [u64],
        require_client_ready_surfaces: bool,
    ) -> PayloadlessReadiness<'a> {
        PayloadlessReadiness {
            tab_id: "tab",
            published_surface_ids,
            ready_surface_ids,
            require_client_ready_surfaces,
        }
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    fn published_identities(
        identities: [IOSurfaceIdentity; 2],
    ) -> HashMap<String, HashSet<IOSurfaceIdentity>> {
        let mut published = HashMap::new();
        published.insert("tab".to_string(), identities.into_iter().collect());
        published
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    fn identity(surface_id: u64, width: u32, height: u32) -> IOSurfaceIdentity {
        IOSurfaceIdentity { surface_id, width, height }
    }
}
