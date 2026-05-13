use std::{env, num::ParseIntError, path::PathBuf};

use ely_domain::{
    DEFAULT_ZOOM_PERCENT, ProfileId, SiteOrigin, SitePermissionDecision, SitePermissionFeature,
    UrlText, validate_zoom_percent,
};
use ely_servo_host::RenderingContextKind;
use serde::Deserialize;
use thiserror::Error;

pub(super) enum SidecarCommand {
    Live(LiveArgs),
    Snapshot(SnapshotArgs),
}

pub(super) struct LiveArgs {
    pub(super) profile_data_dir: PathBuf,
    /// Rendering context the host's webviews are built against.
    /// Defaults to [`RenderingContextKind::Software`], which keeps
    /// the binary's behaviour bit-identical to pre-flag builds.
    /// `Hardware` is only accepted when the `hardware-render`
    /// feature is compiled in (otherwise the `SoftwareServoHost`
    /// constructor returns `HardwareRenderUnavailable`).
    pub(super) rendering_context_kind: RenderingContextKind,
}

pub(super) struct SnapshotArgs {
    pub(super) url: UrlText,
    pub(super) profile_id: ProfileId,
    pub(super) profile_data_dir: PathBuf,
    pub(super) rgba_out: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scroll_x: i32,
    pub(super) scroll_y: i32,
    pub(super) page_zoom_percent: u16,
    pub(super) click_point: Option<ClickPoint>,
    pub(super) drag_points: Option<DragPoints>,
    pub(super) touch_point: Option<ClickPoint>,
    pub(super) typed_text: Option<String>,
    pub(super) site_permissions: Vec<SidecarSitePermission>,
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

pub(super) struct SidecarSitePermission {
    pub(super) origin: SiteOrigin,
    pub(super) feature: SitePermissionFeature,
    pub(super) decision: SitePermissionDecision,
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

    #[error("{name} path is empty")]
    EmptyPath { name: &'static str },

    #[error("invalid --site-permission JSON: {value}")]
    InvalidSitePermissionJson {
        value: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "invalid --rendering-context value: {value:?} (expected \"software\" or \
         \"hardware\")"
    )]
    InvalidRenderingContext { value: String },

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
        "live" => parse_live_args(args).map(SidecarCommand::Live),
        "snapshot" => parse_snapshot_args(args).map(SidecarCommand::Snapshot),
        _ => Err(SidecarArgsError::UnknownCommand { value: command }),
    }
}

fn parse_live_args(args: impl IntoIterator<Item = String>) -> Result<LiveArgs, SidecarArgsError> {
    let mut args = args.into_iter();
    let mut profile_data_dir = None;
    let mut rendering_context_kind = RenderingContextKind::default();

    while let Some(name) = args.next() {
        match name.as_str() {
            "--profile-data-dir" => {
                profile_data_dir = Some(parse_path(
                    "--profile-data-dir",
                    next_argument(&mut args, "--profile-data-dir")?,
                )?)
            }
            "--rendering-context" => {
                let value = next_argument(&mut args, "--rendering-context")?;
                rendering_context_kind = match value.as_str() {
                    "software" => RenderingContextKind::Software,
                    "hardware" => RenderingContextKind::Hardware,
                    _ => return Err(SidecarArgsError::InvalidRenderingContext { value }),
                };
            }
            _ => return Err(SidecarArgsError::UnknownArgument { value: name }),
        }
    }

    Ok(LiveArgs {
        profile_data_dir: profile_data_dir
            .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--profile-data-dir" })?,
        rendering_context_kind,
    })
}

fn parse_snapshot_args(
    args: impl IntoIterator<Item = String>,
) -> Result<SnapshotArgs, SidecarArgsError> {
    let mut args = args.into_iter();
    let mut url = None;
    let mut profile_id = None;
    let mut profile_data_dir = None;
    let mut rgba_out = None;
    let mut width = None;
    let mut height = None;
    let mut scroll_x = 0;
    let mut scroll_y = 0;
    let mut page_zoom_percent = DEFAULT_ZOOM_PERCENT;
    let mut click_x = None;
    let mut click_y = None;
    let mut drag_from_x = None;
    let mut drag_from_y = None;
    let mut drag_to_x = None;
    let mut drag_to_y = None;
    let mut touch_x = None;
    let mut touch_y = None;
    let mut typed_text = None;
    let mut site_permissions = Vec::new();

    while let Some(name) = args.next() {
        match name.as_str() {
            "--url" => url = Some(UrlText::parse(next_argument(&mut args, "--url")?)?),
            "--profile-id" => {
                profile_id = Some(ProfileId::parse(next_argument(&mut args, "--profile-id")?)?)
            }
            "--profile-data-dir" => {
                profile_data_dir = Some(parse_path(
                    "--profile-data-dir",
                    next_argument(&mut args, "--profile-data-dir")?,
                )?)
            }
            "--rgba-out" => {
                rgba_out = Some(parse_path("--rgba-out", next_argument(&mut args, "--rgba-out")?)?)
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
            "--page-zoom-percent" => {
                page_zoom_percent = parse_zoom_percent(
                    "--page-zoom-percent",
                    next_argument(&mut args, "--page-zoom-percent")?,
                )?
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
            "--site-permission" => site_permissions
                .push(parse_site_permission(next_argument(&mut args, "--site-permission")?)?),
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
        profile_id: profile_id
            .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--profile-id" })?,
        profile_data_dir: profile_data_dir
            .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--profile-data-dir" })?,
        rgba_out: rgba_out
            .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--rgba-out" })?,
        width: width.ok_or(SidecarArgsError::MissingRequiredArgument { name: "--width" })?,
        height: height.ok_or(SidecarArgsError::MissingRequiredArgument { name: "--height" })?,
        scroll_x,
        scroll_y,
        page_zoom_percent,
        click_point,
        drag_points,
        touch_point,
        typed_text,
        site_permissions,
    })
}

#[derive(Deserialize)]
struct SitePermissionArg {
    origin: String,
    feature: String,
    decision: String,
}

fn parse_site_permission(value: String) -> Result<SidecarSitePermission, SidecarArgsError> {
    let parsed: SitePermissionArg = serde_json::from_str(&value)
        .map_err(|source| SidecarArgsError::InvalidSitePermissionJson { value, source })?;

    Ok(SidecarSitePermission {
        origin: SiteOrigin::parse(parsed.origin)?,
        feature: SitePermissionFeature::parse(parsed.feature.as_str())?,
        decision: SitePermissionDecision::parse(parsed.decision.as_str())?,
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

fn parse_zoom_percent(name: &'static str, value: String) -> Result<u16, SidecarArgsError> {
    let percent = value.parse::<u16>().map_err(|source| SidecarArgsError::InvalidInteger {
        name,
        value,
        source,
    })?;
    Ok(validate_zoom_percent(percent)?)
}

fn parse_path(name: &'static str, value: String) -> Result<PathBuf, SidecarArgsError> {
    if value.trim().is_empty() {
        return Err(SidecarArgsError::EmptyPath { name });
    }

    Ok(PathBuf::from(value))
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
