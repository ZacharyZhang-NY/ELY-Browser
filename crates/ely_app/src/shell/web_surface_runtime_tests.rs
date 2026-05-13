use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use crate::{
    services::ProfileDataMode,
    shell::{
        web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
        web_surface_state::WebSurfacePendingInput,
    },
};

use super::*;

static FAKE_CLOSE_COUNT: AtomicUsize = AtomicUsize::new(0);

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
    runtime.close_tab(tab.id());

    assert_eq!(runtime.session_scope_for_test(tab.id()), None);
    assert_eq!(FAKE_CLOSE_COUNT.load(Ordering::SeqCst), before + 1);

    runtime.close_tab(tab.id());
    assert_eq!(FAKE_CLOSE_COUNT.load(Ordering::SeqCst), before + 1);
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

impl LiveRuntimeClient for FakeLiveRuntimeClient {
    fn ensure(&mut self, _request: ServoLiveEnsureRequest) -> Result<Option<WebLiveFrame>, String> {
        Ok(None)
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<WebLiveFrame>, String> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), String> {
        FAKE_CLOSE_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn fake_client_factory(
    _config_dir: std::path::PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    Ok(Box::new(FakeLiveRuntimeClient))
}

fn web_tab(tab_id: TabId, profile_id: ProfileId, url: &str) -> Result<BrowserTab, String> {
    let url = UrlText::parse(url).map_err(|error| error.to_string())?;
    Ok(BrowserTab::new(tab_id, SpaceId::new(), profile_id, "Web", url))
}

fn surface_size() -> WebSurfaceSize {
    WebSurfaceSize { width: 640, height: 480, device_pixel_ratio_percent: 100 }
}

fn pending_input() -> WebSurfacePendingInput {
    WebSurfacePendingInput {
        scroll_offset: WebSurfaceScrollOffset::default(),
        scroll_delta: None,
        scroll_point: None,
        click_point: None,
        hover_point: None,
        typed_text: None,
    }
}
