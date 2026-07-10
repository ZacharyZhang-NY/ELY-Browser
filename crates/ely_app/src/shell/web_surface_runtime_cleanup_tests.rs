use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use crate::{
    services::{
        ProfileDataMode,
        servo_live::{ServoLiveEnsureRequest, ServoLiveFrame},
    },
    shell::{
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::WebSurfacePendingInput,
        web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
    },
};

use super::WebSurfaceRuntime;

static FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
static DROP_SETUP: Mutex<Option<DropSetup>> = Mutex::new(None);
static FACTORY_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

struct DropSetup {
    started_tx: mpsc::Sender<()>,
    release_rx: mpsc::Receiver<()>,
}

struct BlockingDropClient {
    started_tx: mpsc::Sender<()>,
    release_rx: mpsc::Receiver<()>,
}

impl LiveRuntimeClient for BlockingDropClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

impl Drop for BlockingDropClient {
    fn drop(&mut self) {
        let _ = self.started_tx.send(());
        let _ = self.release_rx.recv();
    }
}

struct EmptyClient;

impl LiveRuntimeClient for EmptyClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

#[test]
fn private_worker_creation_uses_fresh_storage_during_previous_cleanup() -> Result<(), String> {
    FACTORY_CALLS.store(0, Ordering::SeqCst);
    FACTORY_PATHS.lock().map_err(|_| "factory paths lock was poisoned")?.clear();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *DROP_SETUP.lock().map_err(|_| "drop setup lock was poisoned")? =
        Some(DropSetup { started_tx, release_rx });
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(cleanup_client_factory);
    let first = web_tab(ProfileId::new())?;
    let second = web_tab(ProfileId::new())?;

    runtime.ensure_tab(&first, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();
    runtime.close_tab(first.id());
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|error| format!("cleanup did not start: {error}"))?;

    runtime.ensure_tab(
        &second,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while FACTORY_CALLS.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(FACTORY_CALLS.load(Ordering::SeqCst), 2);
    let paths = FACTORY_PATHS.lock().map_err(|_| "factory paths lock was poisoned")?.clone();
    assert_eq!(paths.len(), 2);
    assert_ne!(paths[0], paths[1]);
    assert!(paths.iter().all(|path| path.is_dir()));

    release_tx.send(()).map_err(|error| error.to_string())?;
    runtime.flush_for_test();
    assert!(!paths[0].exists());
    assert!(paths[1].is_dir());
    Ok(())
}

fn cleanup_client_factory(
    config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    FACTORY_PATHS
        .lock()
        .map_err(|_| "factory paths lock was poisoned".to_string())?
        .push(config_dir);
    if FACTORY_CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
        let setup = DROP_SETUP
            .lock()
            .map_err(|_| "drop setup lock was poisoned".to_string())?
            .take()
            .ok_or_else(|| "drop setup was missing".to_string())?;
        Ok(Box::new(BlockingDropClient {
            started_tx: setup.started_tx,
            release_rx: setup.release_rx,
        }))
    } else {
        Ok(Box::new(EmptyClient))
    }
}

fn web_tab(profile_id: ProfileId) -> Result<BrowserTab, String> {
    Ok(BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        profile_id,
        "Private",
        UrlText::parse("https://example.com/private").map_err(|error| error.to_string())?,
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
