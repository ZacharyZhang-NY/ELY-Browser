//! Manually-invoked sidecar perf bench. Runs the live loop, scrolls a
//! tall page over N frames, and harvests the [`FramePerfSummary`]
//! entries the sidecar emits every window. Invoked once per kind:
//!
//! ```text
//! ELY_PERF_KIND=software ELY_PERF_FRAMES=240 \
//!   cargo test -p ely_servo_host --release --test live_perf_bench \
//!     --features servo-engine -- --ignored --nocapture run_live_bench
//!
//! ELY_PERF_KIND=hardware ELY_PERF_FRAMES=240 \
//!   cargo test -p ely_servo_host --release --test live_perf_bench \
//!     --features servo-engine,hardware-render -- --ignored --nocapture run_live_bench
//! ```
//!
//! Marked `#[ignore]` so normal CI skips it; it shells out to the
//! sidecar binary and runs for tens of seconds.
#![cfg(feature = "servo-engine")]

use std::{
    env,
    error::Error,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use ely_domain::{ProfileId, TabId};
use serde::Deserialize;

const DEFAULT_FRAMES: u32 = 240;
const VIEWPORT_WIDTH: u32 = 1024;
const VIEWPORT_HEIGHT: u32 = 768;
const SCROLL_STEP_PX: i32 = 4;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
const SCROLL_PAGE_DATA_URL: &str = "data:text/html,\
<!doctype html><meta charset=utf-8><title>perf</title>\
<style>html,body{margin:0;padding:0}\
body{height:8000px;background:linear-gradient(180deg,red,teal,navy,white,crimson)}\
div.row{height:80px;border-bottom:2px solid rgba(0,0,0,.5);color:white;font:24px/80px sans-serif;padding-left:24px}\
</style>\
<script>for(let i=0;i<100;i++){let d=document.createElement('div');d.className='row';d.textContent='row '+i;document.body.appendChild(d)}</script>";

/// Solid red page used by the pixel-content sanity test. If the
/// sidecar's paint barrier (T15) does its job, every pixel of the
/// viewport reads back as approximately (255, 0, 0, 255) in Servo's
/// gl::RGBA byte order. If the framebuffer is still being read before
/// Servo paints, every byte is 255 (the initial clear-to-white state)
/// and the assertion catches it.
const SOLID_RED_DATA_URL: &str =
    "data:text/html,<body style=\"margin:0;background:%23ff0000;height:4000px\">";

const SOLID_BLUE_DATA_URL: &str =
    "data:text/html,<body style=\"margin:0;background:%230000ff;height:4000px\">";

#[derive(Deserialize, Debug)]
struct LiveResponse {
    error: Option<String>,
    frame: Option<LiveFrameReport>,
    #[serde(default)]
    perf: Option<FramePerfSummary>,
    #[serde(default)]
    surface_handle: Option<BenchSurfaceHandle>,
    #[serde(default)]
    current_surface_id: Option<u64>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct BenchSurfaceHandle {
    mach_port_name: u32,
    surface_id: u64,
    width: u32,
    height: u32,
}

#[derive(Deserialize, Debug)]
struct LiveFrameReport {
    rgba_byte_count: usize,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}

#[derive(Deserialize, Debug, Clone)]
struct FramePerfSummary {
    window: u32,
    context: String,
    paint_p50_us: u64,
    paint_p95_us: u64,
    paint_p99_us: u64,
    encode_p50_us: u64,
    encode_p95_us: u64,
    encode_p99_us: u64,
    write_p50_us: u64,
    write_p95_us: u64,
    write_p99_us: u64,
    total_p50_us: u64,
    total_p95_us: u64,
    total_p99_us: u64,
}

#[test]
#[ignore = "manual bench: spawns sidecar, scrolls a data: URL for N frames"]
fn run_live_bench() -> Result<(), Box<dyn Error>> {
    let kind = env::var("ELY_PERF_KIND").unwrap_or_else(|_| "software".to_string());
    let frames: u32 = env::var("ELY_PERF_FRAMES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FRAMES);
    let url = env::var("ELY_PERF_URL").unwrap_or_else(|_| SCROLL_PAGE_DATA_URL.to_string());

    let profile_id = ProfileId::new();
    let tab = TabId::new();
    let profile_data_dir = env::temp_dir().join(format!(
        "ely-perf-bench-{}-{}-{}",
        std::process::id(),
        kind,
        profile_id.as_str()
    ));
    fs::create_dir_all(&profile_data_dir)?;

    let mut child = spawn_sidecar(&kind, &profile_data_dir)?;
    let mut stdin = child.stdin.take().ok_or("sidecar stdin missing")?;
    let stdout = child.stdout.take().ok_or("sidecar stdout missing")?;
    let mut reader = BufReader::new(stdout);

    let outcome =
        match drive_bench(&mut stdin, &mut reader, &kind, &tab, &profile_id, &url, frames) {
            Ok(outcome) => outcome,
            Err(error) => {
                drop(stdin);
                let _ = child.kill();
                cleanup(&profile_data_dir)?;
                return Err(error);
            }
        };

    drop(stdin);
    let _ = child.wait();
    cleanup(&profile_data_dir)?;

    print_summaries(&kind, frames, &outcome.summaries);
    print_surface_handles(&kind, &outcome.surface_handles);
    print_current_surface_summary(&kind, &outcome.current_surface_ids);
    eprintln!(
        "\n=== ELY_PERF_KIND={kind} rgba_bytes_received={} ===",
        outcome.rgba_bytes_received
    );
    assert!(
        !outcome.summaries.is_empty(),
        "expected at least one FramePerfSummary across {frames} frames"
    );
    if kind == "hardware" {
        assert!(
            !outcome.surface_handles.is_empty(),
            "hardware path must publish at least one IOSurface handle"
        );
        // Mach port dedup: the surfman attached swap chain on macOS
        // rotates between front+back surfaces, so we expect a SMALL
        // number of distinct mach port publishes — definitely not one
        // per frame. Anything close to `frames` is a regression to the
        // T10.3 starting state where dedup tracked only the last id.
        let max_expected = 8;
        assert!(
            outcome.surface_handles.len() <= max_expected,
            "expected at most {max_expected} IOSurface publishes (one per unique surface), \
             got {} — dedup regressed",
            outcome.surface_handles.len()
        );
        assert!(
            !outcome.current_surface_ids.is_empty(),
            "hardware path must report current_surface_id on every frame"
        );
        // T10.6: once the receiver samples the IOSurface directly,
        // the sidecar drops the RGBA payload. The initial navigate
        // response may still carry bytes (no current_surface_id yet
        // because surfman hasn't bound the painted surface), but the
        // steady-state per-frame cost must be zero.
        assert!(
            outcome.rgba_bytes_received < (frames as u64) * 1_024,
            "hardware path leaked {} RGBA bytes across {} frames \
             (expected ~0 — the wire-drop optimisation regressed)",
            outcome.rgba_bytes_received,
            frames + 1,
        );
    } else {
        assert!(
            outcome.surface_handles.is_empty(),
            "software path must never publish an IOSurface handle"
        );
        assert!(
            outcome.current_surface_ids.is_empty(),
            "software path must never report current_surface_id"
        );
        // Software path keeps streaming pixels — every frame must
        // carry a full RGBA payload.
        let viewport_bytes = (1024u64) * (768u64) * 4;
        assert!(
            outcome.rgba_bytes_received >= viewport_bytes,
            "software path delivered only {} bytes — expected at least one full frame ({})",
            outcome.rgba_bytes_received,
            viewport_bytes,
        );
    }
    Ok(())
}

struct BenchOutcome {
    summaries: Vec<FramePerfSummary>,
    surface_handles: Vec<BenchSurfaceHandle>,
    current_surface_ids: Vec<u64>,
    rgba_bytes_received: u64,
}

fn spawn_sidecar(kind: &str, profile_data_dir: &PathBuf) -> Result<Child, Box<dyn Error>> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"));
    command
        .arg("live")
        .arg("--profile-data-dir")
        .arg(profile_data_dir)
        .arg("--rendering-context")
        .arg(kind)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    Ok(command.spawn()?)
}

fn drive_bench(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    kind: &str,
    tab: &TabId,
    profile_id: &ProfileId,
    url: &str,
    frames: u32,
) -> Result<BenchOutcome, Box<dyn Error>> {
    let mut summaries = Vec::new();
    let mut surface_handles = Vec::new();
    let mut current_surface_ids = Vec::new();
    let mut rgba_bytes_received: u64 = 0;

    let navigate = build_ensure(tab, profile_id, url, 0, 0, false);
    write_request(stdin, &navigate)?;
    let response = read_response(reader, RESPONSE_TIMEOUT)?;
    record_summary(&response, kind, &mut summaries);
    record_surface_handle(&response, kind, &mut surface_handles);
    record_current_surface_id(&response, &mut current_surface_ids);
    rgba_bytes_received += response.frame.as_ref().map_or(0, |f| f.rgba_byte_count as u64);

    let mut accumulated_scroll = 0;
    for frame_index in 0..frames {
        let scroll_delta_y = if frame_index % 80 == 79 {
            -SCROLL_STEP_PX * 60
        } else {
            SCROLL_STEP_PX
        };
        accumulated_scroll += scroll_delta_y;
        let request = build_ensure(tab, profile_id, url, 0, scroll_delta_y, true);
        write_request(stdin, &request)?;
        let response = read_response(reader, RESPONSE_TIMEOUT)?;
        if let Some(error) = response.error.as_ref() {
            return Err(format!("sidecar error at frame {frame_index}: {error}").into());
        }
        record_summary(&response, kind, &mut summaries);
        record_surface_handle(&response, kind, &mut surface_handles);
        record_current_surface_id(&response, &mut current_surface_ids);
        rgba_bytes_received += response.frame.as_ref().map_or(0, |f| f.rgba_byte_count as u64);
    }
    let _ = accumulated_scroll;

    Ok(BenchOutcome { summaries, surface_handles, current_surface_ids, rgba_bytes_received })
}

fn record_surface_handle(
    response: &LiveResponse,
    kind: &str,
    surface_handles: &mut Vec<BenchSurfaceHandle>,
) {
    if let Some(handle) = response.surface_handle {
        eprintln!(
            "[iosurface {kind}] new surface_id=0x{:x} mach_port=0x{:x} {}x{}",
            handle.surface_id, handle.mach_port_name, handle.width, handle.height,
        );
        surface_handles.push(handle);
    }
}

fn record_current_surface_id(response: &LiveResponse, current_surface_ids: &mut Vec<u64>) {
    if let Some(surface_id) = response.current_surface_id {
        current_surface_ids.push(surface_id);
    }
}

fn print_surface_handles(kind: &str, surface_handles: &[BenchSurfaceHandle]) {
    eprintln!(
        "\n=== ELY_PERF_KIND={kind} iosurface_imports={} (one per unique surface) ===",
        surface_handles.len()
    );
    for (index, handle) in surface_handles.iter().enumerate() {
        eprintln!(
            "{:<4} surface_id=0x{:x} mach_port=0x{:x} {}x{}",
            index, handle.surface_id, handle.mach_port_name, handle.width, handle.height,
        );
    }
}

fn print_current_surface_summary(kind: &str, current_surface_ids: &[u64]) {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<u64, u32> = BTreeMap::new();
    for id in current_surface_ids {
        *counts.entry(*id).or_default() += 1;
    }
    eprintln!(
        "\n=== ELY_PERF_KIND={kind} current_surface_id histogram (per-frame selector) ===",
    );
    for (surface_id, count) in counts.iter() {
        eprintln!("surface_id=0x{:x} frames={}", surface_id, count);
    }
}

fn build_ensure(
    tab: &TabId,
    profile_id: &ProfileId,
    url: &str,
    scroll_dx: i32,
    scroll_dy: i32,
    include_hover: bool,
) -> String {
    let hover_x = if include_hover { Some(256u32) } else { None };
    let hover_y = if include_hover { Some(256u32) } else { None };
    let scroll_point = if scroll_dx != 0 || scroll_dy != 0 {
        Some((256u32, 256u32))
    } else {
        None
    };
    let hover_x_json = match hover_x {
        Some(value) => format!("{value}"),
        None => "null".to_string(),
    };
    let hover_y_json = match hover_y {
        Some(value) => format!("{value}"),
        None => "null".to_string(),
    };
    let scroll_point_x_json = match scroll_point {
        Some((x, _)) => format!("{x}"),
        None => "null".to_string(),
    };
    let scroll_point_y_json = match scroll_point {
        Some((_, y)) => format!("{y}"),
        None => "null".to_string(),
    };
    format!(
        r#"{{"type":"ensure","tab_id":"{tab}","profile_id":"{profile}","url":{url},"width":{w},"height":{h},"page_zoom_percent":100,"scroll_delta_x":{dx},"scroll_delta_y":{dy},"scroll_point_x":{sx},"scroll_point_y":{sy},"click_x":null,"click_y":null,"hover_x":{hx},"hover_y":{hy},"typed_text":null,"site_permissions":[]}}"#,
        tab = tab.as_str(),
        profile = profile_id.as_str(),
        url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string()),
        w = VIEWPORT_WIDTH,
        h = VIEWPORT_HEIGHT,
        dx = scroll_dx,
        dy = scroll_dy,
        sx = scroll_point_x_json,
        sy = scroll_point_y_json,
        hx = hover_x_json,
        hy = hover_y_json,
    )
}

fn write_request(stdin: &mut ChildStdin, request: &str) -> Result<(), Box<dyn Error>> {
    stdin.write_all(request.as_bytes())?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn read_response(
    reader: &mut BufReader<ChildStdout>,
    timeout: Duration,
) -> Result<LiveResponse, Box<dyn Error>> {
    Ok(read_response_with_bytes(reader, timeout)?.0)
}

fn read_response_with_bytes(
    reader: &mut BufReader<ChildStdout>,
    timeout: Duration,
) -> Result<(LiveResponse, Vec<u8>), Box<dyn Error>> {
    let started_at = Instant::now();
    let mut json_line = String::new();
    loop {
        json_line.clear();
        let read_bytes = reader.read_line(&mut json_line)?;
        if read_bytes == 0 {
            return Err("sidecar closed stdout".into());
        }
        if json_line.trim().is_empty() {
            if started_at.elapsed() >= timeout {
                return Err("sidecar response timeout".into());
            }
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        break;
    }
    let response: LiveResponse = serde_json::from_str(json_line.trim_end())?;
    let mut rgba = Vec::new();
    if let Some(frame) = response.frame.as_ref()
        && frame.rgba_byte_count > 0
    {
        rgba.resize(frame.rgba_byte_count, 0);
        reader.read_exact(&mut rgba)?;
    }
    Ok((response, rgba))
}

fn record_summary(response: &LiveResponse, kind: &str, summaries: &mut Vec<FramePerfSummary>) {
    if let Some(perf) = response.perf.as_ref() {
        assert_eq!(
            perf.context, kind,
            "sidecar context label must match requested kind"
        );
        eprintln!(
            "[perf {kind}] window={} paint p50/p95/p99={}/{}/{} encode {}/{}/{} write {}/{}/{} total {}/{}/{} (µs)",
            perf.window,
            perf.paint_p50_us, perf.paint_p95_us, perf.paint_p99_us,
            perf.encode_p50_us, perf.encode_p95_us, perf.encode_p99_us,
            perf.write_p50_us, perf.write_p95_us, perf.write_p99_us,
            perf.total_p50_us, perf.total_p95_us, perf.total_p99_us,
        );
        summaries.push(perf.clone());
    }
}

fn print_summaries(kind: &str, frames: u32, summaries: &[FramePerfSummary]) {
    eprintln!("\n=== ELY_PERF_KIND={kind} frames={frames} windows={} ===", summaries.len());
    eprintln!(
        "{:<8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "win",
        "paint50", "paint95", "paint99",
        "enc50", "enc95", "enc99",
        "wr50", "wr95", "wr99",
        "tot50", "tot95", "tot99",
    );
    for summary in summaries {
        eprintln!(
            "{:<8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            summary.window,
            summary.paint_p50_us, summary.paint_p95_us, summary.paint_p99_us,
            summary.encode_p50_us, summary.encode_p95_us, summary.encode_p99_us,
            summary.write_p50_us, summary.write_p95_us, summary.write_p99_us,
            summary.total_p50_us, summary.total_p95_us, summary.total_p99_us,
        );
    }
}

fn cleanup(profile_data_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    match fs::remove_dir_all(profile_data_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// End-to-end pixel-content test. Drives the sidecar with a solid-red
/// HTML page, reads the frame off the wire, samples a handful of
/// pixels from the centre of the viewport, and asserts the RGBA
/// matches red. Catches three regressions in one shot:
///   * T15 paint barrier — if `read_to_image` runs before Servo
///     paints, every pixel is the framebuffer's clear-to-white state
///     `(255, 255, 255, 255)` and the red assertion fires.
///   * T13 hidpi — if the viewport is mis-scaled, the body might not
///     fill the canvas and the centre pixel would sample whatever's
///     outside.
///   * General pipeline rot — confirms `build_ensure` + JSON wire +
///     RGBA byte stream still delivers the bytes Servo painted.
#[test]
#[ignore = "drives a real sidecar via stdin/stdout; takes a few seconds"]
fn red_data_url_yields_red_rgba() -> Result<(), Box<dyn Error>> {
    assert_solid_color_renders("software", SOLID_RED_DATA_URL, ColorTarget::Red)?;
    Ok(())
}

#[test]
#[ignore = "drives a real sidecar via stdin/stdout; takes a few seconds"]
fn blue_data_url_yields_blue_rgba() -> Result<(), Box<dyn Error>> {
    assert_solid_color_renders("software", SOLID_BLUE_DATA_URL, ColorTarget::Blue)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum ColorTarget {
    Red,
    Blue,
}

impl ColorTarget {
    fn label(self) -> &'static str {
        match self {
            ColorTarget::Red => "red",
            ColorTarget::Blue => "blue",
        }
    }
}

fn assert_solid_color_renders(
    kind: &str,
    url: &str,
    target: ColorTarget,
) -> Result<(), Box<dyn Error>> {
    let profile_id = ProfileId::new();
    let tab = TabId::new();
    let profile_data_dir = env::temp_dir().join(format!(
        "ely-pixel-{}-{}-{}",
        std::process::id(),
        target.label(),
        profile_id.as_str(),
    ));
    fs::create_dir_all(&profile_data_dir)?;

    let mut child = spawn_sidecar(kind, &profile_data_dir)?;
    let mut stdin = child.stdin.take().ok_or("sidecar stdin missing")?;
    let stdout = child.stdout.take().ok_or("sidecar stdout missing")?;
    let mut reader = BufReader::new(stdout);

    let outcome = drive_solid_color_render(&mut stdin, &mut reader, &tab, &profile_id, url, target);

    drop(stdin);
    let _ = child.wait();
    cleanup(&profile_data_dir)?;

    outcome
}

fn drive_solid_color_render(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    tab: &TabId,
    profile_id: &ProfileId,
    url: &str,
    target: ColorTarget,
) -> Result<(), Box<dyn Error>> {
    // The navigate response itself is the one most likely to carry
    // real pixels — the sidecar's `awaiting_visible_frame` is armed
    // on a new URL and `poll_frame` will wait inside its own budget
    // for Servo to paint. Send navigate, capture the bytes, then
    // drive scroll iterations to give Servo additional repaint
    // opportunities. The first matching frame wins.
    let mut bytes = Vec::new();
    let mut report = None;
    for iteration in 0..30 {
        let scroll_y = if iteration == 0 {
            0
        } else if iteration % 2 == 1 {
            1
        } else {
            -1
        };
        let request = build_ensure(tab, profile_id, url, 0, scroll_y, false);
        write_request(stdin, &request)?;
        let (response, response_bytes) = read_response_with_bytes(reader, RESPONSE_TIMEOUT)?;
        if let Some(error) = response.error.as_ref() {
            return Err(format!("sidecar error: {error}").into());
        }
        if let Some(frame_report) = response.frame {
            if !response_bytes.is_empty()
                && sample_matches_target(
                    &response_bytes,
                    frame_report.width,
                    frame_report.height,
                    target,
                )
            {
                report = Some(frame_report);
                bytes = response_bytes;
                break;
            }
            if !response_bytes.is_empty() {
                bytes = response_bytes;
                report = Some(frame_report);
            }
        }
    }

    let report = report.ok_or("never received a frame with bytes")?;
    let width = report.width as usize;
    let height = report.height as usize;
    assert_eq!(
        bytes.len(),
        width * height * 4,
        "rgba byte count must match width × height × 4",
    );

    // Sample 9 evenly-spaced points in the inner quartile of the
    // viewport. Solid backgrounds should pass every sample; if Servo
    // is still painting initial-white we'll see (255, 255, 255, 255)
    // across the grid and the per-pixel asserts will explain.
    let mut samples = Vec::new();
    for fy in [1, 2, 3] {
        for fx in [1, 2, 3] {
            let x = width * fx / 4;
            let y = height * fy / 4;
            let idx = (y * width + x) * 4;
            samples.push((x, y, bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3]));
        }
    }
    eprintln!("[pixel sample {}] {:?}", target.label(), samples);

    let mut hits = 0;
    for (_x, _y, r, g, b, _a) in &samples {
        if matches_color(*r, *g, *b, target) {
            hits += 1;
        }
    }
    assert!(
        hits >= 5,
        "expected ≥5/9 centre-quadrant pixels to be {} after rendering {}; got samples {:?}",
        target.label(),
        url,
        samples,
    );
    Ok(())
}

fn sample_matches_target(bytes: &[u8], width: u32, height: u32, target: ColorTarget) -> bool {
    let w = width as usize;
    let h = height as usize;
    if bytes.len() < w * h * 4 || w == 0 || h == 0 {
        return false;
    }
    let cx = w / 2;
    let cy = h / 2;
    let idx = (cy * w + cx) * 4;
    matches_color(bytes[idx], bytes[idx + 1], bytes[idx + 2], target)
}

fn matches_color(r: u8, g: u8, b: u8, target: ColorTarget) -> bool {
    match target {
        ColorTarget::Red => r >= 200 && g <= 60 && b <= 60,
        ColorTarget::Blue => r <= 60 && g <= 60 && b >= 200,
    }
}
