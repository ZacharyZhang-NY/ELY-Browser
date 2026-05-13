use std::{
    collections::{HashMap, HashSet},
    io::Write,
    time::{Duration, Instant},
};

use ely_servo_host::SoftwareServoHost;

use super::live_protocol::{LiveOutcome, LiveSidecarError, PartialFrameTimings};
use super::perf::{FramePerfAggregator, FramePerfSummary, FrameStageTimings, elapsed_ns};

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
pub(super) fn populate_surface_fields(
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
pub(super) fn write_outcome(
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
