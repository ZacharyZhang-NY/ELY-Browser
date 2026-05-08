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
const SERVO_CLICK_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EClick%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Clicked%27%3Bthis.textContent%3D%27Clicked%27%3B%22%3ETap%3C%2Fbutton%3E";
const SERVO_CLICK_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
const SERVO_CLICK_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
const SERVO_TOUCH_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3ETouch%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3Btouch-action%3Amanipulation%3B%7D%3C%2Fstyle%3E%3Cbutton%20ontouchstart%3D%22document.body.dataset.touch%3D%27start%27%3B%22%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Touched%27%3Bthis.textContent%3D%27Touched%27%3B%22%3ETap%3C%2Fbutton%3E";
const SERVO_TOUCH_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
const SERVO_TOUCH_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
const SERVO_TEXT_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EText%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3Bfont%3A28px%20sans-serif%3B%7Dinput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A260px%3Bheight%3A70px%3Bfont%3A28px%20sans-serif%3B%7Doutput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A180px%3Bfont%3A32px%20sans-serif%3B%7D%3C%2Fstyle%3E%3Cinput%20id%3Dq%20autofocus%20oninput%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.getElementById%28%27out%27%29.textContent%3Dthis.value%3B%22%3E%3Coutput%20id%3Dout%3Eempty%3C%2Foutput%3E";
const SERVO_TEXT_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
const SERVO_TEXT_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
const SERVO_TEXT_VALUE: &str = "ely42";

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

#[derive(Clone, Copy)]
struct ClickPoint {
    x: u64,
    y: u64,
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

#[test]
fn sidecar_clicks_page_with_servo_mouse_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_click_probe(None)?;
    let clicked_report = snapshot_click_probe(Some(SERVO_CLICK_POINT))?;

    assert_eq!(report_field_as_u64(&clicked_report, "click_x")?, SERVO_CLICK_POINT.x);
    assert_eq!(report_field_as_u64(&clicked_report, "click_y")?, SERVO_CLICK_POINT.y);
    assert!(report_field_as_bool(&clicked_report, "click_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&clicked_report, "sample_hash")?
    );

    Ok(())
}

#[test]
fn sidecar_touches_page_with_servo_touch_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_touch_probe(None)?;
    let touched_report = snapshot_touch_probe(Some(SERVO_TOUCH_POINT))?;

    assert_eq!(report_field_as_u64(&touched_report, "touch_x")?, SERVO_TOUCH_POINT.x);
    assert_eq!(report_field_as_u64(&touched_report, "touch_y")?, SERVO_TOUCH_POINT.y);
    assert!(report_field_as_bool(&touched_report, "touch_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&touched_report, "sample_hash")?
    );

    Ok(())
}

#[test]
fn sidecar_types_text_with_servo_keyboard_input() -> Result<(), Box<dyn Error>> {
    let initial_report = snapshot_text_probe(None)?;
    let typed_report = snapshot_text_probe(Some(SERVO_TEXT_VALUE))?;

    assert_eq!(
        report_field_as_u64(&typed_report, "typed_text_byte_count")?,
        SERVO_TEXT_VALUE.len() as u64
    );
    assert!(report_field_as_bool(&typed_report, "text_changed_frame")?);
    assert_ne!(
        report_field_as_u64(&initial_report, "sample_hash")?,
        report_field_as_u64(&typed_report, "sample_hash")?
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

    let output =
        run_sidecar_snapshot(case.url, &output_path, size, scroll_offset, None, None, None)?;

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

fn snapshot_click_probe(
    click_point: Option<ClickPoint>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let output_path = std::env::temp_dir().join(format!(
        "ely-servo-sidecar-{}-click-{}x{}.rgba",
        std::process::id(),
        SERVO_CLICK_SIZE.width,
        SERVO_CLICK_SIZE.height
    ));

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output = run_sidecar_snapshot(
        SERVO_CLICK_URL,
        &output_path,
        SERVO_CLICK_SIZE,
        ScrollOffset::ZERO,
        click_point,
        None,
        None,
    )?;

    assert!(
        output.status.success(),
        "click probe\nstatus: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, SERVO_CLICK_SIZE.width);
    assert_eq!(report_field_as_u64(&report, "height")?, SERVO_CLICK_SIZE.height);
    assert!(report_field_as_u64(&report, "content_pixel_count")? > 0);
    assert_eq!(
        std::fs::metadata(&output_path)?.len(),
        SERVO_CLICK_SIZE.width * SERVO_CLICK_SIZE.height * 4
    );

    std::fs::remove_file(&output_path)?;
    Ok(report)
}

fn snapshot_touch_probe(
    touch_point: Option<ClickPoint>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let output_path = std::env::temp_dir().join(format!(
        "ely-servo-sidecar-{}-touch-{}x{}.rgba",
        std::process::id(),
        SERVO_TOUCH_SIZE.width,
        SERVO_TOUCH_SIZE.height
    ));

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let output = run_sidecar_snapshot(
        SERVO_TOUCH_URL,
        &output_path,
        SERVO_TOUCH_SIZE,
        ScrollOffset::ZERO,
        None,
        touch_point,
        None,
    )?;

    assert!(
        output.status.success(),
        "touch probe\nstatus: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, SERVO_TOUCH_SIZE.width);
    assert_eq!(report_field_as_u64(&report, "height")?, SERVO_TOUCH_SIZE.height);
    assert!(report_field_as_u64(&report, "content_pixel_count")? > 0);
    assert_eq!(
        std::fs::metadata(&output_path)?.len(),
        SERVO_TOUCH_SIZE.width * SERVO_TOUCH_SIZE.height * 4
    );

    std::fs::remove_file(&output_path)?;
    Ok(report)
}

fn snapshot_text_probe(typed_text: Option<&str>) -> Result<serde_json::Value, Box<dyn Error>> {
    let output_path = std::env::temp_dir().join(format!(
        "ely-servo-sidecar-{}-text-{}x{}.rgba",
        std::process::id(),
        SERVO_TEXT_SIZE.width,
        SERVO_TEXT_SIZE.height
    ));

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    let click_point = typed_text.map(|_| SERVO_TEXT_POINT);
    let output = run_sidecar_snapshot(
        SERVO_TEXT_URL,
        &output_path,
        SERVO_TEXT_SIZE,
        ScrollOffset::ZERO,
        click_point,
        None,
        typed_text,
    )?;

    assert!(
        output.status.success(),
        "text probe\nstatus: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report_field_as_u64(&report, "width")?, SERVO_TEXT_SIZE.width);
    assert_eq!(report_field_as_u64(&report, "height")?, SERVO_TEXT_SIZE.height);
    assert!(report_field_as_u64(&report, "content_pixel_count")? > 0);
    assert_eq!(
        std::fs::metadata(&output_path)?.len(),
        SERVO_TEXT_SIZE.width * SERVO_TEXT_SIZE.height * 4
    );

    std::fs::remove_file(&output_path)?;
    Ok(report)
}

fn run_sidecar_snapshot(
    site_url: &str,
    output_path: &std::path::Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
    click_point: Option<ClickPoint>,
    touch_point: Option<ClickPoint>,
    typed_text: Option<&str>,
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
    if let Some(click_point) = click_point {
        command.arg("--click-x").arg(click_point.x.to_string());
        command.arg("--click-y").arg(click_point.y.to_string());
    }
    if let Some(touch_point) = touch_point {
        command.arg("--touch-x").arg(touch_point.x.to_string());
        command.arg("--touch-y").arg(touch_point.y.to_string());
    }
    if let Some(typed_text) = typed_text {
        command.arg("--type-text").arg(typed_text);
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
