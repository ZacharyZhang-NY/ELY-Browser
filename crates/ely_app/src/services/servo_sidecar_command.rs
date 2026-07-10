use std::{
    env, io,
    path::{Path, PathBuf},
    process::Command,
};

use thiserror::Error;

const SIDECAR_PATH_ENV: &str = "ELY_SERVO_SIDECAR";
const RENDERING_CONTEXT_ENV: &str = "ELY_SERVO_RENDERING_CONTEXT";

#[derive(Clone, Debug)]
pub(super) struct SidecarCommandTarget(PathBuf);

impl SidecarCommandTarget {
    pub(super) fn command(&self) -> Command {
        Command::new(&self.0)
    }

    pub(super) fn missing_binary_path(&self) -> Option<&Path> {
        (!self.0.is_file()).then_some(self.0.as_path())
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
        return Ok(SidecarCommandTarget(PathBuf::from(path)));
    }

    let current_exe = env::current_exe().map_err(SidecarCommandError::CurrentExecutable)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        SidecarCommandError::CurrentExecutableDirectoryUnavailable { path: current_exe.clone() }
    })?;
    let adjacent_sidecar = exe_dir.join(sidecar_binary_name());
    let workspace_manifest = workspace_manifest_path();
    if adjacent_sidecar.is_file() || is_macos_app_bundle_exe_dir(exe_dir) {
        return Ok(SidecarCommandTarget(adjacent_sidecar));
    }

    if let Some(target_sidecar) =
        workspace_manifest.as_deref().and_then(workspace_target_sidecar_path)
    {
        return Ok(SidecarCommandTarget(target_sidecar));
    }

    Ok(SidecarCommandTarget(adjacent_sidecar))
}

pub(super) fn rendering_context_from_env() -> SidecarRenderingContext {
    let raw = env::var(RENDERING_CONTEXT_ENV).ok();
    rendering_context_selection(raw.as_deref())
}

fn rendering_context_selection(raw: Option<&str>) -> SidecarRenderingContext {
    match raw.map(str::to_lowercase).as_deref() {
        Some("software") => SidecarRenderingContext::Software,
        Some("hardware") => SidecarRenderingContext::Hardware,
        _ if cfg!(target_os = "macos") => SidecarRenderingContext::Hardware,
        _ => SidecarRenderingContext::Software,
    }
}

fn workspace_manifest_path() -> Option<PathBuf> {
    option_env!("ELY_WORKSPACE_MANIFEST").filter(|path| !path.is_empty()).map(PathBuf::from)
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
        SidecarRenderingContext, is_macos_app_bundle_exe_dir, rendering_context_selection,
        sidecar_binary_name, workspace_target_sidecar_path,
    };

    #[test]
    fn workspace_sidecar_path_uses_current_build_profile() {
        let manifest = std::path::Path::new("/workspace/Cargo.toml");
        let profile = if cfg!(debug_assertions) { "debug" } else { "release" };

        assert_eq!(
            workspace_target_sidecar_path(manifest),
            Some(
                std::path::Path::new("/workspace/target").join(profile).join(sidecar_binary_name())
            )
        );
    }

    #[test]
    fn rendering_context_follows_explicit_environment_selection() {
        let hardware = rendering_context_selection(Some("HARDWARE"));
        let software = rendering_context_selection(Some("software"));

        assert_eq!(hardware, SidecarRenderingContext::Hardware);
        assert_eq!(software, SidecarRenderingContext::Software);
    }

    #[test]
    fn recognizes_macos_app_bundle_executable_directory() {
        assert!(is_macos_app_bundle_exe_dir(std::path::Path::new(
            "/tmp/ELY Browser.app/Contents/MacOS"
        )));
        assert!(!is_macos_app_bundle_exe_dir(std::path::Path::new("/tmp/target/debug")));
    }
}
