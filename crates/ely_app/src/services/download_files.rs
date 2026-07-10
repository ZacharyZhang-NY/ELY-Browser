use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadFileAction {
    Open,
    Reveal,
}

#[derive(Debug, Error)]
pub enum DownloadFileError {
    #[error("download file is unavailable: {path:?}")]
    FileUnavailable { path: PathBuf },

    #[error("failed to launch {action} for download file: {path:?}: {source}")]
    LaunchFailed { action: &'static str, path: PathBuf, source: io::Error },

    #[error("{action} failed for download file: {path:?} ({status})")]
    CommandFailed { action: &'static str, path: PathBuf, status: ExitStatus },
}

impl DownloadFileAction {
    pub fn run(self, path: &Path) -> Result<(), DownloadFileError> {
        ensure_regular_file(path)?;
        let mut command = self.command(path);
        let launch_failed = |source| DownloadFileError::LaunchFailed {
            action: self.label(),
            path: path.to_path_buf(),
            source,
        };

        // Windows Explorer returns a nonzero exit code even when it opens
        // the folder, so for Reveal there we only require that it launched.
        if !self.exit_status_is_meaningful() {
            return command.spawn().map(|_| ()).map_err(launch_failed);
        }

        let status = command.status().map_err(launch_failed)?;
        if status.success() {
            return Ok(());
        }
        Err(DownloadFileError::CommandFailed {
            action: self.label(),
            path: path.to_path_buf(),
            status,
        })
    }

    /// The OS launcher for the current platform: `open` on macOS,
    /// `cmd /C start` / `explorer /select,` on Windows, `xdg-open` on
    /// Linux and other Unixes (revealing the containing folder, since
    /// there is no portable "select in file manager").
    fn command(self, path: &Path) -> Command {
        #[cfg(target_os = "macos")]
        {
            let mut command = Command::new("/usr/bin/open");
            match self {
                Self::Open => command.arg(path),
                Self::Reveal => command.arg("-R").arg(path),
            };
            command
        }
        #[cfg(target_os = "windows")]
        {
            match self {
                Self::Open => {
                    let mut command = Command::new("cmd");
                    command.arg("/C").arg("start").arg("").arg(path);
                    command
                }
                Self::Reveal => {
                    let mut select = std::ffi::OsString::from("/select,");
                    select.push(path);
                    let mut command = Command::new("explorer");
                    command.arg(select);
                    command
                }
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let mut command = Command::new("xdg-open");
            match self {
                Self::Open => command.arg(path),
                Self::Reveal => command.arg(path.parent().unwrap_or(path)),
            };
            command
        }
    }

    fn exit_status_is_meaningful(self) -> bool {
        !(cfg!(target_os = "windows") && matches!(self, Self::Reveal))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reveal => "reveal",
        }
    }
}

fn ensure_regular_file(path: &Path) -> Result<(), DownloadFileError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) | Err(_) => Err(DownloadFileError::FileUnavailable { path: path.to_path_buf() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program_and_args(action: DownloadFileAction, path: &Path) -> (String, Vec<String>) {
        let command = action.command(path);
        let program = command.get_program().to_string_lossy().into_owned();
        let args = command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
        (program, args)
    }

    #[test]
    fn open_launches_the_platform_handler_for_the_file() {
        let (program, args) = program_and_args(DownloadFileAction::Open, Path::new("/tmp/a.pdf"));
        #[cfg(target_os = "macos")]
        {
            assert_eq!(program, "/usr/bin/open");
            assert_eq!(args, vec!["/tmp/a.pdf"]);
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(program, "cmd");
            assert_eq!(args, vec!["/C", "start", "", "/tmp/a.pdf"]);
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec!["/tmp/a.pdf"]);
        }
    }

    #[test]
    fn reveal_targets_the_file_or_its_containing_folder() {
        let (program, args) = program_and_args(DownloadFileAction::Reveal, Path::new("/tmp/a.pdf"));
        #[cfg(target_os = "macos")]
        {
            assert_eq!(program, "/usr/bin/open");
            assert_eq!(args, vec!["-R", "/tmp/a.pdf"]);
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(program, "explorer");
            assert_eq!(args, vec!["/select,/tmp/a.pdf"]);
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec!["/tmp"]);
        }
    }

    #[test]
    fn only_windows_reveal_skips_the_exit_status_check() {
        assert!(DownloadFileAction::Open.exit_status_is_meaningful());
        assert_eq!(
            DownloadFileAction::Reveal.exit_status_is_meaningful(),
            !cfg!(target_os = "windows"),
        );
    }
}
