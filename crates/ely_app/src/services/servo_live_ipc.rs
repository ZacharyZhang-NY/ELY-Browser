use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{ChildStdin, ChildStdout},
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{
    ServoLiveError, ServoLiveFrame, ServoLivePermissionGrant,
    wire::{
        LiveRequest, LiveResponse, LiveSurfaceHandle, MAX_FRAME_BYTE_COUNT, MAX_FRAME_DIMENSION,
        MAX_RESPONSE_HEADER_BYTES,
    },
};

pub(super) struct ServoLiveIpc {
    requests: Option<Sender<IpcRequest>>,
    thread: Option<JoinHandle<()>>,
}

pub(super) struct IpcReply {
    pub(super) protocol_version: Option<u32>,
    pub(super) error: Option<String>,
    pub(super) frame: Option<ServoLiveFrame>,
    pub(super) surface_handle: Option<LiveSurfaceHandle>,
    pub(super) current_surface_id: Option<u64>,
    pub(super) permission_consumptions: Vec<ServoLivePermissionGrant>,
}

struct IpcRequest {
    request: LiveRequest,
    response: SyncSender<Result<IpcReply, ServoLiveError>>,
}

impl ServoLiveIpc {
    pub(super) fn spawn(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        let (requests, receiver) = mpsc::channel();
        let thread = thread::spawn(move || run_io(stdin, BufReader::new(stdout), receiver));
        Self { requests: Some(requests), thread: Some(thread) }
    }

    pub(super) fn exchange(
        &self,
        request: LiveRequest,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<IpcReply, ServoLiveError> {
        let requests = self.requests.as_ref().ok_or(ServoLiveError::SidecarExited)?;
        let (response, receiver) = mpsc::sync_channel(1);
        requests
            .send(IpcRequest { request, response })
            .map_err(|_| ServoLiveError::SidecarExited)?;
        receiver.recv_timeout(timeout).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => {
                ServoLiveError::RequestTimedOut { operation, timeout_millis: timeout.as_millis() }
            }
            mpsc::RecvTimeoutError::Disconnected => ServoLiveError::SidecarExited,
        })?
    }

    pub(super) fn close_and_join(&mut self, timeout: Duration) {
        self.requests.take();
        let Some(thread) = self.thread.take() else {
            return;
        };
        let started_at = Instant::now();
        while !thread.is_finished() && started_at.elapsed() < timeout {
            thread::sleep(Duration::from_millis(5));
        }
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
}

fn run_io(
    mut stdin: ChildStdin,
    mut stdout: BufReader<ChildStdout>,
    requests: Receiver<IpcRequest>,
) {
    while let Ok(message) = requests.recv() {
        let should_shutdown = matches!(message.request, LiveRequest::Shutdown);
        let response = exchange_pipes(&mut stdin, &mut stdout, &message.request);
        let should_stop = should_shutdown || response.is_err();
        let _ = message.response.send(response);
        if should_stop {
            break;
        }
    }
}

fn exchange_pipes(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: &LiveRequest,
) -> Result<IpcReply, ServoLiveError> {
    serde_json::to_writer(&mut *stdin, request)?;
    stdin.write_all(b"\n").map_err(ServoLiveError::Command)?;
    stdin.flush().map_err(ServoLiveError::Command)?;
    read_reply(stdout)
}

fn read_reply(stdout: &mut impl BufRead) -> Result<IpcReply, ServoLiveError> {
    let mut line = String::new();
    let bytes = Read::by_ref(stdout)
        .take((MAX_RESPONSE_HEADER_BYTES + 1) as u64)
        .read_line(&mut line)
        .map_err(ServoLiveError::Command)?;
    if bytes == 0 {
        return Err(ServoLiveError::SidecarExited);
    }
    if bytes > MAX_RESPONSE_HEADER_BYTES {
        return Err(ServoLiveError::ResponseHeaderTooLarge { limit: MAX_RESPONSE_HEADER_BYTES });
    }
    if !line.ends_with('\n') {
        return Err(ServoLiveError::InvalidResponse {
            message: "response header is missing its newline delimiter",
        });
    }

    let response: LiveResponse = serde_json::from_str(&line)?;
    if response.error.is_some()
        && (response.frame.is_some()
            || response.surface_handle.is_some()
            || response.current_surface_id.is_some())
    {
        return Err(ServoLiveError::InvalidResponse {
            message: "error response contains a frame or hardware surface",
        });
    }
    if response.frame.is_none()
        && (response.surface_handle.is_some() || response.current_surface_id.is_some())
    {
        return Err(ServoLiveError::InvalidResponse {
            message: "hardware surface response is missing its frame report",
        });
    }
    if let Some(handle) = response.surface_handle.as_ref() {
        if response.current_surface_id != Some(handle.surface_id) {
            return Err(ServoLiveError::InvalidResponse {
                message: "surface_handle does not match current_surface_id",
            });
        }
        if response
            .frame
            .as_ref()
            .is_none_or(|frame| frame.width != handle.width || frame.height != handle.height)
        {
            return Err(ServoLiveError::InvalidResponse {
                message: "surface_handle dimensions do not match frame report",
            });
        }
    }
    let frame = response.frame.map(|report| {
        let rgba_byte_count = validate_frame_layout(report.width, report.height)?;
        let hardware_frame = report.rgba_byte_count == 0;
        if hardware_frame && response.current_surface_id.is_none() {
            return Err(ServoLiveError::InvalidResponse {
                message: "hardware frame is missing current_surface_id",
            });
        }
        if !hardware_frame
            && (response.surface_handle.is_some() || response.current_surface_id.is_some())
        {
            return Err(ServoLiveError::InvalidResponse {
                message: "software frame contains hardware surface metadata",
            });
        }
        if !hardware_frame && report.rgba_byte_count != rgba_byte_count {
            return Err(ServoLiveError::InvalidFrameByteCount {
                advertised: report.rgba_byte_count,
                expected: rgba_byte_count as u64,
                width: report.width,
                height: report.height,
            });
        }
        let rgba_bytes = if hardware_frame {
            None
        } else {
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(rgba_byte_count).map_err(|source| {
                ServoLiveError::FrameAllocation { bytes: rgba_byte_count, source }
            })?;
            bytes.resize(rgba_byte_count, 0);
            stdout.read_exact(&mut bytes).map_err(ServoLiveError::FrameRead)?;
            Some(bytes)
        };
        Ok(ServoLiveFrame::from_parts(report, rgba_bytes))
    });
    Ok(IpcReply {
        protocol_version: response.protocol_version,
        error: response.error,
        frame: frame.transpose()?,
        surface_handle: response.surface_handle,
        current_surface_id: response.current_surface_id,
        permission_consumptions: response
            .permission_consumptions
            .into_iter()
            .map(ServoLivePermissionGrant::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(super) fn validate_frame_layout(width: u32, height: u32) -> Result<usize, ServoLiveError> {
    if width == 0 || height == 0 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return Err(ServoLiveError::InvalidFrameDimensions {
            width,
            height,
            max_dimension: MAX_FRAME_DIMENSION,
        });
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ServoLiveError::InvalidFrameDimensions {
            width,
            height,
            max_dimension: MAX_FRAME_DIMENSION,
        })?;
    if bytes > MAX_FRAME_BYTE_COUNT as u64 {
        return Err(ServoLiveError::FrameByteLimitExceeded { bytes, limit: MAX_FRAME_BYTE_COUNT });
    }
    Ok(bytes as usize)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn frame_layout_rejects_oversized_frames() {
        assert!(matches!(
            validate_frame_layout(MAX_FRAME_DIMENSION, MAX_FRAME_DIMENSION),
            Err(ServoLiveError::FrameByteLimitExceeded { .. })
        ));
        assert!(matches!(
            validate_frame_layout(MAX_FRAME_DIMENSION + 1, 1),
            Err(ServoLiveError::InvalidFrameDimensions { .. })
        ));
    }

    #[test]
    fn reply_rejects_oversized_frame_before_readback_allocation() {
        let header = format!(
            "{{\"protocol_version\":3,\"error\":null,\"frame\":{{\"loaded_url\":null,\"title\":null,\"state\":\"complete\",\"width\":{0},\"height\":{0},\"device_pixel_ratio\":1.0,\"css_viewport_width\":{0},\"css_viewport_height\":{0},\"rgba_byte_count\":1073741824,\"pixels_changed\":true}}}}\n",
            MAX_FRAME_DIMENSION
        );
        let mut input = Cursor::new(header.into_bytes());

        assert!(matches!(
            read_reply(&mut input),
            Err(ServoLiveError::FrameByteLimitExceeded { .. })
        ));
    }

    #[test]
    fn reply_accepts_the_exact_header_limit() -> Result<(), ServoLiveError> {
        let mut header = br#"{"protocol_version":3,"error":null,"frame":null}"#.to_vec();
        header.resize(MAX_RESPONSE_HEADER_BYTES - 1, b' ');
        header.push(b'\n');

        let reply = read_reply(&mut Cursor::new(header))?;

        assert_eq!(reply.protocol_version, Some(3));
        Ok(())
    }

    #[test]
    fn reply_rejects_one_byte_over_the_header_limit() {
        let mut header = br#"{"protocol_version":3,"error":null,"frame":null}"#.to_vec();
        header.resize(MAX_RESPONSE_HEADER_BYTES, b' ');
        header.push(b'\n');

        assert!(matches!(
            read_reply(&mut Cursor::new(header)),
            Err(ServoLiveError::ResponseHeaderTooLarge { limit: MAX_RESPONSE_HEADER_BYTES })
        ));
    }

    #[test]
    fn reply_parses_permission_consumption_without_a_frame() -> Result<(), ServoLiveError> {
        let profile_id = ely_domain::ProfileId::new();
        let header = format!(
            "{{\"protocol_version\":3,\"error\":null,\"frame\":null,\"permission_consumptions\":[{{\"profile_id\":\"{}\",\"origin\":\"https://example.com\",\"feature\":\"camera\",\"grant_revision\":7}}]}}\n",
            profile_id.as_str(),
        );
        let mut input = Cursor::new(header.into_bytes());

        let reply = read_reply(&mut input)?;

        let [consumed] = reply.permission_consumptions.as_slice() else {
            return Err(ServoLiveError::InvalidResponse {
                message: "permission consumption was missing",
            });
        };
        assert_eq!(consumed.profile_id(), &profile_id);
        assert_eq!(consumed.origin().as_str(), "https://example.com");
        assert_eq!(consumed.feature(), ely_domain::SitePermissionFeature::Camera);
        assert_eq!(consumed.grant_revision(), 7);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn hardware_reply_uses_surface_without_rgba_allocation() -> Result<(), ServoLiveError> {
        let header = concat!(
            "{\"protocol_version\":3,\"error\":null,",
            "\"surface_handle\":{\"mach_port_name\":91,\"surface_id\":7,\"width\":64,\"height\":48},",
            "\"current_surface_id\":7,",
            "\"frame\":{\"loaded_url\":null,\"title\":null,\"state\":\"complete\",",
            "\"width\":64,\"height\":48,\"device_pixel_ratio\":1.0,",
            "\"css_viewport_width\":64,\"css_viewport_height\":48,",
            "\"rgba_byte_count\":0,\"pixels_changed\":true}}\n"
        );
        let mut input = Cursor::new(header.as_bytes());

        let reply = read_reply(&mut input)?;
        let Some(frame) = reply.frame else {
            return Err(ServoLiveError::InvalidResponse { message: "test frame was missing" });
        };

        assert_eq!(reply.current_surface_id, Some(7));
        assert_eq!(reply.surface_handle.map(|handle| handle.surface_id), Some(7));
        assert!(frame.into_rgba_bytes().is_none());
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn hardware_reply_rejects_mismatched_surface_identity() {
        let header = hardware_header(8, 64, 48);
        let mut input = Cursor::new(header.into_bytes());

        assert!(matches!(
            read_reply(&mut input),
            Err(ServoLiveError::InvalidResponse {
                message: "surface_handle does not match current_surface_id"
            })
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn hardware_reply_rejects_mismatched_surface_dimensions() {
        let header = hardware_header(7, 96, 72);
        let mut input = Cursor::new(header.into_bytes());

        assert!(matches!(
            read_reply(&mut input),
            Err(ServoLiveError::InvalidResponse {
                message: "surface_handle dimensions do not match frame report"
            })
        ));
    }

    #[cfg(target_os = "macos")]
    fn hardware_header(current_surface_id: u64, handle_width: u32, handle_height: u32) -> String {
        format!(
            "{{\"protocol_version\":3,\"error\":null,\"surface_handle\":{{\"mach_port_name\":91,\"surface_id\":7,\"width\":{handle_width},\"height\":{handle_height}}},\"current_surface_id\":{current_surface_id},\"frame\":{{\"loaded_url\":null,\"title\":null,\"state\":\"complete\",\"width\":64,\"height\":48,\"device_pixel_ratio\":1.0,\"css_viewport_width\":64,\"css_viewport_height\":48,\"rgba_byte_count\":0,\"pixels_changed\":true}}}}\n"
        )
    }
}
