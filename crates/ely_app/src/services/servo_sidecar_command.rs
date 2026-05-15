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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SidecarRenderingContext {
    Software,
    Hardware,
}

impl SidecarRenderingContext {
    pub(super) fn cli_arg(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Hardware => "hardware",
        }
    }

    fn sidecar_features(self) -> &'static str {
        match self {
            Self::Software => SOFTWARE_SIDECAR_FEATURES,
            Self::Hardware => HARDWARE_SIDECAR_FEATURES,
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
    if adjacent_sidecar.is_file() && is_macos_app_bundle_exe_dir(exe_dir) {
        return Ok(SidecarCommandTarget::Binary(adjacent_sidecar));
    }
    let workspace_manifest = workspace_manifest_path();
    let workspace_target_sidecar =
        workspace_manifest.as_ref().and_then(|path| workspace_target_sidecar_path(path));
    let adjacent_is_workspace_target =
        workspace_target_sidecar.as_ref().is_some_and(|path| path == &adjacent_sidecar);
    let workspace_target_sidecar_exists =
        workspace_target_sidecar.as_ref().is_some_and(|path| path.is_file());
    let prefer_cargo_hardware_sidecar = rendering_context_from_env()
        == SidecarRenderingContext::Hardware
        && (adjacent_is_workspace_target || workspace_target_sidecar_exists)
        && workspace_manifest.as_ref().is_some_and(|path| path.is_file());
    if adjacent_sidecar.is_file() && !prefer_cargo_hardware_sidecar {
        return Ok(SidecarCommandTarget::Binary(adjacent_sidecar));
    }

    if let Some(manifest_path) = workspace_manifest {
        if let Some(target_sidecar) = workspace_target_sidecar
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

pub(super) fn rendering_context_from_env() -> SidecarRenderingContext {
    let raw = env::var(RENDERING_CONTEXT_ENV).ok();
    rendering_context_selection(raw.as_deref())
}

fn sidecar_features_from_env() -> &'static str {
    rendering_context_from_env().sidecar_features()
}

fn rendering_context_selection(raw: Option<&str>) -> SidecarRenderingContext {
    match raw.map(str::to_lowercase).as_deref() {
        Some("software") => SidecarRenderingContext::Software,
        Some("hardware") => SidecarRenderingContext::Hardware,
        _ => default_rendering_context(),
    }
}

fn default_rendering_context() -> SidecarRenderingContext {
    if cfg!(target_os = "macos") {
        SidecarRenderingContext::Hardware
    } else {
        SidecarRenderingContext::Software
    }
}

fn workspace_target_sidecar_path(manifest_path: &Path) -> Option<PathBuf> {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    Some(manifest_path.parent()?.join("target").join(profile).join(sidecar_binary_name()))
}

fn sidecar_binary_name() -> String {
    format!("ely_servo_sidecar{}", env::consts::EXE_SUFFIX)
}

fn is_macos_app_bundle_exe_dir(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "MacOS")
        && path
            .parent()
            .is_some_and(|contents| contents.file_name().is_some_and(|name| name == "Contents"))
        && path
            .parent()
            .and_then(Path::parent)
            .is_some_and(|bundle| bundle.extension().is_some_and(|extension| extension == "app"))
}

#[cfg(test)]
mod tests {
    use super::{
        HARDWARE_SIDECAR_FEATURES, SOFTWARE_SIDECAR_FEATURES, SidecarRenderingContext,
        is_macos_app_bundle_exe_dir, rendering_context_selection,
    };

    #[test]
    fn hardware_rendering_context_enables_hardware_sidecar_feature() {
        let context = rendering_context_selection(Some("hardware"));
        assert_eq!(context, SidecarRenderingContext::Hardware);
        assert_eq!(context.sidecar_features(), HARDWARE_SIDECAR_FEATURES);
        assert_eq!(
            rendering_context_selection(Some("HARDWARE")),
            SidecarRenderingContext::Hardware
        );
    }

    #[test]
    fn software_rendering_context_uses_software_sidecar_feature() {
        let context = rendering_context_selection(Some("software"));
        assert_eq!(context, SidecarRenderingContext::Software);
        assert_eq!(context.sidecar_features(), SOFTWARE_SIDECAR_FEATURES);
    }

    #[test]
    fn recognizes_macos_app_bundle_executable_directory() {
        assert!(is_macos_app_bundle_exe_dir(std::path::Path::new(
            "/tmp/ELY Browser.app/Contents/MacOS"
        )));
        assert!(!is_macos_app_bundle_exe_dir(std::path::Path::new("/tmp/target/debug")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn defaults_to_hardware_rendering_context_on_macos() {
        assert_eq!(rendering_context_selection(None), SidecarRenderingContext::Hardware);
        assert_eq!(rendering_context_selection(Some("garbage")), SidecarRenderingContext::Hardware);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn defaults_to_software_rendering_context_off_macos() {
        assert_eq!(rendering_context_selection(None), SidecarRenderingContext::Software);
        assert_eq!(rendering_context_selection(Some("garbage")), SidecarRenderingContext::Software);
    }
}
