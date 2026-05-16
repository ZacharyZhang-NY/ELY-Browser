//! Manual sidecar perf bench; ignored by normal CI.
#![cfg(feature = "servo-engine")]

#[path = "live_perf_bench/pixels.rs"]
mod pixels;

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
    #[serde(default)]
    device_pixel_ratio: f32,
    #[serde(default)]
    css_viewport_width: u32,
    #[serde(default)]
    css_viewport_height: u32,
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

    let outcome = match drive_bench(&mut stdin, &mut reader, &kind, &tab, &profile_id, &url, frames)
    {
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
        "\n=== ELY_PERF_KIND={kind} readback_rgba_bytes={} surface_rgba_bytes={} ===",
        outcome.readback_rgba_bytes, outcome.surface_rgba_bytes,
    );
    assert!(
        !outcome.summaries.is_empty(),
        "expected at least one FramePerfSummary across {frames} frames"
    );
    if kind == "hardware" {
        assert!(
            !outcome.surface_handles.is_empty(),
            "hardware live path must publish IOSurface handles"
        );
        assert!(
            !outcome.current_surface_ids.is_empty(),
            "hardware live path must report current_surface_id selectors"
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
    }
    let viewport_bytes = (1024u64) * (768u64) * 4;
    let total_rgba_bytes = outcome.readback_rgba_bytes + outcome.surface_rgba_bytes;
    assert!(
        total_rgba_bytes >= viewport_bytes,
        "{kind} path delivered only {total_rgba_bytes} bytes — expected at least one full frame ({})",
        viewport_bytes,
    );
    if kind == "hardware" {
        let full_readback_budget = viewport_bytes * u64::from(frames);
        assert!(
            total_rgba_bytes < full_readback_budget,
            "hardware path stayed on full readback: {total_rgba_bytes} >= {full_readback_budget}"
        );
    }
    Ok(())
}

struct BenchOutcome {
    summaries: Vec<FramePerfSummary>,
    surface_handles: Vec<BenchSurfaceHandle>,
    current_surface_ids: Vec<u64>,
    readback_rgba_bytes: u64,
    surface_rgba_bytes: u64,
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
    let mut readback_rgba_bytes: u64 = 0;
    let mut surface_rgba_bytes: u64 = 0;

    let navigate = build_ensure(tab, profile_id, url, 0, 0, false);
    write_request(stdin, &navigate)?;
    let response = read_response(reader, RESPONSE_TIMEOUT)?;
    assert_frame_viewport_report(&response);
    record_summary(&response, kind, &mut summaries);
    record_surface_handle(&response, kind, &mut surface_handles);
    record_current_surface_id(&response, &mut current_surface_ids);
    record_rgba_bytes(&response, &mut readback_rgba_bytes, &mut surface_rgba_bytes);

    let mut accumulated_scroll = 0;
    let mut painted_frames = 0;
    let max_attempts = frames.saturating_mul(4).max(frames + 10);
    for attempt in 0..max_attempts {
        if painted_frames >= frames {
            break;
        }
        let scroll_delta_y =
            if painted_frames % 80 == 79 { -SCROLL_STEP_PX * 60 } else { SCROLL_STEP_PX };
        accumulated_scroll += scroll_delta_y;
        let request = build_ensure(tab, profile_id, url, 0, scroll_delta_y, true);
        write_request(stdin, &request)?;
        let response = read_response(reader, RESPONSE_TIMEOUT)?;
        if let Some(error) = response.error.as_ref() {
            return Err(format!("sidecar error at attempt {attempt}: {error}").into());
        }
        if response.frame.is_some() {
            painted_frames += 1;
        }
        assert_frame_viewport_report(&response);
        record_summary(&response, kind, &mut summaries);
        record_surface_handle(&response, kind, &mut surface_handles);
        record_current_surface_id(&response, &mut current_surface_ids);
        record_rgba_bytes(&response, &mut readback_rgba_bytes, &mut surface_rgba_bytes);
    }
    assert_eq!(painted_frames, frames, "bench did not receive the requested painted frame count");
    let _ = accumulated_scroll;
    for _ in 0..5 {
        if !summaries.is_empty() {
            break;
        }
        let poll = build_poll(tab);
        write_request(stdin, &poll)?;
        let response = read_response(reader, RESPONSE_TIMEOUT)?;
        record_summary(&response, kind, &mut summaries);
    }

    Ok(BenchOutcome {
        summaries,
        surface_handles,
        current_surface_ids,
        readback_rgba_bytes,
        surface_rgba_bytes,
    })
}

fn record_rgba_bytes(
    response: &LiveResponse,
    readback_rgba_bytes: &mut u64,
    surface_rgba_bytes: &mut u64,
) {
    let rgba_byte_count = response.frame.as_ref().map_or(0, |frame| frame.rgba_byte_count as u64);
    if rgba_byte_count > 0 {
        *readback_rgba_bytes += rgba_byte_count;
    } else if response.current_surface_id.is_some() {
        *surface_rgba_bytes += rgba_byte_count;
    }
}

fn assert_frame_viewport_report(response: &LiveResponse) {
    let Some(frame) = response.frame.as_ref() else {
        return;
    };
    let dpr = if frame.device_pixel_ratio.is_finite() && frame.device_pixel_ratio > 0.0 {
        frame.device_pixel_ratio
    } else {
        1.0
    };
    let expected_width = ((frame.width as f32) / dpr).round().max(1.0) as u32;
    let expected_height = ((frame.height as f32) / dpr).round().max(1.0) as u32;

    assert_eq!(
        frame.css_viewport_width, expected_width,
        "CSS viewport width must match physical width divided by DPR",
    );
    assert_eq!(
        frame.css_viewport_height, expected_height,
        "CSS viewport height must match physical height divided by DPR",
    );
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
    eprintln!("\n=== ELY_PERF_KIND={kind} current_surface_id histogram (per-frame selector) ===",);
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
    let scroll_point = if scroll_dx != 0 || scroll_dy != 0 { Some((256u32, 256u32)) } else { None };
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

fn build_poll(tab: &TabId) -> String {
    format!(r#"{{"type":"poll","tab_id":"{}"}}"#, tab.as_str())
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
        assert_eq!(perf.context, kind, "sidecar context label must match requested kind");
        eprintln!(
            "[perf {kind}] window={} paint p50/p95/p99={}/{}/{} encode {}/{}/{} write {}/{}/{} total {}/{}/{} (µs)",
            perf.window,
            perf.paint_p50_us,
            perf.paint_p95_us,
            perf.paint_p99_us,
            perf.encode_p50_us,
            perf.encode_p95_us,
            perf.encode_p99_us,
            perf.write_p50_us,
            perf.write_p95_us,
            perf.write_p99_us,
            perf.total_p50_us,
            perf.total_p95_us,
            perf.total_p99_us,
        );
        summaries.push(perf.clone());
    }
}

fn print_summaries(kind: &str, frames: u32, summaries: &[FramePerfSummary]) {
    eprintln!("\n=== ELY_PERF_KIND={kind} frames={frames} windows={} ===", summaries.len());
    for summary in summaries {
        eprintln!(
            "win={} paint={}/{}/{} encode={}/{}/{} write={}/{}/{} total={}/{}/{}",
            summary.window,
            summary.paint_p50_us,
            summary.paint_p95_us,
            summary.paint_p99_us,
            summary.encode_p50_us,
            summary.encode_p95_us,
            summary.encode_p99_us,
            summary.write_p50_us,
            summary.write_p95_us,
            summary.write_p99_us,
            summary.total_p50_us,
            summary.total_p95_us,
            summary.total_p99_us,
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
