use std::{
    collections::BTreeMap,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use ely_domain::{BrowserTab, TabId};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveEnsureRequest, ServoLivePermissionGrant, ServoLiveSitePermission},
    servo_profile_data::cleanup_stale_transient_profile_data_dirs,
};

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::WebSurfaceSize,
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime_session::{
        WebSurfaceRuntimeScope, WebSurfaceSession, config_dir_for_scope, session_for_scope,
    },
    web_surface_runtime_wire::{
        allow_once_grants, input_requests_history_navigation, log_ensure_submitted,
        pending_input_kind, scroll_wire_fields,
    },
    web_surface_state::WebSurfacePendingInput,
    web_surface_worker::{LiveRuntimeClient, LiveRuntimeWorker, RequestGeneration, WorkerResponse},
};
use cleanup::{
    LiveRuntimeClientFactory, ScopedWorker, new_servo_live_client, shutdown_scoped_worker,
};

pub(super) use super::web_surface_runtime_session::{
    WebSurfaceEnsureResult, WebSurfaceRuntimeFrame, WebSurfaceUrlChange, WebSurfaceUrlChangeKind,
};

pub(super) struct WebSurfaceRuntime {
    workers: BTreeMap<WebSurfaceRuntimeScope, ScopedWorker>,
    sessions: BTreeMap<TabId, WebSurfaceSession>,
    retry_state: BTreeMap<WebSurfaceRuntimeScope, ScopeRetryState>,
    retired_workers: Vec<JoinHandle<Result<(), String>>>,
    retired_permission_consumptions: Vec<ServoLivePermissionGrant>,
    transient_cleanup_error: Option<String>,
    client_factory: LiveRuntimeClientFactory,
    last_generation: u64,
}

const SIDECAR_RESTART_BASE_DELAY: Duration = Duration::from_millis(250);
const SIDECAR_RESTART_MAX_DELAY: Duration = Duration::from_secs(5);
impl WebSurfaceRuntime {
    pub(super) fn new() -> Self {
        let transient_cleanup_error =
            cleanup_stale_transient_profile_data_dirs().err().map(|error| error.to_string());
        Self {
            workers: BTreeMap::new(),
            sessions: BTreeMap::new(),
            retry_state: BTreeMap::new(),
            retired_workers: Vec::new(),
            retired_permission_consumptions: Vec::new(),
            transient_cleanup_error,
            client_factory: new_servo_live_client,
            last_generation: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn new_with_client_factory(client_factory: LiveRuntimeClientFactory) -> Self {
        Self {
            workers: BTreeMap::new(),
            sessions: BTreeMap::new(),
            retry_state: BTreeMap::new(),
            retired_workers: Vec::new(),
            retired_permission_consumptions: Vec::new(),
            transient_cleanup_error: None,
            client_factory,
            last_generation: 0,
        }
    }

    pub(super) fn ensure_tab(
        &mut self,
        tab: &BrowserTab,
        size: WebSurfaceSize,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
        input: WebSurfacePendingInput,
    ) -> Result<WebSurfaceEnsureResult, String> {
        let scope = WebSurfaceRuntimeScope::new(tab.profile_id().clone(), profile_data_mode);
        self.prepare_tab_scope(tab.id(), tab.profile_id(), profile_data_mode)?;

        let requested_url = tab.url().as_str().to_string();
        let zoom_percent = tab.zoom_percent();
        let enqueued_at = input.enqueued_at;
        let input_kind = pending_input_kind(&input);
        let (scroll_delta_x, scroll_delta_y, scroll_point_x, scroll_point_y) =
            scroll_wire_fields(input.scroll_delta, input.scroll_point)?;
        let user_navigation_input = input_requests_history_navigation(&input);
        let next_scroll_offset = input.scroll_offset;
        let generation = self.next_request_generation();

        let submitted_at = Instant::now();
        let started_loading = {
            let session = session_for_scope(&mut self.sessions, tab.id(), scope.clone());
            let started_loading = session.started_loading(&requested_url, size, zoom_percent);
            if started_loading {
                session.pending_user_navigation = false;
            }
            if user_navigation_input {
                session.pending_user_navigation = true;
            }
            session.requested_url = requested_url.clone();
            session.size = size;
            session.zoom_percent = zoom_percent;
            session.scroll_offset = next_scroll_offset;
            session.generation = Some(generation);
            session.cadence.note_ensure(input_kind, started_loading, submitted_at);
            started_loading
        };

        let request = ServoLiveEnsureRequest {
            tab_id: tab.id().as_str().to_string(),
            profile_id: tab.profile_id().as_str().to_string(),
            url: requested_url.clone(),
            width: size.width,
            height: size.height,
            page_zoom_percent: zoom_percent,
            device_pixel_ratio: size.device_pixel_ratio_f32(),
            scroll_delta_x,
            scroll_delta_y,
            scroll_point_x,
            scroll_point_y,
            click_x: input.click_point.map(|point| point.x()),
            click_y: input.click_point.map(|point| point.y()),
            hover_x: input.hover_point.map(|point| point.x()),
            hover_y: input.hover_point.map(|point| point.y()),
            typed_text: input.typed_text,
            site_permission_generation: generation.value(),
            site_permissions: permissions.iter().map(ServoLiveSitePermission::from).collect(),
            allow_once_grants: allow_once_grants(tab.profile_id(), permissions),
        };

        let Some(scoped) = self.workers.get_mut(&scope) else {
            return Err("Servo sidecar worker was created but is no longer registered".to_string());
        };
        scoped.track_allow_once_grants(&request.allow_once_grants);
        scoped.worker.submit_ensure(generation, request);
        if let Some(session) = self.sessions.get_mut(tab.id()) {
            session.cadence.note_poll_submitted(submitted_at);
        }
        log_ensure_submitted(tab, size, input_kind.label(), enqueued_at, started_loading);

        Ok(WebSurfaceEnsureResult { requested_url, started_loading })
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> Vec<WebSurfaceRuntimeFrame> {
        self.reap_retired_workers(false);
        let mut frames = self.take_retired_permission_frames();
        let now = Instant::now();
        let scopes = self.workers.keys().cloned().collect::<Vec<_>>();
        let mut unavailable_scopes = Vec::new();

        for scope in scopes {
            let responses = self
                .workers
                .get(&scope)
                .map(|scoped| scoped.worker.drain_responses())
                .unwrap_or_default();
            if self.collect_responses(&scope, responses, now, &mut frames) {
                unavailable_scopes.push(scope);
            }
        }
        for scope in unavailable_scopes {
            self.invalidate_scope(&scope, now);
        }
        frames.extend(self.take_retired_permission_frames());

        let poll_now = Instant::now();
        for tab_id in visible_tab_ids {
            let Some((scope, generation)) = self.sessions.get(tab_id).and_then(|session| {
                session
                    .cadence
                    .should_poll(poll_now)
                    .then(|| (session.scope.clone(), session.generation))
            }) else {
                continue;
            };
            let Some(generation) = generation else {
                continue;
            };
            if !self.workers.contains_key(&scope) {
                continue;
            }
            let _ = self.workers.get(&scope).is_some_and(|scoped| {
                scoped.worker.submit_poll(generation, tab_id.as_str().to_string())
            });
            if let Some(session) = self.sessions.get_mut(tab_id) {
                session.cadence.note_poll_submitted(poll_now);
            }
        }

        frames
    }

    pub(super) fn next_poll_delay(
        &self,
        visible_tab_ids: &[TabId],
        now: Instant,
    ) -> Option<Duration> {
        visible_tab_ids
            .iter()
            .filter_map(|tab_id| self.sessions.get(tab_id))
            .filter(|session| self.workers.contains_key(&session.scope))
            .map(|session| session.cadence.next_poll_delay(now))
            .min()
    }

    pub(super) fn has_session(
        &self,
        tab_id: &TabId,
        profile_id: &ely_domain::ProfileId,
        profile_data_mode: ProfileDataMode,
    ) -> bool {
        self.sessions.get(tab_id).is_some_and(|session| {
            session.scope == WebSurfaceRuntimeScope::new(profile_id.clone(), profile_data_mode)
                && self.workers.contains_key(&session.scope)
        })
    }

    pub(super) fn prepare_tab_scope(
        &mut self,
        tab_id: &TabId,
        profile_id: &ely_domain::ProfileId,
        profile_data_mode: ProfileDataMode,
    ) -> Result<(), String> {
        let scope = WebSurfaceRuntimeScope::new(profile_id.clone(), profile_data_mode);
        self.detach_tab_from_previous_scope(tab_id, &scope);
        self.ensure_worker(scope.clone())?;
        session_for_scope(&mut self.sessions, tab_id, scope);
        Ok(())
    }

    fn ensure_worker(&mut self, scope: WebSurfaceRuntimeScope) -> Result<(), String> {
        self.reap_retired_workers(false);
        if self.workers.contains_key(&scope) {
            return Ok(());
        }
        if scope.is_transient() && self.transient_cleanup_error.is_some() {
            match cleanup_stale_transient_profile_data_dirs() {
                Ok(()) => self.transient_cleanup_error = None,
                Err(error) => {
                    let message = error.to_string();
                    self.transient_cleanup_error = Some(message.clone());
                    return Err(message);
                }
            }
        }
        let now = Instant::now();
        if let Some(retry) = self.retry_state.get(&scope)
            && now < retry.retry_after
        {
            return Err("Servo sidecar restart is cooling down".to_string());
        }
        let (config_dir, transient_profile_data_dir) = config_dir_for_scope(&scope)?;
        let client_factory = self.client_factory;
        let worker = match LiveRuntimeWorker::new(move || client_factory(config_dir)) {
            Ok(worker) => worker,
            Err(error) => {
                self.note_scope_failure(&scope, now);
                return Err(error);
            }
        };
        self.workers.insert(scope, ScopedWorker::new(worker, transient_profile_data_dir));
        Ok(())
    }

    fn collect_responses(
        &mut self,
        scope: &WebSurfaceRuntimeScope,
        responses: Vec<WorkerResponse>,
        now: Instant,
        frames: &mut Vec<WebSurfaceRuntimeFrame>,
    ) -> bool {
        let mut runtime_unavailable = false;
        for response in responses {
            match response {
                WorkerResponse::Frame { generation, tab_id, frame } => {
                    let Some(tab_id_obj) = self.lookup_session_tab_id(&tab_id) else {
                        continue;
                    };
                    if !self.sessions.get(&tab_id_obj).is_some_and(|session| {
                        &session.scope == scope && session.generation == Some(generation)
                    }) {
                        continue;
                    }
                    self.note_scope_success(scope);
                    let Some(session) = self.sessions.get_mut(&tab_id_obj) else {
                        continue;
                    };
                    let requested_url = session.requested_url.clone();
                    let scroll_offset = session.scroll_offset;
                    let zoom_percent = session.zoom_percent;
                    session.cadence.note_frame(frame.render_state(), frame.pixels_changed(), now);
                    match WebSurfaceFrame::from_live_frame(
                        requested_url.clone(),
                        scroll_offset,
                        zoom_percent,
                        frame,
                    ) {
                        Ok(frame) => {
                            let url_change =
                                session.url_change_for(&tab_id_obj, requested_url.as_str(), &frame);
                            frames.push(WebSurfaceRuntimeFrame::Ready {
                                tab_id: tab_id_obj,
                                frame: Box::new(frame),
                                url_change,
                            });
                        }
                        Err(error) => frames.push(WebSurfaceRuntimeFrame::Failed {
                            tab_id: tab_id_obj,
                            message: error.to_string(),
                        }),
                    }
                }
                WorkerResponse::Failed { generation, tab_id, message } => {
                    let Some(tab_id_obj) = self.lookup_session_tab_id(&tab_id) else {
                        continue;
                    };
                    if self.sessions.get(&tab_id_obj).is_some_and(|session| {
                        &session.scope == scope && session.generation == Some(generation)
                    }) {
                        frames.push(WebSurfaceRuntimeFrame::Failed { tab_id: tab_id_obj, message });
                    }
                }
                WorkerResponse::RuntimeUnavailable => runtime_unavailable = true,
                WorkerResponse::PermissionSnapshotAccepted(grant) => {
                    frames.push(WebSurfaceRuntimeFrame::PermissionSnapshotAccepted(grant));
                }
                WorkerResponse::PermissionConsumed(consumed) => {
                    if let Some(scoped) = self.workers.get_mut(scope) {
                        scoped.mark_permission_consumed(&consumed);
                    }
                    frames.push(WebSurfaceRuntimeFrame::PermissionConsumed(consumed));
                }
            }
        }
        runtime_unavailable
    }

    fn lookup_session_tab_id(&self, tab_id: &str) -> Option<TabId> {
        self.sessions.keys().find(|key| key.as_str() == tab_id).cloned()
    }

    fn next_request_generation(&mut self) -> RequestGeneration {
        assert!(self.last_generation < u64::MAX, "web surface request generation exhausted");
        self.last_generation += 1;
        RequestGeneration::new(self.last_generation)
    }

    #[cfg(test)]
    pub(super) fn client_count_for_test(&self) -> usize {
        self.workers.len()
    }

    #[cfg(test)]
    pub(super) fn session_scope_for_test(&self, tab_id: &TabId) -> Option<&WebSurfaceRuntimeScope> {
        self.sessions.get(tab_id).map(|session| &session.scope)
    }

    #[cfg(test)]
    pub(super) fn flush_for_test(&mut self) {
        for scoped in self.workers.values() {
            scoped.worker.wait_until_idle();
        }
        self.reap_retired_workers(true);
    }

    fn remove_worker(&mut self, scope: &WebSurfaceRuntimeScope) {
        let Some(mut scoped) = self.workers.remove(scope) else {
            return;
        };
        self.retire_scoped_worker_permissions(&mut scoped);
        if scoped.transient_profile_data_dir.is_some() {
            match std::thread::Builder::new()
                .name("ely-servo-profile-cleanup".to_string())
                .spawn(move || shutdown_scoped_worker(scoped))
            {
                Ok(handle) => self.retired_workers.push(handle),
                Err(error) => self.transient_cleanup_error = Some(error.to_string()),
            }
        } else {
            let _ = shutdown_scoped_worker(scoped);
        }
    }

    fn reap_retired_workers(&mut self, wait_for_all: bool) {
        let mut pending = Vec::new();
        for handle in self.retired_workers.drain(..) {
            if !wait_for_all && !handle.is_finished() {
                pending.push(handle);
                continue;
            }
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => self.transient_cleanup_error = Some(error),
                Err(_) => {
                    self.transient_cleanup_error =
                        Some("Servo profile cleanup thread panicked".to_string());
                }
            }
        }
        self.retired_workers = pending;
    }

    fn invalidate_scope(&mut self, scope: &WebSurfaceRuntimeScope, now: Instant) {
        self.remove_worker(scope);
        self.sessions.retain(|_, session| &session.scope != scope);
        self.note_scope_failure(scope, now);
    }

    fn note_scope_failure(&mut self, scope: &WebSurfaceRuntimeScope, now: Instant) {
        let retry = ScopeRetryState::after_failure(self.retry_state.get(scope), now);
        self.retry_state.insert(scope.clone(), retry);
    }

    fn note_scope_success(&mut self, scope: &WebSurfaceRuntimeScope) {
        self.retry_state.remove(scope);
    }
}

impl Drop for WebSurfaceRuntime {
    fn drop(&mut self) {
        let scopes = self.workers.keys().cloned().collect::<Vec<_>>();
        for scope in scopes {
            self.remove_worker(&scope);
        }
        self.reap_retired_workers(true);
    }
}

#[derive(Clone, Copy, Debug)]
struct ScopeRetryState {
    failure_count: u32,
    retry_after: Instant,
}

impl ScopeRetryState {
    fn after_failure(previous: Option<&Self>, now: Instant) -> Self {
        let failure_count = previous.map_or(1, |state| state.failure_count.saturating_add(1));
        let shift = failure_count.saturating_sub(1).min(31);
        let multiplier = 1_u32 << shift;
        let delay =
            SIDECAR_RESTART_BASE_DELAY.saturating_mul(multiplier).min(SIDECAR_RESTART_MAX_DELAY);
        Self { failure_count, retry_after: now + delay }
    }
}

#[path = "web_surface_runtime_cleanup.rs"]
mod cleanup;

#[cfg(test)]
#[path = "web_surface_runtime_backpressure_tests.rs"]
mod backpressure_tests;
#[cfg(test)]
#[path = "web_surface_runtime_cleanup_tests.rs"]
mod cleanup_tests;
#[cfg(test)]
#[path = "web_surface_runtime_generation_tests.rs"]
mod generation_tests;
#[cfg(test)]
#[path = "web_surface_runtime_retry_tests.rs"]
mod retry_tests;
#[cfg(test)]
#[path = "web_surface_runtime_tests.rs"]
mod tests;
