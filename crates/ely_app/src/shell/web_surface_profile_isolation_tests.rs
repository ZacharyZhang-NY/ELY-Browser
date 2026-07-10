use std::{
    error::Error,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
use gpui::{Bounds, point, px, size};

use crate::{
    services::ProfileDataMode,
    shell::web_surface_state::{WebSurfaceInputOutcome, WebSurfaceState},
};

use super::WebSurfaceStore;

const PROBE_WIDTH: u32 = 640;
const PROBE_HEIGHT: u32 = 480;
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[test]
fn web_surface_keeps_profile_site_data_isolated() -> Result<(), Box<dyn Error>> {
    let mut server = ProfileProbeServer::start()?;
    let mut store = WebSurfaceStore::new();
    let profile_a = ProfileId::new();
    let profile_b = ProfileId::new();

    let tab_a_seed = render_probe(
        &mut store,
        &profile_a,
        &format!("{}/probe?value=alpha", server.origin()),
        "request=empty|document=alpha|storage=alpha|cache=cache-1",
    )?;
    let tab_b_seed = render_probe(
        &mut store,
        &profile_b,
        &format!("{}/probe?value=beta", server.origin()),
        "request=empty|document=beta|storage=beta|cache=cache-2",
    )?;
    let tab_a_inspect = render_probe(
        &mut store,
        &profile_a,
        &format!("{}/probe?inspect=alpha", server.origin()),
        "request=alpha|document=alpha|storage=alpha|cache=cache-1",
    )?;
    let tab_b_inspect = render_probe(
        &mut store,
        &profile_b,
        &format!("{}/probe?inspect=beta", server.origin()),
        "request=beta|document=beta|storage=beta|cache=cache-2",
    )?;

    assert_eq!(server.cache_request_count(), 2);
    store.close_surface(&tab_a_seed);
    store.close_surface(&tab_a_inspect);
    store.flush_runtime_for_test();

    let tab_a_reopened = render_probe(
        &mut store,
        &profile_a,
        &format!("{}/probe?value=gamma", server.origin()),
        "request=empty|document=gamma|storage=gamma|cache=cache-3",
    )?;
    assert_eq!(server.cache_request_count(), 3);

    for tab_id in [tab_b_seed, tab_b_inspect, tab_a_reopened] {
        store.close_surface(&tab_id);
    }
    store.flush_runtime_for_test();
    drop(store);
    server.finish()?;
    Ok(())
}

fn render_probe(
    store: &mut WebSurfaceStore,
    profile_id: &ProfileId,
    url: &str,
    expected_title: &str,
) -> Result<TabId, Box<dyn Error>> {
    let tab = BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        profile_id.clone(),
        "Profile probe",
        UrlText::parse(url)?,
    );
    assert_eq!(
        store.record_viewport_size(tab.id(), probe_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));

    let started_at = Instant::now();
    let mut last_title = None;
    loop {
        store.tick(std::slice::from_ref(tab.id()));
        match store.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame)) => {
                last_title = frame.title().map(str::to_string);
                if frame.title() == Some(expected_title) {
                    return Ok(tab.id().clone());
                }
            }
            Some(WebSurfaceState::Failed { message }) => {
                return Err(format!("profile probe failed for {url}: {message}").into());
            }
            Some(WebSurfaceState::Loading { .. }) | None => {}
        }

        if started_at.elapsed() >= PROBE_TIMEOUT {
            return Err(format!(
                "timed out waiting for profile probe title `{expected_title}` at {url}; last title: {last_title:?}",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn probe_bounds() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(PROBE_WIDTH as f32), px(PROBE_HEIGHT as f32)))
}

pub(super) struct ProfileProbeServer {
    origin: String,
    cache_requests: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl ProfileProbeServer {
    pub(super) fn start() -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let cache_requests = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let thread_cache_requests = cache_requests.clone();
        let thread_shutdown = shutdown.clone();
        let thread_error = error.clone();
        let thread = thread::Builder::new().name("ely-profile-probe-server".to_string()).spawn(
            move || {
                serve_profile_probes(
                    listener,
                    thread_cache_requests,
                    thread_shutdown,
                    thread_error,
                );
            },
        )?;

        Ok(Self {
            origin: format!("http://{address}"),
            cache_requests,
            shutdown,
            error,
            thread: Some(thread),
        })
    }

    pub(super) fn origin(&self) -> &str {
        self.origin.as_str()
    }

    fn cache_request_count(&self) -> usize {
        self.cache_requests.load(Ordering::SeqCst)
    }

    pub(super) fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            return Err("profile probe server thread panicked".into());
        }
        let error = self.error.lock().map_err(|_| "profile probe error lock was poisoned")?.take();
        match error {
            Some(message) => Err(message.into()),
            None => Ok(()),
        }
    }
}

impl Drop for ProfileProbeServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_profile_probes(
    listener: TcpListener,
    cache_requests: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let connection_cache_requests = cache_requests.clone();
                let connection_error = error.clone();
                _ = thread::spawn(move || {
                    if let Err(server_error) = serve_connection(stream, &connection_cache_requests)
                    {
                        record_server_error(&connection_error, server_error.to_string());
                    }
                });
            }
            Err(server_error) if server_error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(server_error) => {
                record_server_error(&error, server_error.to_string());
                return;
            }
        }
    }
}

fn record_server_error(error: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut slot) = error.lock() {
        *slot = Some(message);
    }
}

fn serve_connection(mut stream: TcpStream, cache_requests: &AtomicUsize) -> Result<(), io::Error> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_http_request(&mut stream)?;
    let request_text = String::from_utf8_lossy(&request);
    let path = request_text
        .lines()
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .unwrap_or("/");

    if path.starts_with("/cache-token") {
        let request_number = cache_requests.fetch_add(1, Ordering::SeqCst) + 1;
        return write_http_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            "public, max-age=3600, immutable",
            None,
            format!("cache-{request_number}").as_bytes(),
        );
    }

    if path.starts_with("/probe") {
        let request_cookie = cookie_value(&request_text, "ely_profile").unwrap_or("empty");
        let seed = query_value(path, "value");
        let set_cookie =
            seed.map(|value| format!("ely_profile={value}; Path=/; Max-Age=3600; SameSite=Lax"));
        let body = profile_probe_html(request_cookie);
        return write_http_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            "no-store",
            set_cookie.as_deref(),
            body.as_bytes(),
        );
    }

    write_http_response(
        &mut stream,
        "404 Not Found",
        "text/plain; charset=utf-8",
        "no-store",
        None,
        b"missing",
    )
}

fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>, io::Error> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > 64 * 1024 {
            return Err(io::Error::other("profile probe request exceeded 64 KiB"));
        }
    }
    Ok(request)
}

fn write_http_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    cache_control: &str,
    set_cookie: Option<&str>,
    body: &[u8],
) -> Result<(), io::Error> {
    let cookie_header =
        set_cookie.map(|cookie| format!("Set-Cookie: {cookie}\r\n")).unwrap_or_default();
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: {cache_control}\r\n{cookie_header}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn cookie_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.lines().find_map(|line| {
        let (header, value) = line.split_once(':')?;
        if !header.eq_ignore_ascii_case("cookie") {
            return None;
        }
        value.split(';').find_map(|pair| {
            let (cookie_name, cookie_value) = pair.trim().split_once('=')?;
            (cookie_name == name).then_some(cookie_value)
        })
    })
}

fn query_value<'a>(path: &'a str, name: &str) -> Option<&'a str> {
    let (_, query) = path.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn profile_probe_html(request_cookie: &str) -> String {
    format!(
        "<!doctype html><title>loading</title><pre id=state>loading</pre><script>\
         const seed = new URLSearchParams(location.search).get('value');\
         if (seed) localStorage.setItem('ely_profile', seed);\
         const cookie = () => (document.cookie.match(/(?:^|; )ely_profile=([^;]*)/) || [,'empty'])[1];\
         fetch('/cache-token', {{cache:'force-cache'}}).then(response => response.text()).then(cache => {{\
           const title = 'request={request_cookie}|document=' + cookie() + '|storage=' + (localStorage.getItem('ely_profile') || 'empty') + '|cache=' + cache;\
           document.title = title; document.getElementById('state').textContent = title;\
         }});</script>",
    )
}
