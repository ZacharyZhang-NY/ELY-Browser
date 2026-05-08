use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use ely_domain::UrlText;
use serde::Deserialize;
use thiserror::Error;

const SIDECAR_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Debug)]
pub struct ServoSidecarClient {
    binary_path: PathBuf,
}

impl ServoSidecarClient {
    pub fn new() -> Result<Self, ServoSidecarError> {
        Ok(Self { binary_path: default_sidecar_path()? })
    }

    pub fn snapshot(
        &self,
        request: SidecarSnapshotRequest,
    ) -> Result<SidecarSnapshot, ServoSidecarError> {
        if !self.binary_path.is_file() {
            return Err(ServoSidecarError::SidecarBinaryUnavailable {
                path: self.binary_path.clone(),
            });
        }

        let rgba_path = temporary_rgba_path()?;
        let output = match self.run_snapshot_command(&request, &rgba_path) {
            Ok(output) => output,
            Err(error) => {
                remove_temporary_file(&rgba_path)?;
                return Err(error);
            }
        };

        if !output.status.success() {
            remove_temporary_file(&rgba_path)?;
            return Err(ServoSidecarError::SidecarFailed {
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let snapshot = read_sidecar_snapshot(&output.stdout, &rgba_path);
        let cleanup = remove_temporary_file(&rgba_path);
        match (snapshot, cleanup) {
            (Ok(snapshot), Ok(())) => Ok(snapshot),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    fn run_snapshot_command(
        &self,
        request: &SidecarSnapshotRequest,
        rgba_path: &Path,
    ) -> Result<Output, ServoSidecarError> {
        let mut child = Command::new(&self.binary_path)
            .arg("snapshot")
            .arg("--url")
            .arg(request.url.as_str())
            .arg("--rgba-out")
            .arg(rgba_path)
            .arg("--width")
            .arg(request.width.to_string())
            .arg("--height")
            .arg(request.height.to_string())
            .arg("--scroll-x")
            .arg(request.scroll_x.to_string())
            .arg("--scroll-y")
            .arg(request.scroll_y.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ServoSidecarError::Command)?;

        let started_at = Instant::now();
        loop {
            if child.try_wait().map_err(ServoSidecarError::Command)?.is_some() {
                return child.wait_with_output().map_err(ServoSidecarError::Command);
            }

            if started_at.elapsed() >= SIDECAR_COMMAND_TIMEOUT {
                terminate_child(child)?;
                return Err(ServoSidecarError::SidecarTimedOut {
                    url: request.url.as_str().to_string(),
                    seconds: SIDECAR_COMMAND_TIMEOUT.as_secs(),
                });
            }

            thread::sleep(SIDECAR_POLL_INTERVAL);
        }
    }
}

#[derive(Clone, Debug)]
pub struct SidecarSnapshotRequest {
    url: UrlText,
    width: u32,
    height: u32,
    scroll_x: i32,
    scroll_y: i32,
}

impl SidecarSnapshotRequest {
    #[must_use]
    pub fn new(url: UrlText, width: u32, height: u32) -> Self {
        Self { url, width, height, scroll_x: 0, scroll_y: 0 }
    }

    #[must_use]
    pub fn with_scroll_offset(mut self, scroll_x: i32, scroll_y: i32) -> Self {
        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
        self
    }
}

#[derive(Clone, Debug)]
pub struct SidecarSnapshot {
    loaded_url: Option<String>,
    title: Option<String>,
    width: u32,
    height: u32,
    rgba_bytes: Vec<u8>,
}

impl SidecarSnapshot {
    fn from_report(report: SidecarReport, rgba_bytes: Vec<u8>) -> Result<Self, ServoSidecarError> {
        let expected_byte_count = expected_rgba_byte_count(report.width, report.height)?;
        if report.state != "complete" {
            return Err(ServoSidecarError::IncompleteRender { state: report.state });
        }
        if report.rgba_byte_count != expected_byte_count || rgba_bytes.len() != expected_byte_count
        {
            return Err(ServoSidecarError::RgbaByteCountMismatch {
                expected: expected_byte_count,
                reported: report.rgba_byte_count,
                actual: rgba_bytes.len(),
            });
        }
        if report.non_white_pixel_count == 0 {
            return Err(ServoSidecarError::BlankRenderedFrame {
                requested_url: report.requested_url,
            });
        }
        if report.content_pixel_count == 0 {
            return Err(ServoSidecarError::ContentlessRenderedFrame {
                requested_url: report.requested_url,
            });
        }

        Ok(Self {
            loaded_url: report.loaded_url,
            title: report.title,
            width: report.width,
            height: report.height,
            rgba_bytes,
        })
    }

    #[must_use]
    pub fn loaded_url(&self) -> Option<&str> {
        self.loaded_url.as_deref()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn into_rgba_bytes(self) -> Vec<u8> {
        self.rgba_bytes
    }
}

#[derive(Debug, Error)]
pub enum ServoSidecarError {
    #[error("current executable path is unavailable: {0}")]
    CurrentExecutable(#[source] io::Error),

    #[error("current executable directory is unavailable for {path}")]
    CurrentExecutableDirectoryUnavailable { path: PathBuf },

    #[error("servo sidecar binary is unavailable at {path}")]
    SidecarBinaryUnavailable { path: PathBuf },

    #[error("temporary frame directory is unavailable: {0}")]
    TempDirectory(#[source] io::Error),

    #[error("system clock is before UNIX epoch")]
    SystemClock(#[from] SystemTimeError),

    #[error("failed to run servo sidecar: {0}")]
    Command(#[source] io::Error),

    #[error("servo sidecar exited with {status}: {stderr}")]
    SidecarFailed { status: String, stderr: String },

    #[error("servo sidecar timed out after {seconds}s while rendering {url}")]
    SidecarTimedOut { url: String, seconds: u64 },

    #[error("failed to parse servo sidecar report: {0}")]
    Report(#[from] serde_json::Error),

    #[error("failed to read servo frame file: {0}")]
    FrameRead(#[source] io::Error),

    #[error("failed to remove servo frame file: {0}")]
    FrameCleanup(#[source] io::Error),

    #[error("servo sidecar returned incomplete render state: {state}")]
    IncompleteRender { state: String },

    #[error(
        "servo frame byte count mismatch: expected {expected}, reported {reported}, actual {actual}"
    )]
    RgbaByteCountMismatch { expected: usize, reported: usize, actual: usize },

    #[error("servo frame dimensions overflow byte count: {width}x{height}")]
    RgbaByteCountOverflow { width: u32, height: u32 },

    #[error("servo rendered a blank frame for {requested_url}")]
    BlankRenderedFrame { requested_url: String },

    #[error("servo rendered a frame without visible content for {requested_url}")]
    ContentlessRenderedFrame { requested_url: String },
}

#[derive(Deserialize)]
struct SidecarReport {
    requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    state: String,
    width: u32,
    height: u32,
    rgba_byte_count: usize,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
}

fn default_sidecar_path() -> Result<PathBuf, ServoSidecarError> {
    let current_exe = env::current_exe().map_err(ServoSidecarError::CurrentExecutable)?;
    let exe_dir = current_exe.parent().ok_or_else(|| {
        ServoSidecarError::CurrentExecutableDirectoryUnavailable { path: current_exe.clone() }
    })?;

    Ok(exe_dir.join(sidecar_binary_name()))
}

fn sidecar_binary_name() -> String {
    format!("ely_servo_sidecar{}", env::consts::EXE_SUFFIX)
}

fn temporary_rgba_path() -> Result<PathBuf, ServoSidecarError> {
    let directory = env::temp_dir().join("ely-browser-servo");
    fs::create_dir_all(&directory).map_err(ServoSidecarError::TempDirectory)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(directory.join(format!("frame-{}-{timestamp}.rgba", std::process::id())))
}

fn remove_temporary_file(path: &Path) -> Result<(), ServoSidecarError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ServoSidecarError::FrameCleanup(error)),
    }
}

fn terminate_child(mut child: std::process::Child) -> Result<(), ServoSidecarError> {
    match child.kill() {
        Ok(()) => {
            let _output = child.wait_with_output().map_err(ServoSidecarError::Command)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(ServoSidecarError::Command(error)),
    }
}

fn read_sidecar_snapshot(
    stdout: &[u8],
    rgba_path: &Path,
) -> Result<SidecarSnapshot, ServoSidecarError> {
    let report: SidecarReport = serde_json::from_slice(stdout)?;
    let rgba_bytes = fs::read(rgba_path).map_err(ServoSidecarError::FrameRead)?;
    SidecarSnapshot::from_report(report, rgba_bytes)
}

fn expected_rgba_byte_count(width: u32, height: u32) -> Result<usize, ServoSidecarError> {
    let byte_count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ServoSidecarError::RgbaByteCountOverflow { width, height })?;
    usize::try_from(byte_count)
        .map_err(|_| ServoSidecarError::RgbaByteCountOverflow { width, height })
}
