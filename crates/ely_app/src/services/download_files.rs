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
        let status =
            self.command(path).status().map_err(|source| DownloadFileError::LaunchFailed {
                action: self.label(),
                path: path.to_path_buf(),
                source,
            })?;

        if status.success() {
            return Ok(());
        }

        Err(DownloadFileError::CommandFailed {
            action: self.label(),
            path: path.to_path_buf(),
            status,
        })
    }

    fn command(self, path: &Path) -> Command {
        let mut command = Command::new("/usr/bin/open");
        match self {
            Self::Open => {
                command.arg(path);
            }
            Self::Reveal => {
                command.arg("-R").arg(path);
            }
        }
        command
    }

    fn label(self) -> &'static str {
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
