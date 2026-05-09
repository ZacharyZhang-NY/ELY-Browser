use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use ely_domain::{ProfileId, UrlText};
use serde::Deserialize;
use thiserror::Error;

use super::{
    servo_profile_data::{default_profile_data_root, profile_data_dir},
    servo_sidecar_command::{SidecarCommandTarget, default_sidecar_command},
};

const SIDECAR_POLL_INTERVAL: Duration = Duration::from_millis(20);
const SIDECAR_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const SIDECAR_NAVIGATION_ATTEMPTS: usize = 3;
const SIDECAR_INTERACTION_ATTEMPTS: usize = 1;

#[derive(Clone, Debug)]
pub struct ServoSidecarClient {
    command_target: SidecarCommandTarget,
    profile_data_root: PathBuf,
}

impl ServoSidecarClient {
    pub fn new() -> Result<Self, ServoSidecarError> {
        Ok(Self {
            command_target: default_sidecar_command()?,
            profile_data_root: default_profile_data_root()
                .ok_or(ServoSidecarError::ProfileDataRootUnavailable)?,
        })
    }

    pub fn snapshot(
        &self,
        request: SidecarSnapshotRequest,
    ) -> Result<SidecarSnapshot, ServoSidecarError> {
        if let Some(path) = self.command_target.missing_binary_path() {
            return Err(ServoSidecarError::SidecarBinaryUnavailable { path: path.to_path_buf() });
        }

        let mut attempt = 0;
        loop {
            match self.snapshot_once(&request) {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => {
                    attempt += 1;
                    if attempt >= request.max_attempts() {
                        return Err(error);
                    }
                    thread::sleep(SIDECAR_RETRY_INTERVAL);
                }
            }
        }
    }

    fn snapshot_once(
        &self,
        request: &SidecarSnapshotRequest,
    ) -> Result<SidecarSnapshot, ServoSidecarError> {
        let rgba_path = temporary_rgba_path()?;
        let profile_data_dir = profile_data_dir(&self.profile_data_root, &request.profile_id);
        let output = match self.run_snapshot_command(request, &rgba_path, &profile_data_dir) {
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

        let snapshot = read_sidecar_snapshot(&output.stdout, &rgba_path, &request.profile_id);
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
        profile_data_dir: &Path,
    ) -> Result<Output, ServoSidecarError> {
        let mut command = self.command_target.command();
        append_snapshot_args(&mut command, request, rgba_path, profile_data_dir);
        if let Some(click_point) = request.click_point {
            command
                .arg("--click-x")
                .arg(click_point.x.to_string())
                .arg("--click-y")
                .arg(click_point.y.to_string());
        }
        if let Some(typed_text) = request.typed_text.as_deref() {
            command.arg("--type-text").arg(typed_text);
        }

        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ServoSidecarError::Command)?;

        let started_at = Instant::now();
        loop {
            if child.try_wait().map_err(ServoSidecarError::Command)?.is_some() {
                return child.wait_with_output().map_err(ServoSidecarError::Command);
            }

            let timeout = self.command_target.timeout();
            if started_at.elapsed() >= timeout {
                terminate_child(child)?;
                return Err(ServoSidecarError::SidecarTimedOut {
                    url: request.url.as_str().to_string(),
                    seconds: timeout.as_secs(),
                });
            }

            thread::sleep(SIDECAR_POLL_INTERVAL);
        }
    }
}

fn append_snapshot_args(
    command: &mut Command,
    request: &SidecarSnapshotRequest,
    rgba_path: &Path,
    profile_data_dir: &Path,
) {
    command
        .arg("snapshot")
        .arg("--url")
        .arg(request.url.as_str())
        .arg("--profile-id")
        .arg(request.profile_id.as_str())
        .arg("--profile-data-dir")
        .arg(profile_data_dir)
        .arg("--rgba-out")
        .arg(rgba_path)
        .arg("--width")
        .arg(request.width.to_string())
        .arg("--height")
        .arg(request.height.to_string())
        .arg("--scroll-x")
        .arg(request.scroll_x.to_string())
        .arg("--scroll-y")
        .arg(request.scroll_y.to_string());
}

#[derive(Clone, Debug)]
pub struct SidecarSnapshotRequest {
    url: UrlText,
    profile_id: ProfileId,
    width: u32,
    height: u32,
    scroll_x: i32,
    scroll_y: i32,
    click_point: Option<SidecarClickPoint>,
    typed_text: Option<String>,
}

impl SidecarSnapshotRequest {
    #[must_use]
    pub fn new(url: UrlText, profile_id: ProfileId, width: u32, height: u32) -> Self {
        Self {
            url,
            profile_id,
            width,
            height,
            scroll_x: 0,
            scroll_y: 0,
            click_point: None,
            typed_text: None,
        }
    }

    #[must_use]
    pub fn with_scroll_offset(mut self, scroll_x: i32, scroll_y: i32) -> Self {
        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
        self
    }

    #[must_use]
    pub fn with_click_point(mut self, x: u32, y: u32) -> Self {
        self.click_point = Some(SidecarClickPoint { x, y });
        self
    }

    #[must_use]
    pub fn with_typed_text(mut self, typed_text: String) -> Self {
        self.typed_text = Some(typed_text);
        self
    }

    #[cfg(test)]
    pub(crate) fn typed_text_for_test(&self) -> Option<&str> {
        self.typed_text.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn profile_id_for_test(&self) -> &ProfileId {
        &self.profile_id
    }

    fn max_attempts(&self) -> usize {
        if self.click_point.is_some() || self.typed_text.is_some() {
            return SIDECAR_INTERACTION_ATTEMPTS;
        }

        SIDECAR_NAVIGATION_ATTEMPTS
    }
}

#[derive(Clone, Copy, Debug)]
struct SidecarClickPoint {
    x: u32,
    y: u32,
}

#[derive(Clone, Debug)]
pub struct SidecarSnapshot {
    loaded_url: Option<String>,
    title: Option<String>,
    width: u32,
    height: u32,
    #[cfg(test)]
    non_white_pixel_count: u64,
    #[cfg(test)]
    content_pixel_count: u64,
    #[cfg(test)]
    sample_hash: u64,
    rgba_bytes: Vec<u8>,
}

impl SidecarSnapshot {
    fn from_report(
        report: SidecarReport,
        expected_profile_id: &ProfileId,
        rgba_bytes: Vec<u8>,
    ) -> Result<Self, ServoSidecarError> {
        let expected_byte_count = expected_rgba_byte_count(report.width, report.height)?;
        if report.profile_id != expected_profile_id.as_str() {
            return Err(ServoSidecarError::ProfileMismatch {
                expected: expected_profile_id.as_str().to_string(),
                actual: report.profile_id,
            });
        }
        if !is_renderable_state(&report.state) {
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
            #[cfg(test)]
            non_white_pixel_count: report.non_white_pixel_count,
            #[cfg(test)]
            content_pixel_count: report.content_pixel_count,
            #[cfg(test)]
            sample_hash: report.sample_hash,
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

    #[error("profile data root is unavailable")]
    ProfileDataRootUnavailable,

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

    #[error("servo sidecar returned profile {actual}, expected {expected}")]
    ProfileMismatch { expected: String, actual: String },

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

fn is_renderable_state(state: &str) -> bool {
    matches!(state, "complete" | "loading")
}

#[derive(Deserialize)]
struct SidecarReport {
    requested_url: String,
    profile_id: String,
    loaded_url: Option<String>,
    title: Option<String>,
    state: String,
    width: u32,
    height: u32,
    rgba_byte_count: usize,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    #[cfg(test)]
    sample_hash: u64,
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
    expected_profile_id: &ProfileId,
) -> Result<SidecarSnapshot, ServoSidecarError> {
    let report: SidecarReport = serde_json::from_slice(stdout)?;
    let rgba_bytes = fs::read(rgba_path).map_err(ServoSidecarError::FrameRead)?;
    SidecarSnapshot::from_report(report, expected_profile_id, rgba_bytes)
}

fn expected_rgba_byte_count(width: u32, height: u32) -> Result<usize, ServoSidecarError> {
    let byte_count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ServoSidecarError::RgbaByteCountOverflow { width, height })?;
    usize::try_from(byte_count)
        .map_err(|_| ServoSidecarError::RgbaByteCountOverflow { width, height })
}

#[cfg(test)]
#[path = "servo_sidecar_tests.rs"]
mod tests;
