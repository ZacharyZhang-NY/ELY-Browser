use std::{
    env, io,
    path::{Path, PathBuf},
    process::Command,
};

use thiserror::Error;

const SIDECAR_PATH_ENV: &str = "ELY_SERVO_SIDECAR";
const RENDERING_CONTEXT_ENV: &str = "ELY_SERVO_RENDERING_CONTEXT";
const SOFTWARE_SIDECAR_FEATURES: &str = "servo-engine";
const HARDWARE_SIDECAR_FEATURES: &str = "servo-engine,hardware-render";

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
                    .arg(sidecar_features_from_env())
                    .arg("--bin")
                    .arg("ely_servo_sidecar")
                    .arg("--");
                command
            }
        }
    }

    pub(super) fn missing_binary_path(&self) -> Option<&Path> {
        match self {
            Self::Binary(path) if !path.is_file() => Some(path.as_path()),
            Self::Binary(_) | Self::Cargo { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum SidecarCommandError {
    #[error("current executable path is unavailable: {0}")]
    CurrentExecutable(#[source] io::Error),

    #[error("current executable directory is unavailable for {path}")]
    CurrentExecutableDirectoryUnavailable { path: PathBuf },
}

pub(super) fn default_sidecar_command() -> Result<SidecarCommandTarget, SidecarCommandError> {
    if let Some(path) = env::var_os(SIDECAR_PATH_ENV) {
        return Ok(SidecarCommandTarget::Binary(PathBuf::from(path)));
    }

    let current_exe = env::current_exe().map_err(SidecarCommandError::CurrentExecutable)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        SidecarCommandError::CurrentExecutableDirectoryUnavailable { path: current_exe.clone() }
    })?;
    let adjacent_sidecar = exe_dir.join(sidecar_binary_name());
    let workspace_manifest = workspace_manifest_path();
    let prefer_cargo_hardware_sidecar = hardware_rendering_context_requested()
        && workspace_manifest.as_ref().is_some_and(|path| path.is_file());
    if adjacent_sidecar.is_file() && !prefer_cargo_hardware_sidecar {
        return Ok(SidecarCommandTarget::Binary(adjacent_sidecar));
    }

    if let Some(manifest_path) = workspace_manifest {
        if let Some(target_sidecar) = workspace_target_sidecar_path(&manifest_path)
            && target_sidecar.is_file()
            && !prefer_cargo_hardware_sidecar
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

fn hardware_rendering_context_requested() -> bool {
    env::var(RENDERING_CONTEXT_ENV).ok().as_deref().is_some_and(rendering_context_requests_hardware)
}

fn sidecar_features_from_env() -> &'static str {
    env::var(RENDERING_CONTEXT_ENV)
        .ok()
        .as_deref()
        .map(sidecar_features_for_rendering_context)
        .unwrap_or(SOFTWARE_SIDECAR_FEATURES)
}

fn sidecar_features_for_rendering_context(raw: &str) -> &'static str {
    if rendering_context_requests_hardware(raw) {
        HARDWARE_SIDECAR_FEATURES
    } else {
        SOFTWARE_SIDECAR_FEATURES
    }
}

fn rendering_context_requests_hardware(raw: &str) -> bool {
    raw.eq_ignore_ascii_case("hardware")
}

fn workspace_target_sidecar_path(manifest_path: &Path) -> Option<PathBuf> {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    Some(manifest_path.parent()?.join("target").join(profile).join(sidecar_binary_name()))
}

fn sidecar_binary_name() -> String {
    format!("ely_servo_sidecar{}", env::consts::EXE_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::{
        HARDWARE_SIDECAR_FEATURES, SOFTWARE_SIDECAR_FEATURES,
        sidecar_features_for_rendering_context,
    };

    #[test]
    fn hardware_rendering_context_enables_hardware_sidecar_feature() {
        assert_eq!(sidecar_features_for_rendering_context("hardware"), HARDWARE_SIDECAR_FEATURES);
        assert_eq!(sidecar_features_for_rendering_context("HARDWARE"), HARDWARE_SIDECAR_FEATURES);
    }

    #[test]
    fn software_and_unknown_contexts_use_software_sidecar_feature() {
        assert_eq!(sidecar_features_for_rendering_context("software"), SOFTWARE_SIDECAR_FEATURES);
        assert_eq!(sidecar_features_for_rendering_context("garbage"), SOFTWARE_SIDECAR_FEATURES);
    }
}
