use ely_domain::{
    BrowserTab, ProfileId, SiteOrigin, SitePermissionDecision, SitePermissionFeature, SpaceId,
    TabId, UrlText,
};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveEnsureRequest, ServoLiveFrame},
};

use super::{WebSurfaceSitePermission, WebSurfaceStore};
use crate::shell::{
    web_surface_permissions::WebSurfaceSitePermissionState,
    web_surface_runtime::WebSurfaceRuntime,
    web_surface_state::WebSurfaceInputOutcome,
    web_surface_worker::{LiveRuntimeClient, LiveRuntimeClientError},
};

struct AcceptingClient;
struct RejectingClient;
static REJECTED_ENSURE_COUNT: AtomicUsize = AtomicUsize::new(0);

impl LiveRuntimeClient for AcceptingClient {
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

impl LiveRuntimeClient for RejectingClient {
    fn ensure(
        &mut self,
        _request: ServoLiveEnsureRequest,
    ) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        REJECTED_ENSURE_COUNT.fetch_add(1, Ordering::SeqCst);
        Err(LiveRuntimeClientError::Message("ensure rejected".to_string()))
    }

    fn poll(&mut self, _tab_id: String) -> Result<Option<ServoLiveFrame>, LiveRuntimeClientError> {
        Ok(None)
    }

    fn close(&mut self, _tab_id: String) -> Result<(), LiveRuntimeClientError> {
        Ok(())
    }
}

#[test]
fn successful_worker_ensure_confirms_allow_once_transfer() -> Result<(), String> {
    let runtime = WebSurfaceRuntime::new_with_client_factory(|_| Ok(Box::new(AcceptingClient)));
    let mut store = WebSurfaceStore::new_with_runtime(runtime);
    let (tab, permission) = tab_and_permission()?;
    let profile_id = tab.profile_id().clone();

    record_viewport(&mut store, &tab);
    assert!(store.ensure_surface(
        &tab,
        ProfileDataMode::Transient,
        std::slice::from_ref(&permission),
    ));
    store.flush_runtime_for_test();
    let result = store.tick(std::slice::from_ref(tab.id()));

    assert_eq!(
        result.permission_transfers,
        vec![crate::services::servo_live::ServoLivePermissionGrant::new(
            profile_id,
            permission.origin().clone(),
            permission.feature(),
            permission.revision(),
        )],
    );
    Ok(())
}

#[test]
fn rejected_worker_ensure_keeps_allow_once_untransferred() -> Result<(), String> {
    REJECTED_ENSURE_COUNT.store(0, Ordering::SeqCst);
    let runtime = WebSurfaceRuntime::new_with_client_factory(|_| Ok(Box::new(RejectingClient)));
    let mut store = WebSurfaceStore::new_with_runtime(runtime);
    let (tab, permission) = tab_and_permission()?;
    record_viewport(&mut store, &tab);

    assert!(store.ensure_surface(
        &tab,
        ProfileDataMode::Transient,
        std::slice::from_ref(&permission),
    ));
    store.flush_runtime_for_test();
    let result = store.tick(std::slice::from_ref(tab.id()));

    assert!(result.permission_transfers.is_empty());
    let _ =
        store.ensure_surface(&tab, ProfileDataMode::Transient, std::slice::from_ref(&permission));
    store.flush_runtime_for_test();
    let _ = store.tick(std::slice::from_ref(tab.id()));
    assert_eq!(REJECTED_ENSURE_COUNT.load(Ordering::SeqCst), 2);
    Ok(())
}

fn tab_and_permission() -> Result<(BrowserTab, WebSurfaceSitePermission), String> {
    let profile_id = ProfileId::new();
    let tab = BrowserTab::new(
        TabId::new(),
        SpaceId::new(),
        profile_id,
        "Web",
        UrlText::parse("https://example.com/").map_err(|error| error.to_string())?,
    );
    let permission = WebSurfaceSitePermission::new(
        SiteOrigin::parse("https://example.com").map_err(|error| error.to_string())?,
        SitePermissionFeature::Camera,
        WebSurfaceSitePermissionState::Decision(SitePermissionDecision::AllowOnce),
        7,
    );
    Ok((tab, permission))
}

fn record_viewport(store: &mut WebSurfaceStore, tab: &BrowserTab) {
    let bounds = gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(640.0), gpui::px(480.0)),
    );
    assert_eq!(store.record_viewport_size(tab.id(), bounds, 1.0), WebSurfaceInputOutcome::Applied);
}
use std::sync::atomic::{AtomicUsize, Ordering};
