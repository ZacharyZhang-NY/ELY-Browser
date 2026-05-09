use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use super::servo_sidecar::ServoSidecarError;

const SIDECAR_BINARY_TIMEOUT: Duration = Duration::from_secs(45);
const SIDECAR_CARGO_TIMEOUT: Duration = Duration::from_secs(180);
const SIDECAR_PATH_ENV: &str = "ELY_SERVO_SIDECAR";

#[derive(Clone, Debug)]
pub(super) enum SidecarCommandTarget {
    Binary(PathBuf),
    Cargo { manifest_path: PathBuf },
}

impl SidecarCommandTarget {
    pub(super) fn command(&self) -> Command {
        match self {
            Self::Binary(path) => Command::new(path),
            Self::Cargo { manifest_path } => {
                let mut command = Command::new("cargo");
                command
                    .arg("run")
                    .arg("--quiet")
                    .arg("--manifest-path")
                    .arg(manifest_path)
                    .arg("-p")
                    .arg("ely_servo_host")
                    .arg("--features")
                    .arg("servo-engine")
                    .arg("--bin")
                    .arg("ely_servo_sidecar")
                    .arg("--");
                command
            }
        }
    }

    pub(super) fn timeout(&self) -> Duration {
        match self {
            Self::Binary(_) => SIDECAR_BINARY_TIMEOUT,
            Self::Cargo { .. } => SIDECAR_CARGO_TIMEOUT,
        }
    }

    pub(super) fn missing_binary_path(&self) -> Option<&Path> {
        match self {
            Self::Binary(path) if !path.is_file() => Some(path.as_path()),
            Self::Binary(_) | Self::Cargo { .. } => None,
        }
    }
}

pub(super) fn default_sidecar_command() -> Result<SidecarCommandTarget, ServoSidecarError> {
    if let Some(path) = env::var_os(SIDECAR_PATH_ENV) {
        return Ok(SidecarCommandTarget::Binary(PathBuf::from(path)));
    }

    let current_exe = env::current_exe().map_err(ServoSidecarError::CurrentExecutable)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        ServoSidecarError::CurrentExecutableDirectoryUnavailable { path: current_exe.clone() }
    })?;
    let adjacent_sidecar = exe_dir.join(sidecar_binary_name());
    if adjacent_sidecar.is_file() {
        return Ok(SidecarCommandTarget::Binary(adjacent_sidecar));
    }

    if let Some(manifest_path) = workspace_manifest_path() {
        if let Some(target_sidecar) = workspace_target_sidecar_path(&manifest_path)
            && target_sidecar.is_file()
        {
            return Ok(SidecarCommandTarget::Binary(target_sidecar));
        }
        if manifest_path.is_file() {
            return Ok(SidecarCommandTarget::Cargo { manifest_path });
        }
    }

    Ok(SidecarCommandTarget::Binary(adjacent_sidecar))
}

fn workspace_manifest_path() -> Option<PathBuf> {
    option_env!("ELY_WORKSPACE_MANIFEST").map(PathBuf::from)
}

fn workspace_target_sidecar_path(manifest_path: &Path) -> Option<PathBuf> {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    Some(manifest_path.parent()?.join("target").join(profile).join(sidecar_binary_name()))
}

fn sidecar_binary_name() -> String {
    format!("ely_servo_sidecar{}", env::consts::EXE_SUFFIX)
}
