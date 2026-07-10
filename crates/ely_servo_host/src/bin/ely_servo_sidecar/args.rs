use std::{env, path::PathBuf};

use thiserror::Error;

pub(super) enum SidecarCommand {
    Live(LiveArgs),
}

pub(super) struct LiveArgs {
    pub(super) profile_data_dir: PathBuf,
    pub(super) rendering_context: SidecarRenderingContext,
    pub(super) iosurface_mach_service: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SidecarRenderingContext {
    #[default]
    Software,
    Hardware,
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

    #[error("{name} path is empty")]
    EmptyPath { name: &'static str },

    #[error("invalid rendering context: {value}")]
    InvalidRenderingContext { value: String },

    #[error("{name} value is empty")]
    EmptyValue { name: &'static str },
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
        _ => Err(SidecarArgsError::UnknownCommand { value: command }),
    }
}

fn parse_live_args(args: impl IntoIterator<Item = String>) -> Result<LiveArgs, SidecarArgsError> {
    let mut args = args.into_iter();
    let mut profile_data_dir = None;
    let mut rendering_context = SidecarRenderingContext::Software;
    let mut iosurface_mach_service = None;
    while let Some(name) = args.next() {
        match name.as_str() {
            "--profile-data-dir" => {
                let value = next_argument(&mut args, "--profile-data-dir")?;
                profile_data_dir = Some(parse_path("--profile-data-dir", value)?);
            }
            "--rendering-context" => {
                let value = next_argument(&mut args, "--rendering-context")?;
                rendering_context = match value.as_str() {
                    "software" => SidecarRenderingContext::Software,
                    "hardware" => SidecarRenderingContext::Hardware,
                    _ => return Err(SidecarArgsError::InvalidRenderingContext { value }),
                };
            }
            "--iosurface-mach-service" => {
                let value = next_argument(&mut args, "--iosurface-mach-service")?;
                iosurface_mach_service = Some(parse_nonempty("--iosurface-mach-service", value)?);
            }
            _ => return Err(SidecarArgsError::UnknownArgument { value: name }),
        }
    }

    let profile_data_dir = profile_data_dir
        .ok_or(SidecarArgsError::MissingRequiredArgument { name: "--profile-data-dir" })?;
    Ok(LiveArgs { profile_data_dir, rendering_context, iosurface_mach_service })
}

fn next_argument(
    args: &mut impl Iterator<Item = String>,
    name: &'static str,
) -> Result<String, SidecarArgsError> {
    args.next().ok_or(SidecarArgsError::MissingArgumentValue { name })
}

fn parse_path(name: &'static str, value: String) -> Result<PathBuf, SidecarArgsError> {
    if value.trim().is_empty() {
        return Err(SidecarArgsError::EmptyPath { name });
    }
    Ok(PathBuf::from(value))
}

fn parse_nonempty(name: &'static str, value: String) -> Result<String, SidecarArgsError> {
    if value.trim().is_empty() {
        return Err(SidecarArgsError::EmptyValue { name });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_live_profile_data_directory() -> Result<(), SidecarArgsError> {
        let command = parse_command([
            "ely_servo_sidecar".to_string(),
            "live".to_string(),
            "--profile-data-dir".to_string(),
            "/tmp/ely-profile".to_string(),
        ])?;

        let SidecarCommand::Live(args) = command;
        assert_eq!(args.profile_data_dir, PathBuf::from("/tmp/ely-profile"));
        assert_eq!(args.rendering_context, SidecarRenderingContext::Software);
        assert_eq!(args.iosurface_mach_service, None);
        Ok(())
    }

    #[test]
    fn parses_hardware_rendering_context_and_mach_service() -> Result<(), SidecarArgsError> {
        let command = parse_command([
            "ely_servo_sidecar".to_string(),
            "live".to_string(),
            "--profile-data-dir".to_string(),
            "/tmp/ely-profile".to_string(),
            "--rendering-context".to_string(),
            "hardware".to_string(),
            "--iosurface-mach-service".to_string(),
            "com.ely.browser.iosurface.test".to_string(),
        ])?;

        let SidecarCommand::Live(args) = command;
        assert_eq!(args.rendering_context, SidecarRenderingContext::Hardware);
        assert_eq!(args.iosurface_mach_service.as_deref(), Some("com.ely.browser.iosurface.test"));
        Ok(())
    }

    #[test]
    fn requires_profile_data_directory() {
        let result = parse_command(["ely_servo_sidecar".to_string(), "live".to_string()]);

        assert!(matches!(
            result,
            Err(SidecarArgsError::MissingRequiredArgument { name: "--profile-data-dir" })
        ));
    }
}
