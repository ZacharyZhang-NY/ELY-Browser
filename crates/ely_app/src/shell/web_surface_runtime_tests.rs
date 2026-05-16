use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use crate::{
    services::ProfileDataMode,
    shell::{
        WebSurfaceStore,
        web_surface_cadence::{ACTIVE_POLL_INTERVAL, IDLE_POLL_INTERVAL},
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::{WebSurfaceInputOutcome, WebSurfacePendingInput},
        web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
    },
};

use super::*;

use crate::services::servo_live::{ServoLiveEnsureRequest, ServoLiveFrame};

static FAKE_CLOSE_COUNT: AtomicUsize = AtomicUsize::new(0);
static FAKE_ENSURE_COUNT: AtomicUsize = AtomicUsize::new(0);
static IDLE_SKIP_ENSURE_COUNT: AtomicUsize = AtomicUsize::new(0);
static RECOVERY_FACTORY_COUNT: AtomicUsize = AtomicUsize::new(0);
static FAILING_ENSURE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn runtime_keeps_independent_clients_for_profile_scopes() -> Result<(), String> {
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(fake_client_factory);
    let first_profile = ProfileId::new();
    let second_profile = ProfileId::new();
    let first_tab = web_tab(TabId::new(), first_profile.clone(), "https://example.com/first")?;
    let second_tab = web_tab(TabId::new(), second_profile.clone(), "https://example.com/second")?;

    runtime.ensure_tab(
        &first_tab,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;
    runtime.ensure_tab(
        &second_tab,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;
    runtime.ensure_tab(
        &first_tab,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;

    runtime.flush_for_test();

    assert_eq!(runtime.client_count_for_test(), 2);
    assert_eq!(
        runtime.session_scope_for_test(first_tab.id()),
        Some(&WebSurfaceRuntimeScope::new(first_profile, ProfileDataMode::Transient)),
    );
    assert_eq!(
        runtime.session_scope_for_test(second_tab.id()),
        Some(&WebSurfaceRuntimeScope::new(second_profile, ProfileDataMode::Transient)),
    );
    Ok(())
}

#[test]
fn close_tab_removes_session_and_closes_client() -> Result<(), String> {
    let before = FAKE_CLOSE_COUNT.load(Ordering::SeqCst);
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(fake_client_factory);
    let tab = web_tab(TabId::new(), ProfileId::new(), "https://example.com/close")?;

    runtime.ensure_tab(&tab, surface_size(), ProfileDataMode::Transient, &[], pending_input())?;
    runtime.flush_for_test();
    runtime.close_tab(tab.id());
    runtime.flush_for_test();

    assert_eq!(runtime.session_scope_for_test(tab.id()), None);
    assert_eq!(FAKE_CLOSE_COUNT.load(Ordering::SeqCst), before + 1);

    runtime.close_tab(tab.id());
    runtime.flush_for_test();
    assert_eq!(FAKE_CLOSE_COUNT.load(Ordering::SeqCst), before + 1);
    Ok(())
}

#[test]
fn unchanged_surface_without_input_skips_runtime_ensure() -> Result<(), String> {
    IDLE_SKIP_ENSURE_COUNT.store(0, Ordering::SeqCst);
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        idle_skip_client_factory,
    ));
    let tab = web_tab(TabId::new(), ProfileId::new(), "https://example.com/idle")?;

    assert_eq!(
        store.record_viewport_size(tab.id(), viewport_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    assert_eq!(IDLE_SKIP_ENSURE_COUNT.load(Ordering::SeqCst), 1);

    assert!(!store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    assert_eq!(IDLE_SKIP_ENSURE_COUNT.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn store_tick_delay_tracks_runtime_cadence() -> Result<(), String> {
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        fake_client_factory,
    ));
    let tab = web_tab(TabId::new(), ProfileId::new(), "https://example.com/cadence")?;
    let visible = vec![tab.id().clone()];

    assert_eq!(store.next_tick_delay(&visible), IDLE_POLL_INTERVAL);
    assert_eq!(
        store.record_viewport_size(tab.id(), viewport_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    assert_eq!(store.next_tick_delay(&visible), Duration::ZERO);

    let _ = store.tick(&visible);
    let delay = store.next_tick_delay(&visible);

    assert!(delay <= ACTIVE_POLL_INTERVAL);
    assert!(delay > Duration::ZERO);
    Ok(())
}

#[test]
fn sidecar_exit_removes_dead_runtime_client() -> Result<(), String> {
    RECOVERY_FACTORY_COUNT.store(0, Ordering::SeqCst);
    let mut runtime = WebSurfaceRuntime::new_with_client_factory(recovery_client_factory);
    let profile = ProfileId::new();
    let crashed_tab = web_tab(TabId::new(), profile.clone(), "https://example.com/crash")?;

    runtime.ensure_tab(
        &crashed_tab,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;
    runtime.flush_for_test();
    let frames = runtime.tick(&[crashed_tab.id().clone()]);
    assert!(
        frames.iter().any(
            |frame| matches!(frame, WebSurfaceRuntimeFrame::Failed { tab_id, .. } if tab_id == crashed_tab.id())
        ),
        "the crashed tab must surface as a Failed frame",
    );
    assert_eq!(runtime.client_count_for_test(), 0);

    let next_tab = web_tab(TabId::new(), profile, "https://example.com/next")?;
    runtime.ensure_tab(
        &next_tab,
        surface_size(),
        ProfileDataMode::Transient,
        &[],
        pending_input(),
    )?;
    runtime.flush_for_test();

    assert_eq!(RECOVERY_FACTORY_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(runtime.client_count_for_test(), 1);
    Ok(())
}

#[test]
fn failed_surface_ensure_waits_for_a_new_key_before_retrying() -> Result<(), String> {
    FAILING_ENSURE_COUNT.store(0, Ordering::SeqCst);
    let mut store = WebSurfaceStore::new_with_runtime(WebSurfaceRuntime::new_with_client_factory(
        failing_client_factory,
    ));
    let tab = web_tab(TabId::new(), ProfileId::new(), "https://example.com/crash")?;

    assert_eq!(
        store.record_viewport_size(tab.id(), viewport_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    let tick = store.tick(&[tab.id().clone()]);
    assert!(tick.changed, "the failing client must surface a state change via tick");
    assert_eq!(FAILING_ENSURE_COUNT.load(Ordering::SeqCst), 1);

    assert!(!store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    let _ = store.tick(&[tab.id().clone()]);
    assert_eq!(FAILING_ENSURE_COUNT.load(Ordering::SeqCst), 1);

    assert_eq!(
        store.record_viewport_size(tab.id(), resized_viewport_bounds(), 1.0),
        WebSurfaceInputOutcome::Applied,
    );
    assert!(store.ensure_surface(&tab, ProfileDataMode::Transient, &[]));
    store.flush_runtime_for_test();
    let _ = store.tick(&[tab.id().clone()]);
    assert_eq!(FAILING_ENSURE_COUNT.load(Ordering::SeqCst), 2);
    Ok(())
}

#[test]
fn session_scope_change_resets_tab_state() {
    let tab_id = TabId::new();
    let first_scope = WebSurfaceRuntimeScope::new(ProfileId::new(), ProfileDataMode::Persistent);
    let second_scope = WebSurfaceRuntimeScope::new(ProfileId::new(), ProfileDataMode::Transient);
    let mut sessions = BTreeMap::new();

    let session = session_for_scope(&mut sessions, &tab_id, first_scope);
    session.requested_url = "https://example.com/old".to_string();
    session.size = surface_size();
    session.zoom_percent = 150;
    session.scroll_offset = WebSurfaceScrollOffset::default();
    session.pending_user_navigation = true;

    let session = session_for_scope(&mut sessions, &tab_id, second_scope.clone());

    assert_eq!(session.scope, second_scope);
    assert_eq!(session.requested_url, "");
    assert_eq!(session.size, WebSurfaceSize::default());
    assert_eq!(session.zoom_percent, 0);
    assert_eq!(session.scroll_offset, WebSurfaceScrollOffset::default());
    assert!(!session.pending_user_navigation);
}

struct FakeLiveRuntimeClient;
struct IdleSkipLiveRuntimeClient;
struct SidecarExitLiveRuntimeClient;
struct FailingLiveRuntimeClient;

impl LiveRuntimeClient for FakeLiveRuntimeClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        FAKE_ENSURE_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        FAKE_CLOSE_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl LiveRuntimeClient for IdleSkipLiveRuntimeClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        IDLE_SKIP_ENSURE_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

impl LiveRuntimeClient for SidecarExitLiveRuntimeClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::SidecarExited)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::SidecarExited)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Err(LiveRuntimeClientError::SidecarExited)
    }
}

impl LiveRuntimeClient for FailingLiveRuntimeClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        FAILING_ENSURE_COUNT.fetch_add(1, Ordering::SeqCst);
        Err(LiveRuntimeClientError::SidecarExited)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

fn fake_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(FakeLiveRuntimeClient))
}

fn idle_skip_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(IdleSkipLiveRuntimeClient))
}

fn recovery_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    let factory_call = RECOVERY_FACTORY_COUNT.fetch_add(1, Ordering::SeqCst);
    if factory_call == 0 {
        return Ok(Box::new(SidecarExitLiveRuntimeClient));
    }
    Ok(Box::new(FakeLiveRuntimeClient))
}

fn failing_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(FailingLiveRuntimeClient))
}

fn web_tab(tab_id: TabId, profile_id: ProfileId, url: &str) -> Result<BrowserTab, String> {
    let url = UrlText::parse(url).map_err(|error| error.to_string())?;
    Ok(BrowserTab::new(tab_id, SpaceId::new(), profile_id, "Web", url))
}

fn surface_size() -> WebSurfaceSize {
    WebSurfaceSize { width: 640, height: 480, device_pixel_ratio_percent: 100 }
}

fn viewport_bounds() -> gpui::Bounds<gpui::Pixels> {
    gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(640.0), gpui::px(480.0)),
    )
}

fn resized_viewport_bounds() -> gpui::Bounds<gpui::Pixels> {
    gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(720.0), gpui::px(480.0)),
    )
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
