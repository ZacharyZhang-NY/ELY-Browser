use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
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
        web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError, WorkerResponse},
    },
    WebSurfaceRuntime, WebSurfaceRuntimeFrame, WebSurfaceRuntimeScope,
};

struct EmptyClient;

static FAILING_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
static FAILING_FACTORY_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);

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

struct CloseRecordingClient;

impl LiveRuntimeClient for CloseRecordingClient {
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

impl Drop for CloseRecordingClient {
    fn drop(&mut self) {
        FAILING_FACTORY_SHUTDOWNS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn late_frame_from_a_is_discarded_after_a_to_b() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(empty_client_factory);
    let tab_id = TabId::new();
    let profile_a = ProfileId::new();
    let profile_b = ProfileId::new();
    let scope_a = scope(&profile_a);
    let tab_a = web_tab(tab_id.clone(), profile_a, "https://example.com/a")?;
    let tab_b = web_tab(tab_id.clone(), profile_b, "https://example.com/b")?;

    runtime.ensure_tab(&tab_a, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let generation_a = runtime
        .sessions
        .get(&tab_id)
        .and_then(|session| session.generation)
        .ok_or_else(|| "profile A ensure generation was missing".to_string())?;
    runtime.ensure_tab(&tab_b, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let generation_b = runtime
        .sessions
        .get(&tab_id)
        .and_then(|session| session.generation)
        .ok_or_else(|| "profile B ensure generation was missing".to_string())?;

    let mut frames = Vec::new();
    runtime.collect_responses(
        &scope_a,
        vec![WorkerResponse::Frame {
            generation: generation_a,
            tab_id: tab_id.as_str().to_string(),
            frame: live_frame(),
        }],
        Instant::now(),
        &mut frames,
    );

    assert!(generation_a < generation_b);
    assert!(frames.is_empty());
    Ok(())
}

#[test]
fn generation_blocks_aba_frames_and_failures() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(empty_client_factory);
    let tab_id = TabId::new();
    let profile_a = ProfileId::new();
    let profile_b = ProfileId::new();
    let scope_a = scope(&profile_a);
    let tab_a = web_tab(tab_id.clone(), profile_a, "https://example.com/a")?;
    let tab_b = web_tab(tab_id.clone(), profile_b, "https://example.com/b")?;

    runtime.ensure_tab(&tab_a, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let first_a = current_generation(&runtime, &tab_id)?;
    runtime.ensure_tab(&tab_b, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let generation_b = current_generation(&runtime, &tab_id)?;
    runtime.ensure_tab(&tab_a, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let second_a = current_generation(&runtime, &tab_id)?;

    let mut frames = Vec::new();
    runtime.collect_responses(
        &scope_a,
        vec![
            WorkerResponse::Frame {
                generation: first_a,
                tab_id: tab_id.as_str().to_string(),
                frame: live_frame(),
            },
            WorkerResponse::Failed {
                generation: first_a,
                tab_id: tab_id.as_str().to_string(),
                message: "stale failure".to_string(),
            },
        ],
        Instant::now(),
        &mut frames,
    );
    assert!(frames.is_empty());

    runtime.collect_responses(
        &scope_a,
        vec![WorkerResponse::Frame {
            generation: second_a,
            tab_id: tab_id.as_str().to_string(),
            frame: live_frame(),
        }],
        Instant::now(),
        &mut frames,
    );

    assert!(first_a < generation_b && generation_b < second_a);
    assert!(matches!(
        frames.as_slice(),
        [WebSurfaceRuntimeFrame::Ready { tab_id: ready_tab_id, .. }] if ready_tab_id == &tab_id
    ));
    Ok(())
}

#[test]
fn scope_creation_failure_invalidates_previous_session_generation() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(empty_client_factory);
    let tab_id = TabId::new();
    let profile_a = ProfileId::new();
    let profile_b = ProfileId::new();
    let scope_a = scope(&profile_a);
    let scope_b = scope(&profile_b);
    let tab_a = web_tab(tab_id.clone(), profile_a, "https://example.com/a")?;
    runtime.ensure_tab(&tab_a, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    let generation_a = current_generation(&runtime, &tab_id)?;
    runtime.retry_state.insert(
        scope_b,
        super::ScopeRetryState {
            failure_count: 1,
            retry_after: Instant::now() + std::time::Duration::from_secs(1),
        },
    );

    assert!(runtime.prepare_tab_scope(&tab_id, &profile_b, ProfileDataMode::Transient).is_err());
    assert!(!runtime.sessions.contains_key(&tab_id));

    let mut frames = Vec::new();
    runtime.collect_responses(
        &scope_a,
        vec![WorkerResponse::Frame {
            generation: generation_a,
            tab_id: tab_id.as_str().to_string(),
            frame: live_frame(),
        }],
        Instant::now(),
        &mut frames,
    );
    assert!(frames.is_empty());
    Ok(())
}

#[test]
fn stale_frame_preserves_scope_backoff_until_current_frame() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(empty_client_factory);
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();
    let scope = scope(&profile_id);
    let tab = web_tab(tab_id.clone(), profile_id, "https://example.com/current")?;
    runtime.ensure_tab(&tab, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();
    let current = current_generation(&runtime, &tab_id)?;
    runtime.note_scope_failure(&scope, Instant::now());

    let mut frames = Vec::new();
    runtime.collect_responses(
        &scope,
        vec![WorkerResponse::Frame {
            generation: super::super::web_surface_worker::RequestGeneration::new(0),
            tab_id: tab_id.as_str().to_string(),
            frame: live_frame(),
        }],
        Instant::now(),
        &mut frames,
    );
    assert!(frames.is_empty());
    assert!(runtime.retry_state.contains_key(&scope));

    runtime.collect_responses(
        &scope,
        vec![WorkerResponse::Frame {
            generation: current,
            tab_id: tab_id.as_str().to_string(),
            frame: live_frame(),
        }],
        Instant::now(),
        &mut frames,
    );
    assert!(matches!(frames.as_slice(), [WebSurfaceRuntimeFrame::Ready { .. }]));
    assert!(!runtime.retry_state.contains_key(&scope));
    Ok(())
}

#[test]
fn previous_scope_shuts_down_when_new_worker_factory_fails_async() -> Result<(), String> {
    FAILING_FACTORY_CALLS.store(0, Ordering::SeqCst);
    FAILING_FACTORY_SHUTDOWNS.store(0, Ordering::SeqCst);
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(fail_second_client_factory);
    let tab_id = TabId::new();
    let tab_a = web_tab(tab_id.clone(), ProfileId::new(), "https://example.com/a")?;
    let tab_b = web_tab(tab_id, ProfileId::new(), "https://example.com/b")?;

    runtime.ensure_tab(&tab_a, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();
    runtime.ensure_tab(&tab_b, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();
    let frames = runtime.tick(std::slice::from_ref(tab_b.id()));

    assert_eq!(FAILING_FACTORY_SHUTDOWNS.load(Ordering::SeqCst), 1);
    assert!(matches!(
        frames.as_slice(),
        [WebSurfaceRuntimeFrame::Failed { message, .. }]
            if message == "injected client creation failure"
    ));
    Ok(())
}

fn current_generation(
    runtime: &WebSurfaceRuntime,
    tab_id: &TabId,
) -> Result<super::super::web_surface_worker::RequestGeneration, String> {
    runtime
        .sessions
        .get(tab_id)
        .and_then(|session| session.generation)
        .ok_or_else(|| "session generation was missing".to_string())
}

fn empty_client_factory(_path: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(EmptyClient))
}

fn fail_second_client_factory(_path: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    if FAILING_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
        Ok(Box::new(CloseRecordingClient))
    } else {
        Err("injected client creation failure".to_string())
    }
}

fn scope(profile_id: &ProfileId) -> WebSurfaceRuntimeScope {
    WebSurfaceRuntimeScope::new(profile_id.clone(), ProfileDataMode::Transient)
}

fn web_tab(tab_id: TabId, profile_id: ProfileId, url: &str) -> Result<BrowserTab, String> {
    Ok(BrowserTab::new(
        tab_id,
        SpaceId::new(),
        profile_id,
        "Web",
        UrlText::parse(url).map_err(|error| error.to_string())?,
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

fn live_frame() -> ServoLiveFrame {
    ServoLiveFrame::for_test(1, 1, vec![16, 32, 64, 255])
}
