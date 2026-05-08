use std::{
    env,
    num::ParseIntError,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    NavigationRequest, RenderedFrame, ServoHost, ServoHostError, ServoSurfaceSize,
    SoftwareServoHost, WebViewSnapshot, WebViewState,
};
use serde::Serialize;
use thiserror::Error;

const WAIT_ITERATIONS: usize = 5_000;
const WAIT_INTERVAL: Duration = Duration::from_millis(2);
const RENDER_TIMEOUT: Duration = Duration::from_secs(20);

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

    #[error("{name} must be a positive integer: {value}")]
    InvalidInteger {
        name: &'static str,
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("{name} must be greater than zero")]
    ZeroDimension { name: &'static str },

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
            _ => return Err(SidecarError::UnknownArgument { value: name }),
        }
    }

    Ok(SnapshotArgs {
        url: url.ok_or(SidecarError::MissingRequiredArgument { name: "--url" })?,
        rgba_out: rgba_out.ok_or(SidecarError::MissingRequiredArgument { name: "--rgba-out" })?,
        width: width.ok_or(SidecarError::MissingRequiredArgument { name: "--width" })?,
        height: height.ok_or(SidecarError::MissingRequiredArgument { name: "--height" })?,
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
    let frame = host.last_rendered_frame()?;
    std::fs::write(&args.rgba_out, frame.rgba_bytes())?;

    serde_json::to_writer(
        std::io::stdout().lock(),
        &SnapshotReport::new(args.url.as_str(), &args.rgba_out, &snapshot, &frame),
    )?;
    Ok(())
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
}

impl SnapshotReport {
    fn new(
        requested_url: &str,
        rgba_path: &std::path::Path,
        snapshot: &WebViewSnapshot,
        frame: &RenderedFrame,
    ) -> Self {
        Self {
            requested_url: requested_url.to_string(),
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            rgba_path: rgba_path.display().to_string(),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            rgba_byte_count: frame.rgba_bytes().len(),
            opaque_pixel_count: frame.opaque_pixel_count(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
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
