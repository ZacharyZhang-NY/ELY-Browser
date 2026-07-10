use std::{
    path::PathBuf,
    sync::{Mutex, mpsc},
    thread,
    time::Duration,
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveEnsureRequest, ServoLiveFrame},
};

use super::{
    super::{
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::WebSurfacePendingInput,
        web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
    },
    WebSurfaceRuntime,
};

struct SlowPollClient;

struct BlockingEnsureClient {
    ensure_started_tx: mpsc::Sender<()>,
    release_ensure_rx: mpsc::Receiver<()>,
}

struct BlockingEnsureSetup {
    ensure_started_tx: mpsc::Sender<()>,
    release_ensure_rx: mpsc::Receiver<()>,
}

static BLOCKING_ENSURE_SETUP: Mutex<Option<BlockingEnsureSetup>> = Mutex::new(None);

impl LiveRuntimeClient for SlowPollClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        thread::sleep(Duration::from_millis(100));
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

impl LiveRuntimeClient for BlockingEnsureClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        self.ensure_started_tx.send(()).map_err(|error| error.to_string())?;
        self.release_ensure_rx.recv().map_err(|error| error.to_string())?;
        Ok(Some(ServoLiveFrame::for_test(1, 1, vec![16, 32, 64, 255])))
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::Message("unexpected poll".to_string()))
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

fn slow_client_factory(_path: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(SlowPollClient))
}

fn blocking_ensure_client_factory(_path: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    let setup = BLOCKING_ENSURE_SETUP
        .lock()
        .map_err(|_| "blocking ensure setup lock was poisoned".to_string())?
        .take()
        .ok_or_else(|| "blocking ensure setup was missing".to_string())?;
    Ok(Box::new(BlockingEnsureClient {
        ensure_started_tx: setup.ensure_started_tx,
        release_ensure_rx: setup.release_ensure_rx,
    }))
}

#[test]
fn rejected_tick_poll_preserves_in_flight_ensure_generation() -> Result<(), String> {
    let (ensure_started_tx, ensure_started_rx) = mpsc::channel();
    let (release_ensure_tx, release_ensure_rx) = mpsc::channel();
    *BLOCKING_ENSURE_SETUP
        .lock()
        .map_err(|_| "blocking ensure setup lock was poisoned".to_string())? =
        Some(BlockingEnsureSetup { ensure_started_tx, release_ensure_rx });
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(blocking_ensure_client_factory);
    let tab = web_tab("Blocked ensure")?;

    runtime.ensure_tab(&tab, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    ensure_started_rx.recv_timeout(Duration::from_secs(1)).map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_millis(10));
    assert!(runtime.tick(std::slice::from_ref(tab.id())).is_empty());

    release_ensure_tx.send(()).map_err(|error| error.to_string())?;
    runtime.flush_for_test();
    let frames = runtime.tick(std::slice::from_ref(tab.id()));

    assert!(matches!(
        frames.as_slice(),
        [super::WebSurfaceRuntimeFrame::Ready { tab_id, .. }] if tab_id == tab.id()
    ));
    Ok(())
}

#[test]
fn pending_poll_advances_deadline_under_worker_backpressure() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(slow_client_factory);
    let tab = web_tab("Backpressure")?;
    runtime.ensure_tab(&tab, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();

    thread::sleep(Duration::from_millis(10));
    runtime.tick(std::slice::from_ref(tab.id()));
    thread::sleep(Duration::from_millis(10));
    runtime.tick(std::slice::from_ref(tab.id()));

    let delay = runtime
        .next_poll_delay(std::slice::from_ref(tab.id()), std::time::Instant::now())
        .ok_or_else(|| "visible tab lost its poll deadline".to_string())?;
    assert!(delay > Duration::ZERO, "backpressured poll must not schedule a zero-delay timer");
    Ok(())
}

fn web_tab(title: &str) -> Result<BrowserTab, String> {
    Ok(BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        ProfileId::new(),
        title,
        UrlText::parse("https://example.com/backpressure").map_err(|error| error.to_string())?,
    ))
}

fn surface_size() -> WebSurfaceSize {
    WebSurfaceSize { width: 640, height: 480, device_pixel_ratio_percent: 100 }
}

fn pending_input() -> WebSurfacePendingInput {
    WebSurfacePendingInput {
        enqueued_at: None,
        scroll_offset: WebSurfaceScrollOffset::default(),
        scroll_delta: None,
        scroll_point: None,
        click_point: None,
        hover_point: None,
        typed_text: None,
    }
}
