use std::{
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use ely_domain::{ProfileId, TabId};
use serde_json::{Value, json};

use super::pages::{
    HISTORY_PAGE, OVERSIZED_HISTORY_PAGE, READ_PAGE, RSA_PRIVATE_KEY, RSA_PRIVATE_OPERATION_PAGE,
    RSA_PUBLIC_KEY, SET_PAGE, WHITE_PAGE,
};

pub(super) const WIDTH: u32 = 360;
pub(super) const HEIGHT: u32 = 240;
pub(super) const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
pub(super) const LIVE_PROTOCOL_VERSION: u32 = 4;
pub(super) const MAX_FRAME_DIMENSION: u32 = 16_384;
const MAX_FRAME_BYTE_COUNT: usize = 256 * 1024 * 1024;

pub(super) fn ensure_request(tab_id: &TabId, profile_id: &ProfileId, url: &str) -> Value {
    json!({
        "type": "ensure",
        "tab_id": tab_id.as_str(),
        "profile_id": profile_id.as_str(),
        "url": url,
        "width": WIDTH,
        "height": HEIGHT,
        "page_zoom_percent": 100,
        "device_pixel_ratio": 1.0,
        "color_scheme": "light",
        "site_permission_generation": 0,
        "site_permissions": [],
    })
}

pub(super) struct Sidecar {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: Option<ChildStderr>,
}

impl Sidecar {
    pub(super) fn spawn(profile_data_dir: &Path) -> Result<Self, Box<dyn Error>> {
        let mut sidecar = Self::spawn_without_handshake(profile_data_dir)?;
        sidecar.handshake()?;
        Ok(sidecar)
    }

    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) fn spawn_hardware(
        profile_data_dir: &Path,
        mach_service: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let mut sidecar = Self::spawn_process(
            profile_data_dir,
            &["--rendering-context", "hardware", "--iosurface-mach-service", mach_service],
        )?;
        sidecar.handshake()?;
        Ok(sidecar)
    }

    fn handshake(&mut self) -> Result<(), Box<dyn Error>> {
        let response = self.exchange(&json!({
            "type": "handshake",
            "protocol_version": LIVE_PROTOCOL_VERSION,
        }))?;
        if response.protocol_version != Some(LIVE_PROTOCOL_VERSION) || response.error.is_some() {
            return Err(io::Error::other("sidecar protocol handshake failed").into());
        }
        Ok(())
    }

    pub(super) fn spawn_without_handshake(profile_data_dir: &Path) -> Result<Self, Box<dyn Error>> {
        Self::spawn_process(profile_data_dir, &[])
    }

    fn spawn_process(profile_data_dir: &Path, extra_args: &[&str]) -> Result<Self, Box<dyn Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ely_servo_sidecar"));
        command.arg("live").arg("--profile-data-dir").arg(profile_data_dir);
        command.args(extra_args);
        let mut child =
            command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stdin =
            child.stdin.take().ok_or_else(|| io::Error::other("sidecar stdin was not piped"))?;
        let stdout =
            child.stdout.take().ok_or_else(|| io::Error::other("sidecar stdout was not piped"))?;
        let stderr = child.stderr.take();
        Ok(Self { child, stdin: Some(stdin), stdout: BufReader::new(stdout), stderr })
    }

    pub(super) fn ensure_and_wait(
        &mut self,
        profile_id: &ProfileId,
        url: &str,
        expected_title: &str,
    ) -> Result<FramePacket, Box<dyn Error>> {
        self.ensure_and_wait_matching(profile_id, url, expected_title, |_| true)
    }

    pub(super) fn ensure_and_wait_visible(
        &mut self,
        profile_id: &ProfileId,
        url: &str,
        expected_title: &str,
    ) -> Result<FramePacket, Box<dyn Error>> {
        self.ensure_and_wait_matching(profile_id, url, expected_title, |frame| {
            frame.non_white_pixel_count > 0 && frame.content_pixel_count > 0
        })
    }

    fn ensure_and_wait_matching(
        &mut self,
        profile_id: &ProfileId,
        url: &str,
        expected_title: &str,
        matches_frame: impl Fn(&FramePacket) -> bool,
    ) -> Result<FramePacket, Box<dyn Error>> {
        let tab_id = TabId::new();
        let ensure = ensure_request(&tab_id, profile_id, url);
        let mut response = self.exchange(&ensure)?;
        let started_at = Instant::now();
        let mut latest_title = None;
        loop {
            if let Some(error) = response.error {
                return Err(io::Error::other(format!("sidecar response error: {error}")).into());
            }
            if let Some(frame) = response.frame {
                latest_title = frame.title.clone();
                if frame.title.as_deref() == Some(expected_title) && matches_frame(&frame) {
                    return Ok(frame);
                }
            }
            if started_at.elapsed() >= RESPONSE_TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "timed out waiting for title {expected_title:?}; latest={latest_title:?}"
                    ),
                )
                .into());
            }
            thread::sleep(Duration::from_millis(2));
            response = self.exchange(&json!({ "type": "poll", "tab_id": tab_id.as_str() }))?;
        }
    }

    pub(super) fn exchange(&mut self, request: &Value) -> Result<WireResponse, Box<dyn Error>> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "sidecar stdin is closed"))?;
        serde_json::to_writer(&mut *stdin, request)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        read_response(&mut self.stdout)
    }

    pub(super) fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        let response = self.exchange(&json!({ "type": "shutdown" }))?;
        if response.protocol_version != Some(LIVE_PROTOCOL_VERSION)
            || response.error.is_some()
            || response.frame.is_some()
        {
            return Err(
                io::Error::other("shutdown response must be an empty acknowledgement").into()
            );
        }
        self.stdin.take();
        let status = wait_for_exit(&mut self.child, RESPONSE_TIMEOUT)?;
        if status.success() {
            return Ok(());
        }
        let mut stderr = String::new();
        if let Some(mut pipe) = self.stderr.take() {
            pipe.read_to_string(&mut stderr)?;
        }
        Err(io::Error::other(format!("sidecar exited with {status}: {stderr}")).into())
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<ExitStatus, Box<dyn Error>> {
    let started_at = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started_at.elapsed() >= timeout {
            child.kill()?;
            let _ = child.wait();
            return Err(
                io::Error::new(io::ErrorKind::TimedOut, "sidecar shutdown timed out").into()
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) struct WireResponse {
    pub(super) protocol_version: Option<u32>,
    pub(super) error: Option<String>,
    pub(super) frame: Option<FramePacket>,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) surface_handle: Option<SurfaceHandle>,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) current_surface_id: Option<u64>,
}

pub(super) struct FramePacket {
    pub(super) loaded_url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) non_white_pixel_count: u64,
    pub(super) content_pixel_count: u64,
    pub(super) sample_hash: u64,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) rgba_byte_count: usize,
    _rgba: Vec<u8>,
}

#[cfg(all(feature = "hardware-render", target_os = "macos"))]
#[derive(Clone, Copy, Debug)]
pub(super) struct SurfaceHandle {
    pub(super) mach_port_name: u32,
    pub(super) surface_id: u64,
    pub(super) width: u32,
    pub(super) height: u32,
}

fn read_response(stdout: &mut BufReader<ChildStdout>) -> Result<WireResponse, Box<dyn Error>> {
    let mut line = String::new();
    if stdout.read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "sidecar response ended").into());
    }
    let header: Value = serde_json::from_str(&line)?;
    let protocol_version = header
        .get("protocol_version")
        .and_then(Value::as_u64)
        .and_then(|value| value.try_into().ok());
    let error = header.get("error").and_then(Value::as_str).map(str::to_string);
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    let surface_handle = header
        .get("surface_handle")
        .filter(|value| !value.is_null())
        .map(|handle| {
            Ok::<_, Box<dyn Error>>(SurfaceHandle {
                mach_port_name: u32_field(handle, "mach_port_name")?,
                surface_id: u64_field(handle, "surface_id")?,
                width: u32_field(handle, "width")?,
                height: u32_field(handle, "height")?,
            })
        })
        .transpose()?;
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    let current_surface_id = header.get("current_surface_id").and_then(Value::as_u64);
    let Some(frame) = header.get("frame").filter(|value| !value.is_null()) else {
        return Ok(WireResponse {
            protocol_version,
            error,
            frame: None,
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            surface_handle,
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            current_surface_id,
        });
    };
    let width = u32_field(frame, "width")?;
    let height = u32_field(frame, "height")?;
    let rgba_byte_count = usize_field(frame, "rgba_byte_count")?;
    let expected = usize::try_from(width)?
        .checked_mul(usize::try_from(height)?)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame size overflow"))?;
    if (rgba_byte_count != 0 && rgba_byte_count != expected)
        || rgba_byte_count > MAX_FRAME_BYTE_COUNT
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid frame byte count {rgba_byte_count}; expected {expected}"),
        )
        .into());
    }
    let mut rgba = vec![0; rgba_byte_count];
    stdout.read_exact(&mut rgba)?;
    let packet = FramePacket {
        loaded_url: frame.get("loaded_url").and_then(Value::as_str).map(str::to_string),
        title: frame.get("title").and_then(Value::as_str).map(str::to_string),
        width,
        height,
        non_white_pixel_count: u64_field(frame, "non_white_pixel_count")?,
        content_pixel_count: u64_field(frame, "content_pixel_count")?,
        sample_hash: u64_field(frame, "sample_hash")?,
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        rgba_byte_count,
        _rgba: rgba,
    };
    Ok(WireResponse {
        protocol_version,
        error,
        frame: Some(packet),
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        surface_handle,
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        current_surface_id,
    })
}

fn u32_field(value: &Value, name: &str) -> Result<u32, Box<dyn Error>> {
    Ok(u32::try_from(u64_field(value, name)?)?)
}

fn usize_field(value: &Value, name: &str) -> Result<usize, Box<dyn Error>> {
    Ok(usize::try_from(u64_field(value, name)?)?)
}

fn u64_field(value: &Value, name: &str) -> Result<u64, Box<dyn Error>> {
    value.get(name).and_then(Value::as_u64).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, format!("missing frame field {name}")).into()
    })
}

pub(super) struct TestDirectory(PathBuf);

impl TestDirectory {
    pub(super) fn new() -> Result<Self, io::Error> {
        let path = std::env::temp_dir().join(format!(
            "ely-sidecar-test-{}-{}",
            std::process::id(),
            ProfileId::new()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) struct TestServer {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    diagnostics: Arc<Mutex<ServerDiagnostics>>,
}

#[derive(Default)]
struct ServerDiagnostics {
    requests: Vec<String>,
    errors: Vec<String>,
}

impl TestServer {
    pub(super) fn start() -> Result<Self, io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let diagnostics = Arc::new(Mutex::new(ServerDiagnostics::default()));
        let thread_diagnostics = diagnostics.clone();
        let thread = thread::spawn(move || serve(listener, &thread_stop, &thread_diagnostics));
        Ok(Self { address, stop, thread: Some(thread), diagnostics })
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    pub(super) fn diagnostics(&self) -> String {
        self.diagnostics.lock().map_or_else(
            |_| "lock poisoned".to_string(),
            |diagnostics| {
                format!("requests={:?}, errors={:?}", diagnostics.requests, diagnostics.errors)
            },
        )
    }

    pub(super) fn request_count(&self, path: &str) -> usize {
        self.diagnostics.lock().map_or(0, |diagnostics| {
            diagnostics
                .requests
                .iter()
                .filter(|request| {
                    request.split_whitespace().nth(1).is_some_and(|url| {
                        url == path
                            || url.strip_prefix(path).is_some_and(|suffix| suffix.starts_with('?'))
                    })
                })
                .count()
        })
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(listener: TcpListener, stop: &AtomicBool, diagnostics: &Mutex<ServerDiagnostics>) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(error) = serve_connection(stream, diagnostics)
                    && let Ok(mut diagnostics) = diagnostics.lock()
                {
                    diagnostics.errors.push(error.to_string());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(_) => return,
        }
    }
}

fn serve_connection(
    stream: TcpStream,
    diagnostics: &Mutex<ServerDiagnostics>,
) -> Result<(), io::Error> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if let Ok(mut diagnostics) = diagnostics.lock() {
        diagnostics.requests.push(request_line.trim().to_string());
    }
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line)? == 0 || header_line == "\r\n" {
            break;
        }
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let is_set = path == "/set";
    let (body, content_type): (&[u8], &str) = if is_set {
        (SET_PAGE.as_bytes(), "text/html; charset=utf-8")
    } else if path.starts_with("/history") {
        (HISTORY_PAGE.as_bytes(), "text/html; charset=utf-8")
    } else if path == "/oversized-history" {
        (OVERSIZED_HISTORY_PAGE.as_bytes(), "text/html; charset=utf-8")
    } else if path == "/white" {
        (WHITE_PAGE.as_bytes(), "text/html; charset=utf-8")
    } else if path == "/rsa-private-operations" {
        (RSA_PRIVATE_OPERATION_PAGE.as_bytes(), "text/html; charset=utf-8")
    } else if path == "/rsa-private.der" {
        (RSA_PRIVATE_KEY, "application/octet-stream")
    } else if path == "/rsa-public.der" {
        (RSA_PUBLIC_KEY, "application/octet-stream")
    } else {
        (READ_PAGE.as_bytes(), "text/html; charset=utf-8")
    };
    let cookie_header =
        if is_set { "Set-Cookie: ely_cookie=persisted; Path=/; SameSite=Lax\r\n" } else { "" };
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{cookie_header}Connection: close\r\n\r\n",
        body.len()
    );
    reader.get_mut().write_all(headers.as_bytes())?;
    reader.get_mut().write_all(body)?;
    reader.get_mut().flush()
}
