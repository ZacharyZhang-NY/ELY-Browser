use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Stdio},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use ely_domain::SitePermissionDecision;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::servo_sidecar_command::{SidecarCommandError, default_sidecar_command};

pub(crate) struct ServoLiveClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    frame_dir: PathBuf,
    frame_path: PathBuf,
}

impl ServoLiveClient {
    pub fn new(profile_data_dir: PathBuf) -> Result<Self, ServoLiveError> {
        let command_target = default_sidecar_command()?;
        if let Some(path) = command_target.missing_binary_path() {
            return Err(ServoLiveError::SidecarBinaryUnavailable { path: path.to_path_buf() });
        }

        let frame_dir = temporary_frame_dir()?;
        let frame_path = frame_dir.join("frame.rgba");
        let mut command = command_target.command();
        command.arg("live").arg("--profile-data-dir").arg(profile_data_dir);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ServoLiveError::Command)?;
        let stdin = child.stdin.take().ok_or(ServoLiveError::PipeUnavailable { name: "stdin" })?;
        let stdout =
            child.stdout.take().ok_or(ServoLiveError::PipeUnavailable { name: "stdout" })?;

        Ok(Self { child, stdin, stdout: BufReader::new(stdout), frame_dir, frame_path })
    }

    pub fn ensure(
        &mut self,
        request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        self.request(LiveRequest::Ensure {
            tab_id: request.tab_id,
            profile_id: request.profile_id,
            url: request.url,
            width: request.width,
            height: request.height,
            page_zoom_percent: request.page_zoom_percent,
            scroll_delta_x: request.scroll_delta_x,
            scroll_delta_y: request.scroll_delta_y,
            click_x: request.click_x,
            click_y: request.click_y,
            hover_x: request.hover_x,
            hover_y: request.hover_y,
            typed_text: request.typed_text,
            site_permissions: request.site_permissions,
            rgba_out: self.frame_path.display().to_string(),
        })
    }

    pub fn poll(&mut self, tab_id: String) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        self.request(LiveRequest::Poll { tab_id, rgba_out: self.frame_path.display().to_string() })
    }

    fn request(&mut self, request: LiveRequest) -> Result<Option<ServoLiveFrame>, ServoLiveError> {
        serde_json::to_writer(&mut self.stdin, &request)?;
        self.stdin.write_all(b"\n").map_err(ServoLiveError::Command)?;
        self.stdin.flush().map_err(ServoLiveError::Command)?;

        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line).map_err(ServoLiveError::Command)?;
        if bytes == 0 {
            return Err(ServoLiveError::SidecarExited);
        }

        let response: LiveResponse = serde_json::from_str(&line)?;
        if let Some(error) = response.error {
            return Err(ServoLiveError::SidecarFailed { message: error });
        }

        response.frame.map(ServoLiveFrame::from_report).transpose()
    }
}

impl Drop for ServoLiveClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.frame_dir);
    }
}

pub(crate) struct ServoLiveEnsureRequest {
    pub(crate) tab_id: String,
    pub(crate) profile_id: String,
    pub(crate) url: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) page_zoom_percent: u16,
    pub(crate) scroll_delta_x: i32,
    pub(crate) scroll_delta_y: i32,
    pub(crate) click_x: Option<u32>,
    pub(crate) click_y: Option<u32>,
    pub(crate) hover_x: Option<u32>,
    pub(crate) hover_y: Option<u32>,
    pub(crate) typed_text: Option<String>,
    pub(crate) site_permissions: Vec<ServoLiveSitePermission>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ServoLiveSitePermission {
    pub(crate) origin: String,
    pub(crate) feature: String,
    pub(crate) decision: String,
}

impl ServoLiveSitePermission {
    pub fn new(
        origin: impl Into<String>,
        feature: impl Into<String>,
        decision: SitePermissionDecision,
    ) -> Self {
        Self { origin: origin.into(), feature: feature.into(), decision: decision.as_str().into() }
    }
}

pub(crate) struct ServoLiveFrame {
    loaded_url: Option<String>,
    title: Option<String>,
    render_state: String,
    width: u32,
    height: u32,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
    rgba_bytes: Vec<u8>,
}

impl ServoLiveFrame {
    fn from_report(report: LiveFrameReport) -> Result<Self, ServoLiveError> {
        let rgba_bytes = fs::read(&report.rgba_path).map_err(ServoLiveError::FrameRead)?;
        if rgba_bytes.len() != report.rgba_byte_count {
            return Err(ServoLiveError::RgbaByteCountMismatch {
                expected: report.rgba_byte_count,
                actual: rgba_bytes.len(),
            });
        }

        Ok(Self {
            loaded_url: report.loaded_url,
            title: report.title,
            render_state: report.state,
            width: report.width,
            height: report.height,
            #[cfg(all(test, feature = "live-site-smoke"))]
            non_white_pixel_count: report.non_white_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
            content_pixel_count: report.content_pixel_count,
            #[cfg(all(test, feature = "live-site-smoke"))]
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
    pub fn render_state(&self) -> &str {
        self.render_state.as_str()
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn non_white_pixel_count(&self) -> u64 {
        self.non_white_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn content_pixel_count(&self) -> u64 {
        self.content_pixel_count
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    #[must_use]
    pub fn sample_hash(&self) -> u64 {
        self.sample_hash
    }

    #[must_use]
    pub fn into_rgba_bytes(self) -> Vec<u8> {
        self.rgba_bytes
    }
}

#[derive(Debug, Error)]
pub(crate) enum ServoLiveError {
    #[error("servo sidecar binary is unavailable at {path}")]
    SidecarBinaryUnavailable { path: PathBuf },

    #[error("failed to run servo live sidecar: {0}")]
    Command(#[source] io::Error),

    #[error("servo live sidecar pipe is unavailable: {name}")]
    PipeUnavailable { name: &'static str },

    #[error("servo live sidecar exited")]
    SidecarExited,

    #[error("servo live sidecar failed: {message}")]
    SidecarFailed { message: String },

    #[error("failed to read servo live frame file: {0}")]
    FrameRead(#[source] io::Error),

    #[error("servo live frame byte count mismatch: expected {expected}, actual {actual}")]
    RgbaByteCountMismatch { expected: usize, actual: usize },

    #[error("temporary live frame directory is unavailable: {0}")]
    TempDirectory(#[source] io::Error),

    #[error("temporary live frame timestamp is unavailable: {0}")]
    SystemClock(#[source] SystemTimeError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    SidecarCommand(#[from] SidecarCommandError),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LiveRequest {
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        click_x: Option<u32>,
        click_y: Option<u32>,
        hover_x: Option<u32>,
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permissions: Vec<ServoLiveSitePermission>,
        rgba_out: String,
    },
    Poll {
        tab_id: String,
        rgba_out: String,
    },
}

#[derive(Deserialize)]
struct LiveResponse {
    error: Option<String>,
    frame: Option<LiveFrameReport>,
}

#[derive(Deserialize)]
struct LiveFrameReport {
    loaded_url: Option<String>,
    title: Option<String>,
    state: String,
    width: u32,
    height: u32,
    rgba_path: PathBuf,
    rgba_byte_count: usize,
    #[cfg(all(test, feature = "live-site-smoke"))]
    non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    sample_hash: u64,
}

fn temporary_frame_dir() -> Result<PathBuf, ServoLiveError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(ServoLiveError::SystemClock)?
        .as_nanos();
    let directory = env::temp_dir()
        .join("ely-browser-servo-live")
        .join(format!("{}-{timestamp}", std::process::id()));
    fs::create_dir_all(&directory).map_err(ServoLiveError::TempDirectory)?;
    Ok(directory)
}
