use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use ely_domain::ProfileId;

pub(super) use super::site_cases::{
    ClickPoint, DragPoints, FrameSize, PRD_REFERENCE_SITE_COMPATIBILITY_CASES,
    PRD_REFERENCE_SITE_SIZE, PRD_SITE_COMPATIBILITY_CASES, PRD_SITE_COMPATIBILITY_SIZES,
    PrdSiteCompatibilityCase, SERVO_CLICK_POINT, SERVO_DRAG_FROM, SERVO_DRAG_TO,
    SERVO_SCROLL_OFFSET, SERVO_SCROLL_SITE, SERVO_SCROLL_SIZE, SERVO_TEXT_VALUE, SERVO_TOUCH_POINT,
    ScrollOffset,
};
use super::site_cases::{
    SERVO_CLICK_SIZE, SERVO_CLICK_URL, SERVO_DRAG_SIZE, SERVO_DRAG_URL, SERVO_TEXT_POINT,
    SERVO_TEXT_SIZE, SERVO_TEXT_URL, SERVO_TOUCH_SIZE, SERVO_TOUCH_URL,
};

pub(super) const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const SIDECAR_TIMEOUT: Duration = Duration::from_secs(45);
const SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(20);
const SIDECAR_COMMAND_COOLDOWN: Duration = Duration::from_millis(750);
const SIDECAR_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const SIDECAR_MAX_ATTEMPTS: usize = 3;
static SIDECAR_COMMAND_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Default)]
struct SnapshotInput<'a> {
    click_point: Option<ClickPoint>,
    drag_points: Option<DragPoints>,
    touch_point: Option<ClickPoint>,
    typed_text: Option<&'a str>,
}

pub(super) fn snapshot_prd_site(
    case: &PrdSiteCompatibilityCase,
    size: FrameSize,
    scroll_offset: ScrollOffset,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let site_name = case
        .url
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>();
    let output_path = std::env::temp_dir().join(format!(
        "ely-servo-sidecar-{}-{site_name}-{}x{}-{}-{}.rgba",
        std::process::id(),
        size.width,
        size.height,
        scroll_offset.x,
        scroll_offset.y
    ));

    let output = run_sidecar_snapshot_with_retry(
        case.url,
        &output_path,
        size,
        scroll_offset,
        SnapshotInput::default(),
    )?;

    assert!(
        output.status.success(),
        "{} {}x{}\nstatus: {:?}\nstdout: {}\nstderr: {}",
        case.url,
        size.width,
        size.height,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_report_state_is_renderable(&report)?;
    assert_eq!(report_field_as_u64(&report, "width")?, size.width, "{}", case.url);
    assert_eq!(report_field_as_u64(&report, "height")?, size.height, "{}", case.url);
    assert_eq!(
        report_field_as_u64(&report, "rgba_byte_count")?,
        size.width * size.height * 4,
        "{}",
        case.url
    );
    assert_report_text_contains(&report, "loaded_url", case.url)?;
    assert_report_text_equals(&report, "requested_url", case.url)?;
    assert_report_text_contains(&report, "title", case.title_fragment)?;
    assert!(report_field_as_u64(&report, "non_white_pixel_count")? > 0, "{}", case.url);
    assert!(
        report_field_as_u64(&report, "content_pixel_count")? >= MINIMUM_CONTENT_PIXELS,
        "{}",
        case.url
    );
    assert!(report_field_as_u64(&report, "sample_hash")? > 0, "{}", case.url);
    assert_eq!(std::fs::metadata(&output_path)?.len(), size.width * size.height * 4);

    std::fs::remove_file(&output_path)?;
    Ok(report)
}

pub(super) fn snapshot_click_probe(
    click_point: Option<ClickPoint>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    snapshot_probe(
        SERVO_CLICK_URL,
        SERVO_CLICK_SIZE,
        "click",
        SnapshotInput { click_point, ..SnapshotInput::default() },
    )
}

pub(super) fn snapshot_drag_probe(
    drag_points: Option<DragPoints>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    snapshot_probe(
        SERVO_DRAG_URL,
        SERVO_DRAG_SIZE,
        "drag",
        SnapshotInput { drag_points, ..SnapshotInput::default() },
    )
}

pub(super) fn snapshot_touch_probe(
    touch_point: Option<ClickPoint>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    snapshot_probe(
        SERVO_TOUCH_URL,
        SERVO_TOUCH_SIZE,
        "touch",
        SnapshotInput { touch_point, ..SnapshotInput::default() },
    )
}

pub(super) fn snapshot_text_probe(
    typed_text: Option<&str>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    snapshot_probe(
        SERVO_TEXT_URL,
        SERVO_TEXT_SIZE,
        "text",
        SnapshotInput {
            click_point: typed_text.map(|_| SERVO_TEXT_POINT),
            typed_text,
            ..SnapshotInput::default()
        },
    )
}

fn snapshot_probe(
    url: &str,
    size: FrameSize,
    label: &'static str,
    input: SnapshotInput<'_>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let output_path = std::env::temp_dir().join(format!(
        "ely-servo-sidecar-{}-{label}-{}x{}.rgba",
        std::process::id(),
        size.width,
        size.height
    ));

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output =
        run_sidecar_snapshot_with_retry(url, &output_path, size, ScrollOffset::ZERO, input)?;

    assert!(
        output.status.success(),
        "{label} probe\nstatus: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, size.width);
    assert_eq!(report_field_as_u64(&report, "height")?, size.height);
    assert!(report_field_as_u64(&report, "content_pixel_count")? > 0);
    assert_eq!(std::fs::metadata(&output_path)?.len(), size.width * size.height * 4);

    std::fs::remove_file(&output_path)?;
    Ok(report)
}

fn run_sidecar_snapshot(
    site_url: &str,
    output_path: &Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
    input: SnapshotInput<'_>,
) -> Result<Output, Box<dyn Error>> {
    let _guard = SIDECAR_COMMAND_LOCK
        .lock()
        .map_err(|_| io::Error::other("sidecar command lock poisoned"))?;
    let profile_id = ProfileId::new();
    let profile_data_dir = temporary_profile_data_dir(&profile_id);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"));
    command
        .arg("snapshot")
        .arg("--url")
        .arg(site_url)
        .arg("--profile-id")
        .arg(profile_id.as_str())
        .arg("--profile-data-dir")
        .arg(&profile_data_dir)
        .arg("--rgba-out")
        .arg(output_path)
        .arg("--width")
        .arg(size.width.to_string())
        .arg("--height")
        .arg(size.height.to_string());
    if scroll_offset.x != 0 {
        command.arg("--scroll-x").arg(scroll_offset.x.to_string());
    }
    if scroll_offset.y != 0 {
        command.arg("--scroll-y").arg(scroll_offset.y.to_string());
    }
    if let Some(click_point) = input.click_point {
        command.arg("--click-x").arg(click_point.x.to_string());
        command.arg("--click-y").arg(click_point.y.to_string());
    }
    if let Some(drag_points) = input.drag_points {
        command.arg("--drag-from-x").arg(drag_points.from.x.to_string());
        command.arg("--drag-from-y").arg(drag_points.from.y.to_string());
        command.arg("--drag-to-x").arg(drag_points.to.x.to_string());
        command.arg("--drag-to-y").arg(drag_points.to.y.to_string());
    }
    if let Some(touch_point) = input.touch_point {
        command.arg("--touch-x").arg(touch_point.x.to_string());
        command.arg("--touch-y").arg(touch_point.y.to_string());
    }
    if let Some(typed_text) = input.typed_text {
        command.arg("--type-text").arg(typed_text);
    }

    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let started_at = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            remove_temporary_dir(&profile_data_dir)?;
            thread::sleep(SIDECAR_COMMAND_COOLDOWN);
            return Ok(output);
        }

        if started_at.elapsed() >= SIDECAR_TIMEOUT {
            terminate_child(child)?;
            remove_temporary_dir(&profile_data_dir)?;
            thread::sleep(SIDECAR_COMMAND_COOLDOWN);
            return Err(format!(
                "timed out rendering {site_url} at {}x{}",
                size.width, size.height
            )
            .into());
        }

        thread::sleep(SIDECAR_POLL_INTERVAL);
    }
}

fn temporary_profile_data_dir(profile_id: &ProfileId) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ely-servo-sidecar-profile-{}-{}",
        std::process::id(),
        profile_id.as_str()
    ))
}

fn remove_temporary_dir(path: &Path) -> Result<(), Box<dyn Error>> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn run_sidecar_snapshot_with_retry(
    site_url: &str,
    output_path: &std::path::Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
    input: SnapshotInput<'_>,
) -> Result<Output, Box<dyn Error>> {
    for attempt in 0..SIDECAR_MAX_ATTEMPTS {
        if output_path.exists() {
            std::fs::remove_file(output_path)?;
        }

        match run_sidecar_snapshot(site_url, output_path, size, scroll_offset, input) {
            Ok(output) if output.status.success() => return Ok(output),
            Ok(output) if attempt + 1 == SIDECAR_MAX_ATTEMPTS => return Ok(output),
            Ok(_output) => {}
            Err(error) if attempt + 1 == SIDECAR_MAX_ATTEMPTS => return Err(error),
            Err(_error) => {}
        }

        thread::sleep(SIDECAR_RETRY_INTERVAL);
    }

    Err("sidecar snapshot retry did not produce output".into())
}

fn terminate_child(mut child: Child) -> Result<(), Box<dyn Error>> {
    match child.kill() {
        Ok(()) => {
            let _output = child.wait_with_output()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn assert_report_text_equals(
    report: &serde_json::Value,
    field: &'static str,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let value = report_field_as_text(report, field)?;
    assert_eq!(value, expected, "{field}");
    Ok(())
}

fn assert_report_text_contains(
    report: &serde_json::Value,
    field: &'static str,
    fragment: &str,
) -> Result<(), Box<dyn Error>> {
    let value = report_field_as_text(report, field)?;
    assert!(value.contains(fragment), "{field}: {value}");
    Ok(())
}

fn assert_report_state_is_renderable(report: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let state = report_field_as_text(report, "state")?;
    assert!(matches!(state, "complete" | "loading"), "state: {state}");
    Ok(())
}

fn report_field_as_text<'a>(
    report: &'a serde_json::Value,
    field: &'static str,
) -> Result<&'a str, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("missing text report field: {field}").into())
}

pub(super) fn report_field_as_bool(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<bool, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("missing boolean report field: {field}").into())
}

pub(super) fn report_field_as_i64(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<i64, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("missing signed report field: {field}").into())
}

pub(super) fn report_field_as_u64(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<u64, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("missing numeric report field: {field}").into())
}
