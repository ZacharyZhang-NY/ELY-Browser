#![cfg(feature = "servo-engine")]

use std::{
    error::Error,
    io,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const SIDECAR_TIMEOUT: Duration = Duration::from_secs(25);
const SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(20);
const PRD_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase { url: "https://example.com", title_fragment: "Example Domain" },
    PrdSiteCompatibilityCase { url: "https://servo.org", title_fragment: "Servo" },
];
const PRD_SITE_COMPATIBILITY_SIZES: &[FrameSize] = &[
    FrameSize { width: 640, height: 480 },
    FrameSize { width: 934, height: 657 },
    FrameSize { width: 1614, height: 980 },
];
const SERVO_SCROLL_SITE: PrdSiteCompatibilityCase =
    PrdSiteCompatibilityCase { url: "https://servo.org", title_fragment: "Servo" };
const SERVO_SCROLL_SIZE: FrameSize = FrameSize { width: 934, height: 657 };
const SERVO_SCROLL_OFFSET: ScrollOffset = ScrollOffset { x: 0, y: 480 };

struct PrdSiteCompatibilityCase {
    url: &'static str,
    title_fragment: &'static str,
}

#[derive(Clone, Copy)]
struct FrameSize {
    width: u64,
    height: u64,
}

#[derive(Clone, Copy)]
struct ScrollOffset {
    x: i64,
    y: i64,
}

impl ScrollOffset {
    const ZERO: Self = Self { x: 0, y: 0 };
}

#[test]
fn sidecar_snapshots_prd_sites_to_rgba_files() -> Result<(), Box<dyn Error>> {
    for case in PRD_SITE_COMPATIBILITY_CASES {
        for size in PRD_SITE_COMPATIBILITY_SIZES {
            snapshot_prd_site(case, *size, ScrollOffset::ZERO)?;
        }
    }

    Ok(())
}

#[test]
fn sidecar_scrolls_prd_site_with_servo_input() -> Result<(), Box<dyn Error>> {
    let initial_report =
        snapshot_prd_site(&SERVO_SCROLL_SITE, SERVO_SCROLL_SIZE, ScrollOffset::ZERO)?;
    let scrolled_report =
        snapshot_prd_site(&SERVO_SCROLL_SITE, SERVO_SCROLL_SIZE, SERVO_SCROLL_OFFSET)?;

    assert_eq!(report_field_as_i64(&scrolled_report, "scroll_x")?, SERVO_SCROLL_OFFSET.x);
    assert_eq!(report_field_as_i64(&scrolled_report, "scroll_y")?, SERVO_SCROLL_OFFSET.y);
    assert_eq!(
        report_field_as_u64(&scrolled_report, "width")?,
        SERVO_SCROLL_SIZE.width,
        "{}",
        SERVO_SCROLL_SITE.url
    );
    assert!(
        report_field_as_bool(&scrolled_report, "scroll_changed_frame")?,
        "{}",
        SERVO_SCROLL_SITE.url
    );
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&scrolled_report, "sample_hash")?,
        "{}",
        SERVO_SCROLL_SITE.url
    );

    Ok(())
}

fn snapshot_prd_site(
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

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output = run_sidecar_snapshot(case.url, &output_path, size, scroll_offset)?;

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
    assert_eq!(report_field_as_u64(&report, "width")?, size.width, "{}", case.url);
    assert_eq!(report_field_as_u64(&report, "height")?, size.height, "{}", case.url);
    assert_eq!(
        report_field_as_u64(&report, "rgba_byte_count")?,
        size.width * size.height * 4,
        "{}",
        case.url
    );
    assert_report_text_contains(&report, "loaded_url", case.url)?;
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

fn run_sidecar_snapshot(
    site_url: &str,
    output_path: &std::path::Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
) -> Result<Output, Box<dyn Error>> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"));
    command
        .arg("snapshot")
        .arg("--url")
        .arg(site_url)
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

    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let started_at = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(Into::into);
        }

        if started_at.elapsed() >= SIDECAR_TIMEOUT {
            terminate_child(child)?;
            return Err(format!(
                "timed out rendering {site_url} at {}x{}",
                size.width, size.height
            )
            .into());
        }

        thread::sleep(SIDECAR_POLL_INTERVAL);
    }
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

fn assert_report_text_contains(
    report: &serde_json::Value,
    field: &'static str,
    fragment: &str,
) -> Result<(), Box<dyn Error>> {
    let value = report
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("missing text report field: {field}"))?;
    assert!(value.contains(fragment), "{field}: {value}");
    Ok(())
}

fn report_field_as_bool(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<bool, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("missing boolean report field: {field}").into())
}

fn report_field_as_i64(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<i64, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("missing signed report field: {field}").into())
}

fn report_field_as_u64(
    report: &serde_json::Value,
    field: &'static str,
) -> Result<u64, Box<dyn Error>> {
    report
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("missing numeric report field: {field}").into())
}
