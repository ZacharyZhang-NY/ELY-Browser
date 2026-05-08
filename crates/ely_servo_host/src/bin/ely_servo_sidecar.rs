use std::{
    env,
    num::ParseIntError,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, NavigationRequest, RenderedFrame, ScrollRequest,
    ServoHost, ServoHostError, ServoSurfaceSize, SoftwareServoHost, WebViewSnapshot, WebViewState,
};
use serde::Serialize;
use thiserror::Error;

const WAIT_ITERATIONS: usize = 5_000;
const WAIT_INTERVAL: Duration = Duration::from_millis(2);
const RENDER_TIMEOUT: Duration = Duration::from_secs(20);
const INPUT_SETTLE_TIMEOUT: Duration = Duration::from_millis(700);

fn main() -> Result<(), SidecarError> {
    match parse_command(env::args())? {
        SidecarCommand::Snapshot(args) => run_snapshot(args),
    }
}

enum SidecarCommand {
    Snapshot(SnapshotArgs),
}

struct SnapshotArgs {
    url: UrlText,
    rgba_out: PathBuf,
    width: u32,
    height: u32,
    scroll_x: i32,
    scroll_y: i32,
    click_point: Option<ClickPoint>,
    typed_text: Option<String>,
}

#[derive(Clone, Copy)]
struct ClickPoint {
    x: u32,
    y: u32,
}

#[derive(Debug, Error)]
enum SidecarError {
    #[error("missing sidecar command")]
    MissingCommand,

    #[error("unknown sidecar command: {value}")]
    UnknownCommand { value: String },

    #[error("missing argument value for {name}")]
    MissingArgumentValue { name: &'static str },

    #[error("missing required argument: {name}")]
    MissingRequiredArgument { name: &'static str },

    #[error("unknown argument: {value}")]
    UnknownArgument { value: String },

    #[error("{name} must be an integer: {value}")]
    InvalidInteger {
        name: &'static str,
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("{name} must be greater than zero")]
    ZeroDimension { name: &'static str },

    #[error("--click-x and --click-y must be provided together")]
    IncompleteClickPoint,

    #[error("rgba output path is empty")]
    EmptyRgbaOutputPath,

    #[error("timed out rendering {url}: {snapshot:?}")]
    RenderTimeout { url: String, snapshot: Box<WebViewSnapshot> },

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),

    #[error(transparent)]
    Host(#[from] ServoHostError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn parse_command(args: impl IntoIterator<Item = String>) -> Result<SidecarCommand, SidecarError> {
    let mut args = args.into_iter();
    let _program_name = args.next();
    let command = args.next().ok_or(SidecarError::MissingCommand)?;

    match command.as_str() {
        "snapshot" => parse_snapshot_args(args).map(SidecarCommand::Snapshot),
        _ => Err(SidecarError::UnknownCommand { value: command }),
    }
}

fn parse_snapshot_args(
    args: impl IntoIterator<Item = String>,
) -> Result<SnapshotArgs, SidecarError> {
    let mut args = args.into_iter();
    let mut url = None;
    let mut rgba_out = None;
    let mut width = None;
    let mut height = None;
    let mut scroll_x = 0;
    let mut scroll_y = 0;
    let mut click_x = None;
    let mut click_y = None;
    let mut typed_text = None;

    while let Some(name) = args.next() {
        match name.as_str() {
            "--url" => url = Some(UrlText::parse(next_argument(&mut args, "--url")?)?),
            "--rgba-out" => {
                rgba_out = Some(parse_output_path(next_argument(&mut args, "--rgba-out")?)?)
            }
            "--width" => {
                width = Some(parse_dimension("--width", next_argument(&mut args, "--width")?)?)
            }
            "--height" => {
                height = Some(parse_dimension("--height", next_argument(&mut args, "--height")?)?)
            }
            "--scroll-x" => {
                scroll_x =
                    parse_scroll_delta("--scroll-x", next_argument(&mut args, "--scroll-x")?)?
            }
            "--scroll-y" => {
                scroll_y =
                    parse_scroll_delta("--scroll-y", next_argument(&mut args, "--scroll-y")?)?
            }
            "--click-x" => {
                click_x = Some(parse_click_coordinate(
                    "--click-x",
                    next_argument(&mut args, "--click-x")?,
                )?)
            }
            "--click-y" => {
                click_y = Some(parse_click_coordinate(
                    "--click-y",
                    next_argument(&mut args, "--click-y")?,
                )?)
            }
            "--type-text" => typed_text = Some(next_argument(&mut args, "--type-text")?),
            _ => return Err(SidecarError::UnknownArgument { value: name }),
        }
    }

    let click_point = match (click_x, click_y) {
        (Some(x), Some(y)) => Some(ClickPoint { x, y }),
        (None, None) => None,
        _ => return Err(SidecarError::IncompleteClickPoint),
    };

    Ok(SnapshotArgs {
        url: url.ok_or(SidecarError::MissingRequiredArgument { name: "--url" })?,
        rgba_out: rgba_out.ok_or(SidecarError::MissingRequiredArgument { name: "--rgba-out" })?,
        width: width.ok_or(SidecarError::MissingRequiredArgument { name: "--width" })?,
        height: height.ok_or(SidecarError::MissingRequiredArgument { name: "--height" })?,
        scroll_x,
        scroll_y,
        click_point,
        typed_text,
    })
}

fn next_argument(
    args: &mut impl Iterator<Item = String>,
    name: &'static str,
) -> Result<String, SidecarError> {
    args.next().ok_or(SidecarError::MissingArgumentValue { name })
}

fn parse_dimension(name: &'static str, value: String) -> Result<u32, SidecarError> {
    let dimension = value.parse::<u32>().map_err(|source| SidecarError::InvalidInteger {
        name,
        value,
        source,
    })?;
    if dimension == 0 {
        return Err(SidecarError::ZeroDimension { name });
    }

    Ok(dimension)
}

fn parse_scroll_delta(name: &'static str, value: String) -> Result<i32, SidecarError> {
    value.parse::<i32>().map_err(|source| SidecarError::InvalidInteger { name, value, source })
}

fn parse_click_coordinate(name: &'static str, value: String) -> Result<u32, SidecarError> {
    value.parse::<u32>().map_err(|source| SidecarError::InvalidInteger { name, value, source })
}

fn parse_output_path(value: String) -> Result<PathBuf, SidecarError> {
    if value.trim().is_empty() {
        return Err(SidecarError::EmptyRgbaOutputPath);
    }

    Ok(PathBuf::from(value))
}

fn run_snapshot(args: SnapshotArgs) -> Result<(), SidecarError> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(args.width, args.height))?;
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();
    let webview_id = host.create_webview(tab_id.clone(), profile_id)?;

    host.navigate(NavigationRequest {
        webview_id: webview_id.clone(),
        tab_id,
        url: args.url.clone(),
    })?;

    let snapshot = wait_for_frame(&mut host, &webview_id, args.url.as_str())?;
    let (snapshot, scroll_changed_frame) =
        apply_scroll_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, click_changed_frame) =
        apply_click_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, text_changed_frame) =
        apply_text_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let frame = host.last_rendered_frame()?;
    std::fs::write(&args.rgba_out, frame.rgba_bytes())?;

    serde_json::to_writer(
        std::io::stdout().lock(),
        &SnapshotReport::new(
            &args,
            &snapshot,
            &frame,
            scroll_changed_frame,
            click_changed_frame,
            text_changed_frame,
        ),
    )?;
    Ok(())
}

fn apply_scroll_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    if args.scroll_x == 0 && args.scroll_y == 0 {
        return Ok((snapshot, false));
    }

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.scroll(ScrollRequest {
        webview_id: webview_id.clone(),
        delta_x: args.scroll_x,
        delta_y: args.scroll_y,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_click_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(click_point) = args.click_point else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.click(MouseClickRequest {
        webview_id: webview_id.clone(),
        x: click_point.x,
        y: click_point.y,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_text_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(typed_text) = args.typed_text.as_ref() else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.type_text(KeyboardTextRequest {
        webview_id: webview_id.clone(),
        text: typed_text.clone(),
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn wait_for_frame(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    url: &str,
) -> Result<WebViewSnapshot, SidecarError> {
    let started_at = Instant::now();
    for _ in 0..WAIT_ITERATIONS {
        if started_at.elapsed() >= RENDER_TIMEOUT {
            break;
        }

        host.tick();
        let snapshot = host.snapshot(webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(webview_id)?;
        }

        let snapshot = host.snapshot(webview_id)?;
        let has_rendered_frame =
            host.last_rendered_frame().is_ok_and(|frame| frame.non_white_pixel_count() > 0);
        if snapshot.state() == &WebViewState::Complete && has_rendered_frame {
            return Ok(snapshot);
        }

        thread::sleep(WAIT_INTERVAL);
    }

    Err(SidecarError::RenderTimeout {
        url: url.to_string(),
        snapshot: Box::new(host.snapshot(webview_id)?),
    })
}

fn wait_for_changed_or_settled_frame(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: u64,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let started_at = Instant::now();
    let mut latest_snapshot = host.snapshot(webview_id)?;

    for _ in 0..WAIT_ITERATIONS {
        if started_at.elapsed() >= INPUT_SETTLE_TIMEOUT {
            break;
        }

        host.tick();
        let snapshot = host.snapshot(webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(webview_id)?;
        }

        latest_snapshot = host.snapshot(webview_id)?;
        let changed_frame = host.last_rendered_frame().is_ok_and(|frame| {
            frame.non_white_pixel_count() > 0 && frame.sample_hash() != previous_frame_hash
        });
        if changed_frame {
            return Ok((latest_snapshot, true));
        }

        thread::sleep(WAIT_INTERVAL);
    }

    Ok((latest_snapshot, false))
}

#[derive(Serialize)]
struct SnapshotReport {
    requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    rgba_path: String,
    state: &'static str,
    width: u32,
    height: u32,
    rgba_byte_count: usize,
    opaque_pixel_count: u64,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    sample_hash: u64,
    scroll_x: i32,
    scroll_y: i32,
    scroll_changed_frame: bool,
    click_x: Option<u32>,
    click_y: Option<u32>,
    click_changed_frame: bool,
    typed_text_byte_count: usize,
    text_changed_frame: bool,
}

impl SnapshotReport {
    fn new(
        args: &SnapshotArgs,
        snapshot: &WebViewSnapshot,
        frame: &RenderedFrame,
        scroll_changed_frame: bool,
        click_changed_frame: bool,
        text_changed_frame: bool,
    ) -> Self {
        Self {
            requested_url: args.url.as_str().to_string(),
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            rgba_path: args.rgba_out.display().to_string(),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            rgba_byte_count: frame.rgba_bytes().len(),
            opaque_pixel_count: frame.opaque_pixel_count(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
            scroll_x: args.scroll_x,
            scroll_y: args.scroll_y,
            scroll_changed_frame,
            click_x: args.click_point.map(|point| point.x),
            click_y: args.click_point.map(|point| point.y),
            click_changed_frame,
            typed_text_byte_count: args.typed_text.as_ref().map_or(0, String::len),
            text_changed_frame,
        }
    }
}

fn state_label(state: &WebViewState) -> &'static str {
    match state {
        WebViewState::Created => "created",
        WebViewState::Loading => "loading",
        WebViewState::Complete => "complete",
        WebViewState::Sleeping => "sleeping",
        WebViewState::Crashed => "crashed",
    }
}
