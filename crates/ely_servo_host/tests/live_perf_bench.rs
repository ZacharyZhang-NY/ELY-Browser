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
body{height:8000px;background:linear-gradient(180deg,#ff6b6b,#4ecdc4,#1d3557,#f1faee,#e63946)}\
div.row{height:80px;border-bottom:2px solid #0008;color:#fff;font:24px/80px sans-serif;padding-left:24px}\
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
    Ok(())
}

struct BenchOutcome {
    summaries: Vec<FramePerfSummary>,
    surface_handles: Vec<BenchSurfaceHandle>,
    current_surface_ids: Vec<u64>,
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

    let navigate = build_ensure(tab, profile_id, url, 0, 0, false);
    write_request(stdin, &navigate)?;
    let response = read_response(reader, RESPONSE_TIMEOUT)?;
    record_summary(&response, kind, &mut summaries);
    record_surface_handle(&response, kind, &mut surface_handles);
    record_current_surface_id(&response, &mut current_surface_ids);

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
    }
    let _ = accumulated_scroll;

    Ok(BenchOutcome { summaries, surface_handles, current_surface_ids })
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
    let hover_x_json = match hover_x {
        Some(value) => format!("{value}"),
        None => "null".to_string(),
    };
    let hover_y_json = match hover_y {
        Some(value) => format!("{value}"),
        None => "null".to_string(),
    };
    format!(
        r#"{{"type":"ensure","tab_id":"{tab}","profile_id":"{profile}","url":{url},"width":{w},"height":{h},"page_zoom_percent":100,"scroll_delta_x":{dx},"scroll_delta_y":{dy},"click_x":null,"click_y":null,"hover_x":{hx},"hover_y":{hy},"typed_text":null,"site_permissions":[]}}"#,
        tab = tab.as_str(),
        profile = profile_id.as_str(),
        url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string()),
        w = VIEWPORT_WIDTH,
        h = VIEWPORT_HEIGHT,
        dx = scroll_dx,
        dy = scroll_dy,
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
    if let Some(frame) = response.frame.as_ref() {
        if frame.rgba_byte_count > 0 {
            let mut scratch = vec![0u8; frame.rgba_byte_count];
            reader.read_exact(&mut scratch)?;
        }
    }
    Ok(response)
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
