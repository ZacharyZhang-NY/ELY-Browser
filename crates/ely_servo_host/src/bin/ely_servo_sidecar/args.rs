use std::{env, num::ParseIntError, path::PathBuf};

use ely_domain::UrlText;
use thiserror::Error;

pub(super) enum SidecarCommand {
    Snapshot(SnapshotArgs),
}

pub(super) struct SnapshotArgs {
    pub(super) url: UrlText,
    pub(super) rgba_out: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scroll_x: i32,
    pub(super) scroll_y: i32,
    pub(super) click_point: Option<ClickPoint>,
    pub(super) drag_points: Option<DragPoints>,
    pub(super) touch_point: Option<ClickPoint>,
    pub(super) typed_text: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) struct ClickPoint {
    pub(super) x: u32,
    pub(super) y: u32,
}

#[derive(Clone, Copy)]
pub(super) struct DragPoints {
    pub(super) from: ClickPoint,
    pub(super) to: ClickPoint,
}

#[derive(Debug, Error)]
pub(super) enum SidecarArgsError {
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

    #[error("--drag-from-x, --drag-from-y, --drag-to-x, and --drag-to-y must be provided together")]
    IncompleteDragPoints,

    #[error("--touch-x and --touch-y must be provided together")]
    IncompleteTouchPoint,

    #[error("rgba output path is empty")]
    EmptyRgbaOutputPath,

    #[error(transparent)]
    Domain(#[from] ely_domain::DomainError),
}

pub(super) fn parse_env_command() -> Result<SidecarCommand, SidecarArgsError> {
    parse_command(env::args())
}

fn parse_command(
    args: impl IntoIterator<Item = String>,
) -> Result<SidecarCommand, SidecarArgsError> {
    let mut args = args.into_iter();
    let _program_name = args.next();
    let command = args.next().ok_or(SidecarArgsError::MissingCommand)?;

    match command.as_str() {
        "snapshot" => parse_snapshot_args(args).map(SidecarCommand::Snapshot),
        _ => Err(SidecarArgsError::UnknownCommand { value: command }),
    }
}

fn parse_snapshot_args(
    args: impl IntoIterator<Item = String>,
) -> Result<SnapshotArgs, SidecarArgsError> {
    let mut args = args.into_iter();
    let mut url = None;
    let mut rgba_out = None;
    let mut width = None;
    let mut height = None;
    let mut scroll_x = 0;
    let mut scroll_y = 0;
    let mut click_x = None;
    let mut click_y = None;
    let mut drag_from_x = None;
    let mut drag_from_y = None;
    let mut drag_to_x = None;
    let mut drag_to_y = None;
    let mut touch_x = None;
    let mut touch_y = None;
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
            "--drag-from-x" => {
                drag_from_x = Some(parse_click_coordinate(
                    "--drag-from-x",
                    next_argument(&mut args, "--drag-from-x")?,
                )?)
            }
            "--drag-from-y" => {
                drag_from_y = Some(parse_click_coordinate(
                    "--drag-from-y",
                    next_argument(&mut args, "--drag-from-y")?,
                )?)
            }
            "--drag-to-x" => {
                drag_to_x = Some(parse_click_coordinate(
                    "--drag-to-x",
                    next_argument(&mut args, "--drag-to-x")?,
                )?)
            }
            "--drag-to-y" => {
                drag_to_y = Some(parse_click_coordinate(
                    "--drag-to-y",
                    next_argument(&mut args, "--drag-to-y")?,
                )?)
            }
            "--touch-x" => {
                touch_x = Some(parse_click_coordinate(
                    "--touch-x",
                    next_argument(&mut args, "--touch-x")?,
                )?)
            }
            "--touch-y" => {
                touch_y = Some(parse_click_coordinate(
                    "--touch-y",
                    next_argument(&mut args, "--touch-y")?,
                )?)
            }
            "--type-text" => typed_text = Some(next_argument(&mut args, "--type-text")?),
            _ => return Err(SidecarArgsError::UnknownArgument { value: name }),
        }
    }

    let click_point = match (click_x, click_y) {
        (Some(x), Some(y)) => Some(ClickPoint { x, y }),
        (None, None) => None,
        _ => return Err(SidecarArgsError::IncompleteClickPoint),
    };
    let drag_points = match (drag_from_x, drag_from_y, drag_to_x, drag_to_y) {
        (Some(from_x), Some(from_y), Some(to_x), Some(to_y)) => Some(DragPoints {
            from: ClickPoint { x: from_x, y: from_y },
            to: ClickPoint { x: to_x, y: to_y },
        }),
        (None, None, None, None) => None,
        _ => return Err(SidecarArgsError::IncompleteDragPoints),
    };
    let touch_point = match (touch_x, touch_y) {
        (Some(x), Some(y)) => Some(ClickPoint { x, y }),
        (None, None) => None,
        _ => return Err(SidecarArgsError::IncompleteTouchPoint),
    };

    Ok(SnapshotArgs {
        url: url.ok_or(SidecarArgsError::MissingRequiredArgument { name: "--url" })?,
        rgba_out: rgba_out
            .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--rgba-out" })?,
        width: width.ok_or(SidecarArgsError::MissingRequiredArgument { name: "--width" })?,
        height: height.ok_or(SidecarArgsError::MissingRequiredArgument { name: "--height" })?,
        scroll_x,
        scroll_y,
        click_point,
        drag_points,
        touch_point,
        typed_text,
    })
}

fn next_argument(
    args: &mut impl Iterator<Item = String>,
    name: &'static str,
) -> Result<String, SidecarArgsError> {
    args.next().ok_or(SidecarArgsError::MissingArgumentValue { name })
}

fn parse_dimension(name: &'static str, value: String) -> Result<u32, SidecarArgsError> {
    let dimension = value.parse::<u32>().map_err(|source| SidecarArgsError::InvalidInteger {
        name,
        value,
        source,
    })?;
    if dimension == 0 {
        return Err(SidecarArgsError::ZeroDimension { name });
    }

    Ok(dimension)
}

fn parse_scroll_delta(name: &'static str, value: String) -> Result<i32, SidecarArgsError> {
    value.parse::<i32>().map_err(|source| SidecarArgsError::InvalidInteger { name, value, source })
}

fn parse_click_coordinate(name: &'static str, value: String) -> Result<u32, SidecarArgsError> {
    value.parse::<u32>().map_err(|source| SidecarArgsError::InvalidInteger { name, value, source })
}

fn parse_output_path(value: String) -> Result<PathBuf, SidecarArgsError> {
    if value.trim().is_empty() {
        return Err(SidecarArgsError::EmptyRgbaOutputPath);
    }

    Ok(PathBuf::from(value))
}
