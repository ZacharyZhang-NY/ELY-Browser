use std::{
    collections::{HashMap, HashSet},
    io::Write,
    time::{Duration, Instant},
};

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
use ely_servo_host::IOSurfaceHandle;
use ely_servo_host::{IOSurfaceIdentity, SoftwareServoHost};

use super::live_protocol::{LiveOutcome, LiveSidecarError, PartialFrameTimings};
use super::perf::{FramePerfAggregator, FramePerfSummary, FrameStageTimings, elapsed_ns};

/// Populate the hardware surface protocol fields on `outcome`. Readback
/// warm-up frames publish IOSurface handles so the app can import them
/// on its dedicated importer thread before steady-state payloadless
/// frames select the rotating surface ids. Two pieces of state ride out
/// together:
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
pub(super) fn populate_surface_fields(
    host: &SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    tab_id: &str,
    published_surface_ids: &mut HashMap<String, HashSet<IOSurfaceIdentity>>,
    publish_readback_surface_fields: bool,
    outcome: &mut LiveOutcome,
) {
    if outcome.response.frame.is_none() {
        return;
    }
    if outcome.frame.is_some() && !publish_readback_surface_fields {
        return;
    }
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    {
        let Ok(Some(identity)) = host.peek_iosurface_identity(webview_id) else {
            return;
        };
        if let Err(message) = require_report_matches_surface_identity(outcome, identity) {
            *outcome = LiveOutcome::error(message);
            return;
        }
        let handle = if surface_has_been_published(published_surface_ids, tab_id, identity) {
            None
        } else {
            host.current_iosurface_handle(webview_id).ok().flatten()
        };
        let publication = surface_publication_for(published_surface_ids, tab_id, identity, handle);
        outcome.response.current_surface_id = publication.current_surface_id;
        outcome.response.surface_handle = publication.surface_handle;
    }
    #[cfg(not(all(feature = "hardware-render", target_os = "macos")))]
    {
        let _ = (host, webview_id, tab_id, published_surface_ids, publish_readback_surface_fields);
    }
}

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
fn surface_has_been_published(
    published_surface_ids: &HashMap<String, HashSet<IOSurfaceIdentity>>,
    tab_id: &str,
    identity: IOSurfaceIdentity,
) -> bool {
    published_surface_ids.get(tab_id).is_some_and(|published| published.contains(&identity))
}

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
#[derive(Clone, Copy)]
struct SurfacePublication {
    current_surface_id: Option<u64>,
    surface_handle: Option<IOSurfaceHandle>,
}

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
fn surface_publication_for(
    published_surface_ids: &mut HashMap<String, HashSet<IOSurfaceIdentity>>,
    tab_id: &str,
    identity: IOSurfaceIdentity,
    handle: Option<IOSurfaceHandle>,
) -> SurfacePublication {
    if surface_has_been_published(published_surface_ids, tab_id, identity) {
        return SurfacePublication {
            current_surface_id: Some(identity.surface_id),
            surface_handle: None,
        };
    }

    let Some(handle) = handle.filter(|handle| handle_matches_identity(*handle, identity)) else {
        return SurfacePublication { current_surface_id: None, surface_handle: None };
    };

    published_surface_ids
        .entry(tab_id.to_string())
        .or_default()
        .insert(IOSurfaceIdentity::from_handle(handle));
    SurfacePublication { current_surface_id: Some(handle.surface_id), surface_handle: Some(handle) }
}

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
fn handle_matches_identity(handle: IOSurfaceHandle, identity: IOSurfaceIdentity) -> bool {
    handle.surface_id == identity.surface_id
        && handle.width == identity.width
        && handle.height == identity.height
}

#[cfg(any(test, all(feature = "hardware-render", target_os = "macos")))]
fn require_report_matches_surface_identity(
    outcome: &LiveOutcome,
    identity: IOSurfaceIdentity,
) -> Result<(), String> {
    let Some(frame) = outcome.response.frame.as_ref() else {
        return Ok(());
    };
    if frame.width == identity.width && frame.height == identity.height {
        return Ok(());
    }

    Err(format!(
        "servo hardware surface size {}x{} did not match frame report {}x{}",
        identity.width, identity.height, frame.width, frame.height,
    ))
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
pub(super) fn write_outcome(
    stdout: &mut impl Write,
    perf: &mut FramePerfAggregator,
    pending_summary: &mut Option<FramePerfSummary>,
    outcome: Result<LiveOutcome, LiveSidecarError>,
    frame_started_at: Instant,
) -> Result<(), LiveSidecarError> {
    let mut outcome = outcome.unwrap_or_else(|error| LiveOutcome::error(error.to_string()));
    let partial_timings = outcome.partial_timings.take();
    let frame_present = outcome.response.frame.is_some();
    if let Some(summary) = pending_summary.take() {
        outcome.response.perf = Some(summary);
    }
    // Payloadless hardware frames carry only the IOSurface selector.
    let drop_rgba_payload =
        outcome.response.current_surface_id.is_some() && outcome.frame.is_none();
    if drop_rgba_payload && let Some(report) = outcome.response.frame.as_mut() {
        report.rgba_byte_count = 0;
    }
    let write_started_at = Instant::now();
    serde_json::to_writer(&mut *stdout, &outcome.response)?;
    stdout.write_all(b"\n")?;
    if !drop_rgba_payload && let Some(frame) = outcome.frame.as_ref() {
        stdout.write_all(frame.rgba_bytes())?;
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, error::Error, time::Instant};

    use ely_servo_host::{IOSurfaceHandle, IOSurfaceIdentity};

    use super::super::{
        live_protocol::{LiveFrameReport, LiveOutcome, PartialFrameTimings},
        perf::FramePerfAggregator,
    };
    use super::{require_report_matches_surface_identity, surface_publication_for, write_outcome};

    #[test]
    fn unpublished_surface_without_handle_leaves_selector_empty() {
        let mut published = HashMap::new();
        let publication =
            surface_publication_for(&mut published, "tab-1", identity(7, 800, 600), None);

        assert_eq!(publication.current_surface_id, None);
        assert!(publication.surface_handle.is_none());
        assert!(published.is_empty());
    }

    #[test]
    fn unpublished_surface_with_matching_handle_publishes_selector_and_handle() {
        let mut published = HashMap::new();
        let handle = handle(7, 800, 600);
        let publication =
            surface_publication_for(&mut published, "tab-1", identity(7, 800, 600), Some(handle));

        assert_eq!(publication.current_surface_id, Some(7));
        assert_eq!(publication.surface_handle, Some(handle));
        assert!(published.get("tab-1").is_some_and(|ids| ids.contains(&identity(7, 800, 600))));
    }

    #[test]
    fn published_surface_reuses_selector_without_republishing_handle() {
        let mut published = HashMap::new();
        let handle = handle(7, 800, 600);
        let _ =
            surface_publication_for(&mut published, "tab-1", identity(7, 800, 600), Some(handle));
        let publication =
            surface_publication_for(&mut published, "tab-1", identity(7, 800, 600), None);

        assert_eq!(publication.current_surface_id, Some(7));
        assert!(publication.surface_handle.is_none());
    }

    #[test]
    fn same_surface_id_with_changed_dimensions_republishes_handle() {
        let mut published = HashMap::new();
        let initial = handle(7, 800, 600);
        let resized = handle(7, 1024, 768);

        let _ =
            surface_publication_for(&mut published, "tab-1", identity(7, 800, 600), Some(initial));
        let publication =
            surface_publication_for(&mut published, "tab-1", identity(7, 1024, 768), Some(resized));

        assert_eq!(publication.current_surface_id, Some(7));
        assert_eq!(publication.surface_handle, Some(resized));
    }

    #[test]
    fn hardware_report_mismatch_is_reported() -> Result<(), Box<dyn Error>> {
        let outcome = LiveOutcome::from_report(
            report_with_size(2180, 1586),
            PartialFrameTimings { paint_ns: 1_000, encode_ns: 2_000 },
        );

        let error = match require_report_matches_surface_identity(&outcome, identity(7, 2168, 1566))
        {
            Ok(()) => return Err("mismatched IOSurface dimensions must be reported".into()),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "servo hardware surface size 2168x1566 did not match frame report 2180x1586",
        );
        Ok(())
    }

    #[test]
    fn mismatched_handle_leaves_surface_unpublished() {
        let mut published = HashMap::new();
        let publication = surface_publication_for(
            &mut published,
            "tab-1",
            identity(7, 800, 600),
            Some(handle(8, 800, 600)),
        );

        assert_eq!(publication.current_surface_id, None);
        assert!(publication.surface_handle.is_none());
        assert!(published.is_empty());
    }

    #[test]
    fn payloadless_surface_report_records_perf_and_writes_no_rgba() -> Result<(), Box<dyn Error>> {
        let mut outcome = LiveOutcome::from_report(
            report_with_byte_count(16),
            PartialFrameTimings { paint_ns: 1_000, encode_ns: 2_000 },
        );
        outcome.response.current_surface_id = Some(7);
        let mut stdout = Vec::new();
        let mut perf = FramePerfAggregator::new("hardware", 1);
        let mut pending_summary = None;

        write_outcome(&mut stdout, &mut perf, &mut pending_summary, Ok(outcome), Instant::now())?;

        let Some(newline_index) = stdout.iter().position(|byte| *byte == b'\n') else {
            return Err("response newline missing".into());
        };
        let line = std::str::from_utf8(&stdout[..newline_index])?;
        let response: serde_json::Value = serde_json::from_str(line)?;
        let rgba_byte_count = response
            .get("frame")
            .and_then(|frame| frame.get("rgba_byte_count"))
            .and_then(serde_json::Value::as_u64);

        assert_eq!(rgba_byte_count, Some(0));
        assert!(stdout[newline_index + 1..].is_empty());
        assert!(pending_summary.is_some());
        Ok(())
    }

    fn identity(surface_id: u64, width: u32, height: u32) -> IOSurfaceIdentity {
        IOSurfaceIdentity { surface_id, width, height }
    }

    fn handle(surface_id: u64, width: u32, height: u32) -> IOSurfaceHandle {
        IOSurfaceHandle { mach_port_name: 42, surface_id, width, height }
    }

    fn report_with_byte_count(rgba_byte_count: usize) -> LiveFrameReport {
        let mut report = report_with_size(2, 2);
        report.rgba_byte_count = rgba_byte_count;
        report
    }

    fn report_with_size(width: u32, height: u32) -> LiveFrameReport {
        LiveFrameReport {
            loaded_url: Some("https://example.com/".to_string()),
            title: Some("Example".to_string()),
            state: "complete",
            width,
            height,
            device_pixel_ratio: 1.0,
            css_viewport_width: width,
            css_viewport_height: height,
            rgba_byte_count: 0,
            non_white_pixel_count: 0,
            content_pixel_count: 0,
            sample_hash: 0,
        }
    }
}
