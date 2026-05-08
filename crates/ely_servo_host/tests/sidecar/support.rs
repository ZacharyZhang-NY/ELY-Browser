use std::{
    error::Error,
    io,
    process::{Child, Command, Output, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

pub(super) const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const SIDECAR_TIMEOUT: Duration = Duration::from_secs(45);
const SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(20);
const SIDECAR_RETRY_INTERVAL: Duration = Duration::from_millis(250);
static SIDECAR_COMMAND_LOCK: Mutex<()> = Mutex::new(());
pub(super) const PRD_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase { url: "https://example.com", title_fragment: "Example Domain" },
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" },
];
pub(super) const PRD_REFERENCE_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase {
        url: "https://blog.google/products-and-platforms/products/chrome/new-chrome-productivity-features/",
        title_fragment: "Chrome",
    },
    PrdSiteCompatibilityCase {
        url: "https://www.microsoft.com/en-us/edge/features/vertical-tabs",
        title_fragment: "Microsoft Edge",
    },
    PrdSiteCompatibilityCase {
        url: "https://resources.arc.net/hc/en-us/articles/19230755904151-Favorites-Top-Tabs-Across-Every-Space",
        title_fragment: "Favorites",
    },
    PrdSiteCompatibilityCase {
        url: "https://resources.arc.net/hc/en-us/articles/19228855311127-Auto-Archive-Clean-as-you-go",
        title_fragment: "Auto Archive",
    },
    PrdSiteCompatibilityCase {
        url: "https://vivaldi.com/features/workspaces/",
        title_fragment: "Workspaces",
    },
    PrdSiteCompatibilityCase {
        url: "https://help.vivaldi.com/desktop/tabs/tab-tiling/",
        title_fragment: "Tab Tiling",
    },
    PrdSiteCompatibilityCase { url: "https://www.gpui.rs/", title_fragment: "gpui" },
    PrdSiteCompatibilityCase { url: "https://docs.rs/gpui/latest/gpui/", title_fragment: "gpui" },
    PrdSiteCompatibilityCase {
        url: "https://zed.dev/blog/videogame",
        title_fragment: "Leveraging Rust",
    },
    PrdSiteCompatibilityCase {
        url: "https://github.com/longbridge/gpui-component/",
        title_fragment: "gpui-component",
    },
    PrdSiteCompatibilityCase {
        url: "https://github.com/zed-industries/awesome-gpui/",
        title_fragment: "awesome-gpui",
    },
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" },
    PrdSiteCompatibilityCase {
        url: "https://servo.org/blog/2026/04/13/servo-0.1.0-release/",
        title_fragment: "Servo",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/d1/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/workers/platform/storage-options/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/kv/concepts/how-kv-works/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://better-auth.com/blog/1-5",
        title_fragment: "Better Auth",
    },
    PrdSiteCompatibilityCase {
        url: "https://developers.cloudflare.com/d1/platform/limits/",
        title_fragment: "Cloudflare",
    },
    PrdSiteCompatibilityCase {
        url: "https://component-model.bytecodealliance.org/",
        title_fragment: "WebAssembly Component Model",
    },
    PrdSiteCompatibilityCase {
        url: "https://docs.wasmtime.dev/api/wasmtime/component/index.html",
        title_fragment: "wasmtime",
    },
    PrdSiteCompatibilityCase {
        url: "https://docs.wasmtime.dev/security.html",
        title_fragment: "Wasmtime",
    },
];
pub(super) const PRD_SITE_COMPATIBILITY_SIZES: &[FrameSize] = &[
    FrameSize { width: 640, height: 480 },
    FrameSize { width: 934, height: 657 },
    FrameSize { width: 1614, height: 980 },
];
pub(super) const PRD_REFERENCE_SITE_SIZE: FrameSize = FrameSize { width: 934, height: 657 };
pub(super) const SERVO_SCROLL_SITE: PrdSiteCompatibilityCase =
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" };
pub(super) const SERVO_SCROLL_SIZE: FrameSize = FrameSize { width: 934, height: 657 };
pub(super) const SERVO_SCROLL_OFFSET: ScrollOffset = ScrollOffset { x: 0, y: 480 };
const SERVO_CLICK_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EClick%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Clicked%27%3Bthis.textContent%3D%27Clicked%27%3B%22%3ETap%3C%2Fbutton%3E";
const SERVO_CLICK_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_CLICK_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
const SERVO_DRAG_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3EDrag%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20id%3Dbox%3EDrag%3C%2Fbutton%3E%3Cscript%3Elet%20dragging%3Dfalse%3Bconst%20box%3Ddocument.getElementById%28%27box%27%29%3BaddEventListener%28%27mousedown%27%2Cevent%3D%3E%7Bif%28event.target%3D%3D%3Dbox%29%7Bdragging%3Dtrue%3B%7D%7D%29%3BaddEventListener%28%27mousemove%27%2Cevent%3D%3E%7Bif%28dragging%26%26event.clientX%3E280%29%7Bdocument.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Dragged%27%3Bbox.textContent%3D%27Dragged%27%3B%7D%7D%29%3BaddEventListener%28%27mouseup%27%2C%28%29%3D%3E%7Bdragging%3Dfalse%3B%7D%29%3B%3C%2Fscript%3E";
const SERVO_DRAG_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_DRAG_FROM: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_DRAG_TO: ClickPoint = ClickPoint { x: 320, y: 120 };
const SERVO_TOUCH_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3ETouch%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3Btouch-action%3Amanipulation%3B%7D%3C%2Fstyle%3E%3Cbutton%20ontouchstart%3D%22document.body.dataset.touch%3D%27start%27%3B%22%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Touched%27%3Bthis.textContent%3D%27Touched%27%3B%22%3ETap%3C%2Fbutton%3E";
const SERVO_TOUCH_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
pub(super) const SERVO_TOUCH_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
const SERVO_TEXT_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EText%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3Bfont%3A28px%20sans-serif%3B%7Dinput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A260px%3Bheight%3A70px%3Bfont%3A28px%20sans-serif%3B%7Doutput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A180px%3Bfont%3A32px%20sans-serif%3B%7D%3C%2Fstyle%3E%3Cinput%20id%3Dq%20autofocus%20oninput%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.getElementById%28%27out%27%29.textContent%3Dthis.value%3B%22%3E%3Coutput%20id%3Dout%3Eempty%3C%2Foutput%3E";
const SERVO_TEXT_SIZE: FrameSize = FrameSize { width: 640, height: 480 };
const SERVO_TEXT_POINT: ClickPoint = ClickPoint { x: 160, y: 120 };
pub(super) const SERVO_TEXT_VALUE: &str = "ely42";

pub(super) struct PrdSiteCompatibilityCase {
    pub(super) url: &'static str,
    title_fragment: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct FrameSize {
    pub(super) width: u64,
    height: u64,
}

#[derive(Clone, Copy)]
pub(super) struct ScrollOffset {
    pub(super) x: i64,
    pub(super) y: i64,
}

impl ScrollOffset {
    pub(super) const ZERO: Self = Self { x: 0, y: 0 };
}

#[derive(Clone, Copy)]
pub(super) struct ClickPoint {
    pub(super) x: u64,
    pub(super) y: u64,
}

#[derive(Clone, Copy)]
pub(super) struct DragPoints {
    pub(super) from: ClickPoint,
    pub(super) to: ClickPoint,
}

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
    output_path: &std::path::Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
    input: SnapshotInput<'_>,
) -> Result<Output, Box<dyn Error>> {
    let _guard = SIDECAR_COMMAND_LOCK
        .lock()
        .map_err(|_| io::Error::other("sidecar command lock poisoned"))?;
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

fn run_sidecar_snapshot_with_retry(
    site_url: &str,
    output_path: &std::path::Path,
    size: FrameSize,
    scroll_offset: ScrollOffset,
    input: SnapshotInput<'_>,
) -> Result<Output, Box<dyn Error>> {
    for attempt in 0..2 {
        if output_path.exists() {
            std::fs::remove_file(output_path)?;
        }

        match run_sidecar_snapshot(site_url, output_path, size, scroll_offset, input) {
            Ok(output) if output.status.success() => return Ok(output),
            Ok(output) if attempt == 1 => return Ok(output),
            Ok(_output) => {}
            Err(error) if attempt == 1 => return Err(error),
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

fn assert_report_state_is_renderable(report: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let state = report
        .get("state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "missing text report field: state".to_string())?;
    assert!(matches!(state, "complete" | "loading"), "state: {state}");
    Ok(())
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
