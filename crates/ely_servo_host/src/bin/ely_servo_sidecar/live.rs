use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, BufRead},
    path::Path,
};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    NavigationRequest, RenderingContextKind, ServoHost, ServoSurfaceSize, SoftwareServoHost,
};

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
use super::live_surface::HardwareSurfaceTransport;
use super::{
    args::{LiveArgs, SidecarRenderingContext},
    live_output::write_outcome,
    live_protocol::{
        LIVE_PROTOCOL_VERSION, LiveFrameReport, LiveOutcome, LiveRequest, LiveSidecarError,
        validated_frame_byte_count,
    },
    live_session::{
        LiveInput, LiveSession, apply_input, apply_layout, apply_permissions, bind_profile,
        ensure_session,
    },
};

const PROFILE_DATA_LEASE_FILE: &str = ".ely-servo-sidecar.lock";

pub(super) fn run(args: LiveArgs) -> Result<(), LiveSidecarError> {
    let LiveArgs { profile_data_dir, rendering_context, iosurface_mach_service } = args;
    fs::create_dir_all(&profile_data_dir)?;
    let _profile_data_lease = acquire_profile_data_lease(&profile_data_dir)?;
    let rendering_context_kind = match rendering_context {
        SidecarRenderingContext::Software => RenderingContextKind::Software,
        SidecarRenderingContext::Hardware => RenderingContextKind::Hardware,
    };
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    let mut hardware_transport = match rendering_context {
        SidecarRenderingContext::Software => None,
        SidecarRenderingContext::Hardware => {
            let service = iosurface_mach_service
                .as_deref()
                .ok_or(LiveSidecarError::IOSurfaceMachServiceRequired)?;
            Some(HardwareSurfaceTransport::connect(service)?)
        }
    };
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    drop(iosurface_mach_service);
    let mut host = SoftwareServoHost::new_with_config_dir_and_kind(
        ServoSurfaceSize::new(1, 1),
        Some(profile_data_dir),
        rendering_context_kind,
    )?;
    let mut sessions = HashMap::new();
    let mut active_profile = None;
    let mut handshake_complete = false;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request = serde_json::from_str::<LiveRequest>(&line);
        let should_shutdown =
            request.as_ref().is_ok_and(|request| matches!(request, LiveRequest::Shutdown));
        let outcome = request.map_err(LiveSidecarError::from).and_then(|request| {
            handle_request(
                &mut host,
                &mut sessions,
                &mut active_profile,
                &mut handshake_complete,
                rendering_context_kind,
                #[cfg(all(feature = "hardware-render", target_os = "macos"))]
                hardware_transport.as_mut(),
                request,
            )
        });
        let outcome = outcome
            .map(|outcome| outcome.with_permission_consumptions(host.take_consumed_permissions()));
        write_outcome(&mut stdout, outcome)?;
        if should_shutdown {
            break;
        }
    }
    Ok(())
}

fn acquire_profile_data_lease(profile_data_dir: &Path) -> Result<File, LiveSidecarError> {
    let lease = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(profile_data_dir.join(PROFILE_DATA_LEASE_FILE))?;
    match lease.try_lock() {
        Ok(()) => Ok(lease),
        Err(TryLockError::WouldBlock) => Err(LiveSidecarError::ProfileDataDirectoryInUse {
            path: profile_data_dir.to_path_buf(),
        }),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn handle_request(
    host: &mut SoftwareServoHost,
    sessions: &mut HashMap<String, LiveSession>,
    active_profile: &mut Option<ProfileId>,
    handshake_complete: &mut bool,
    rendering_context_kind: RenderingContextKind,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))] hardware_transport: Option<
        &mut HardwareSurfaceTransport,
    >,
    request: LiveRequest,
) -> Result<LiveOutcome, LiveSidecarError> {
    if !*handshake_complete
        && !matches!(&request, LiveRequest::Handshake { .. } | LiveRequest::Shutdown)
    {
        return Err(LiveSidecarError::ProtocolHandshakeRequired);
    }
    match request {
        LiveRequest::Handshake { protocol_version } => {
            if protocol_version != LIVE_PROTOCOL_VERSION {
                return Err(LiveSidecarError::ProtocolVersionMismatch {
                    expected: LIVE_PROTOCOL_VERSION,
                    actual: protocol_version,
                });
            }
            *handshake_complete = true;
            Ok(LiveOutcome::empty())
        }
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
            site_permission_generation,
            site_permissions,
            ready_surface_ids,
            pending_surface_ids,
        } => {
            validated_frame_byte_count(width, height)?;
            let tab = TabId::parse(tab_id.clone())?;
            let profile = ProfileId::parse(profile_id)?;
            bind_profile(active_profile, &profile)?;
            let url = UrlText::parse(url)?;
            let session =
                ensure_session(host, sessions, tab_id.clone(), &tab, &profile, width, height)?;
            apply_layout(host, session, width, height, page_zoom_percent, device_pixel_ratio)?;
            apply_permissions(
                host,
                session,
                &profile,
                site_permission_generation,
                site_permissions,
            )?;
            if session.requested_url != url.as_str() {
                let servo_current_url =
                    host.snapshot(&session.webview_id)?.url().map(str::to_string);
                if servo_current_url.as_deref() == Some(url.as_str()) {
                    session.requested_url = url.as_str().to_string();
                } else {
                    session.clear_presented_frame();
                    host.navigate(NavigationRequest {
                        webview_id: session.webview_id.clone(),
                        tab_id: tab,
                        url: url.clone(),
                    })?;
                    session.requested_url = url.as_str().to_string();
                }
            }
            apply_input(
                host,
                session,
                LiveInput {
                    scroll_delta_x,
                    scroll_delta_y,
                    scroll_point_x,
                    scroll_point_y,
                    click_x,
                    click_y,
                    hover_x,
                    hover_y,
                    typed_text,
                },
            )?;
            let webview_id = session.webview_id.clone();
            let outcome = poll_frame(
                host,
                session,
                rendering_context_kind,
                &ready_surface_ids,
                &pending_surface_ids,
            )?;
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            let outcome = {
                let mut outcome = outcome;
                if let Some(transport) = hardware_transport {
                    transport.publish_frame(
                        host,
                        &tab_id,
                        &webview_id,
                        &ready_surface_ids,
                        &pending_surface_ids,
                        &mut outcome,
                    )?;
                }
                outcome
            };
            #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
            drop((webview_id, ready_surface_ids, pending_surface_ids));
            Ok(outcome)
        }
        LiveRequest::Poll { tab_id, ready_surface_ids, pending_surface_ids } => {
            let Some(session) = sessions.get_mut(&tab_id) else {
                return Ok(LiveOutcome::empty());
            };
            let webview_id = session.webview_id.clone();
            let outcome = poll_frame(
                host,
                session,
                rendering_context_kind,
                &ready_surface_ids,
                &pending_surface_ids,
            )?;
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            let outcome = {
                let mut outcome = outcome;
                if let Some(transport) = hardware_transport {
                    transport.publish_frame(
                        host,
                        &tab_id,
                        &webview_id,
                        &ready_surface_ids,
                        &pending_surface_ids,
                        &mut outcome,
                    )?;
                }
                outcome
            };
            #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
            drop((webview_id, ready_surface_ids, pending_surface_ids));
            Ok(outcome)
        }
        LiveRequest::Close { tab_id } => {
            if let Some(session) = sessions.remove(&tab_id) {
                host.close_webview(&session.webview_id);
            }
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            if let Some(transport) = hardware_transport {
                transport.close_tab(&tab_id);
            }
            Ok(LiveOutcome::empty())
        }
        LiveRequest::Shutdown => Ok(LiveOutcome::empty()),
    }
}

fn poll_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    rendering_context_kind: RenderingContextKind,
    ready_surface_ids: &[u64],
    pending_surface_ids: &[u64],
) -> Result<LiveOutcome, LiveSidecarError> {
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    let _ = (ready_surface_ids, pending_surface_ids);
    match rendering_context_kind {
        RenderingContextKind::Software => poll_software_frame(host, session),
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        RenderingContextKind::Hardware => {
            poll_hardware_frame(host, session, ready_surface_ids, pending_surface_ids)
        }
        #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
        RenderingContextKind::Hardware => poll_software_frame(host, session),
    }
}

fn poll_software_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
) -> Result<LiveOutcome, LiveSidecarError> {
    host.tick();
    let snapshot = host.snapshot(&session.webview_id)?;
    let has_pending_frame = snapshot.has_pending_frame();
    if !has_pending_frame {
        if snapshot.has_pending_metadata()
            && let Some(frame) = session.last_frame.clone()
        {
            let snapshot = host.snapshot_and_mark_metadata_observed(&session.webview_id)?;
            let report =
                LiveFrameReport::new(&snapshot, &frame, session.device_pixel_ratio(), false);
            return Ok(LiveOutcome::frame(report, frame));
        }
        return Ok(LiveOutcome::empty());
    }

    host.paint_with_readback(&session.webview_id)?;
    let snapshot = host.snapshot_and_mark_metadata_observed(&session.webview_id)?;
    let frame = host.last_rendered_frame()?;
    session.last_frame = Some(frame.clone());
    let report = LiveFrameReport::new(&snapshot, &frame, session.device_pixel_ratio(), true);
    Ok(LiveOutcome::frame(report, frame))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn poll_hardware_frame(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    ready_surface_ids: &[u64],
    pending_surface_ids: &[u64],
) -> Result<LiveOutcome, LiveSidecarError> {
    host.acknowledge_iosurfaces(&session.webview_id, ready_surface_ids)?;
    let (last_surface_became_ready, last_surface_is_missing) = session
        .last_surface
        .map(|identity| {
            let was_ready = session.last_surface_ready;
            let ready = ready_surface_ids.contains(&identity.surface_id);
            let pending = pending_surface_ids.contains(&identity.surface_id);
            session.last_surface_ready = ready;
            (ready && !was_ready, !ready && !pending)
        })
        .unwrap_or((false, false));
    host.tick();
    let snapshot = host.snapshot(&session.webview_id)?;
    match hardware_poll_action(
        snapshot.has_pending_frame(),
        snapshot.has_pending_metadata(),
        last_surface_became_ready,
        last_surface_is_missing,
        session.last_surface.is_some(),
        session.last_surface_ready,
    ) {
        HardwarePollAction::ReplaySurface => {
            let identity = session.last_surface.ok_or_else(|| {
                ely_servo_host::ServoHostError::HardwareSurfaceUnavailable {
                    id: session.webview_id.clone(),
                }
            })?;
            let snapshot = host.snapshot_and_mark_metadata_observed(&session.webview_id)?;
            let report = LiveFrameReport::from_surface(
                &snapshot,
                identity.width,
                identity.height,
                session.device_pixel_ratio(),
                false,
            );
            return Ok(LiveOutcome::surface(report));
        }
        HardwarePollAction::Empty => return Ok(LiveOutcome::empty()),
        HardwarePollAction::PaintFrame => {}
    }

    host.paint_without_readback(&session.webview_id)?;
    let snapshot = host.snapshot_and_mark_metadata_observed(&session.webview_id)?;
    let identity = host.peek_iosurface_identity(&session.webview_id)?.ok_or_else(|| {
        ely_servo_host::ServoHostError::HardwareSurfaceUnavailable {
            id: session.webview_id.clone(),
        }
    })?;
    session.last_surface = Some(identity);
    session.last_surface_ready = ready_surface_ids.contains(&identity.surface_id);
    let report = LiveFrameReport::from_surface(
        &snapshot,
        identity.width,
        identity.height,
        session.device_pixel_ratio(),
        true,
    );
    Ok(LiveOutcome::surface(report))
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HardwarePollAction {
    ReplaySurface,
    PaintFrame,
    Empty,
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
fn hardware_poll_action(
    has_pending_frame: bool,
    has_pending_metadata: bool,
    last_surface_became_ready: bool,
    last_surface_is_missing: bool,
    has_last_surface: bool,
    last_surface_ready: bool,
) -> HardwarePollAction {
    if has_last_surface && (last_surface_became_ready || last_surface_is_missing) {
        HardwarePollAction::ReplaySurface
    } else if has_last_surface && !last_surface_ready {
        HardwarePollAction::Empty
    } else if has_last_surface && !has_pending_frame && has_pending_metadata {
        HardwarePollAction::ReplaySurface
    } else if has_pending_frame {
        HardwarePollAction::PaintFrame
    } else {
        HardwarePollAction::Empty
    }
}

#[cfg(all(test, feature = "hardware-render", target_os = "macos"))]
mod tests {
    use super::{HardwarePollAction, hardware_poll_action};

    #[test]
    fn newly_ready_surface_replays_before_pending_frame() {
        assert_eq!(
            hardware_poll_action(true, false, true, false, true, true),
            HardwarePollAction::ReplaySurface
        );
        assert_eq!(
            hardware_poll_action(true, false, false, false, true, true),
            HardwarePollAction::PaintFrame
        );
    }

    #[test]
    fn awaiting_ready_surface_backpressures_pending_frame() {
        assert_eq!(
            hardware_poll_action(true, true, false, false, true, false),
            HardwarePollAction::Empty
        );
    }

    #[test]
    fn missing_surface_replays_before_first_ready() {
        assert_eq!(
            hardware_poll_action(true, false, false, true, true, false),
            HardwarePollAction::ReplaySurface
        );
    }
}
