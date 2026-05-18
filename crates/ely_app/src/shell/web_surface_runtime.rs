use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use ely_domain::{BrowserTab, ProfileId, TabId};
use gpui::NativeSurfaceHandle;

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveClient, ServoLiveEnsureRequest, ServoLiveSitePermission},
    servo_profile_data::{default_profile_data_root, profile_data_dir, transient_profile_data_dir},
};

use super::{
    web_surface_cadence::{WebSurfaceInputKind, WebSurfacePollCadence},
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_state::WebSurfacePendingInput,
    web_surface_worker::{LiveRuntimeClient, LiveRuntimeWorker, WorkerResponse},
};

pub(super) struct WebSurfaceRuntime {
    worker: Option<ScopedWorker>,
    direct_client: Option<ScopedDirectClient>,
    pending_direct_responses: Vec<WorkerResponse>,
    sessions: BTreeMap<TabId, WebSurfaceSession>,
    client_factory: LiveRuntimeClientFactory,
}

impl WebSurfaceRuntime {
    pub(super) fn new() -> Self {
        Self {
            worker: None,
            direct_client: None,
            pending_direct_responses: Vec::new(),
            sessions: BTreeMap::new(),
            client_factory: new_servo_live_client,
        }
    }

    #[cfg(test)]
    pub(super) fn new_with_client_factory(client_factory: LiveRuntimeClientFactory) -> Self {
        Self {
            worker: None,
            direct_client: None,
            pending_direct_responses: Vec::new(),
            sessions: BTreeMap::new(),
            client_factory,
        }
    }

    #[cfg(test)]
    pub(super) fn ensure_tab(
        &mut self,
        tab: &BrowserTab,
        size: WebSurfaceSize,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
        input: WebSurfacePendingInput,
    ) -> Result<WebSurfaceEnsureResult, String> {
        self.ensure_tab_inner(tab, size, None, profile_data_mode, permissions, input)
    }

    pub(super) fn ensure_tab_with_native_surface(
        &mut self,
        tab: &BrowserTab,
        size: WebSurfaceSize,
        native_surface: NativeSurfaceHandle,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
        input: WebSurfacePendingInput,
    ) -> Result<WebSurfaceEnsureResult, String> {
        self.ensure_tab_inner(
            tab,
            size,
            Some(native_surface),
            profile_data_mode,
            permissions,
            input,
        )
    }

    fn ensure_tab_inner(
        &mut self,
        tab: &BrowserTab,
        size: WebSurfaceSize,
        native_surface: Option<NativeSurfaceHandle>,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
        input: WebSurfacePendingInput,
    ) -> Result<WebSurfaceEnsureResult, String> {
        let scope = WebSurfaceRuntimeScope::new(tab.profile_id().clone(), profile_data_mode);
        let use_direct_client = native_surface.is_some();
        if use_direct_client {
            self.ensure_direct_client(scope.clone())?;
        } else {
            self.ensure_worker(scope.clone())?;
        }

        let requested_url = tab.url().as_str().to_string();
        let tab_id_string = tab.id().as_str().to_string();
        let zoom_percent = tab.zoom_percent();
        let enqueued_at = input.enqueued_at;
        let input_kind = pending_input_kind(&input);
        let (scroll_delta_x, scroll_delta_y, scroll_point_x, scroll_point_y) =
            scroll_wire_fields(input.scroll_delta, input.scroll_point)?;
        let user_navigation_input = input_requests_history_navigation(&input);
        let next_scroll_offset = input.scroll_offset;

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
            native_surface,
            scroll_delta_x,
            scroll_delta_y,
            scroll_point_x,
            scroll_point_y,
            click_x: input.click_point.map(|point| point.x()),
            click_y: input.click_point.map(|point| point.y()),
            hover_x: input.hover_point.map(|point| point.x()),
            hover_y: input.hover_point.map(|point| point.y()),
            typed_text: input.typed_text,
            site_permissions: permissions.iter().map(ServoLiveSitePermission::from).collect(),
        };

        if use_direct_client {
            let response = self.ensure_direct(request, tab_id_string.clone())?;
            self.pending_direct_responses.extend(response);
        } else {
            let Some(scoped) = self.worker.as_ref() else {
                return Err("Servo worker was created but is no longer registered".to_string());
            };
            scoped.worker.submit_ensure(request);
        }
        if let Some(session) = self.sessions.get_mut(tab.id()) {
            session.cadence.note_poll_submitted(submitted_at);
        }
        log_ensure_submitted(tab, size, input_kind.label(), enqueued_at, started_loading);

        Ok(WebSurfaceEnsureResult { requested_url, started_loading })
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> Vec<WebSurfaceRuntimeFrame> {
        let mut frames = Vec::new();
        let now = Instant::now();
        let mut responses = std::mem::take(&mut self.pending_direct_responses);
        responses.extend(
            self.worker.as_ref().map(|scoped| scoped.worker.drain_responses()).unwrap_or_default(),
        );
        let runtime_unavailable = self.collect_responses(responses, now, &mut frames);
        if runtime_unavailable {
            self.remove_worker();
            self.remove_direct_client();
        }

        let poll_now = Instant::now();
        let mut direct_polls = Vec::new();
        for tab_id in visible_tab_ids {
            let Some(session) = self.sessions.get_mut(tab_id) else {
                continue;
            };
            if !session.cadence.should_poll(poll_now) {
                continue;
            }
            if let Some(scoped) = self.worker.as_ref() {
                let _ = scoped.worker.submit_poll(tab_id.as_str().to_string());
                session.cadence.note_poll_submitted(poll_now);
            } else if self.direct_client.is_some() {
                direct_polls.push(tab_id.as_str().to_string());
                session.cadence.note_poll_submitted(poll_now);
            }
        }

        if !direct_polls.is_empty() {
            let (responses, runtime_unavailable) = self.poll_direct(direct_polls);
            let unavailable_from_responses =
                self.collect_responses(responses, Instant::now(), &mut frames);
            if runtime_unavailable || unavailable_from_responses {
                self.remove_direct_client();
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
            .filter(|_| self.worker.is_some() || self.direct_client.is_some())
            .map(|session| session.cadence.next_poll_delay(now))
            .min()
    }

    pub(super) fn close_tab(&mut self, tab_id: &TabId) {
        if self.sessions.remove(tab_id).is_none() {
            return;
        }
        let direct_result = self
            .direct_client
            .as_mut()
            .map(|scoped| scoped.client.close(tab_id.as_str().to_string()));
        if direct_result.as_ref().is_some_and(|result| {
            result.as_ref().is_err_and(|error| error.is_runtime_unavailable())
        }) {
            self.remove_direct_client();
        }
        if let Some(scoped) = self.worker.as_ref() {
            scoped.worker.submit_close(tab_id.as_str().to_string());
        }
    }

    fn ensure_worker(&mut self, scope: WebSurfaceRuntimeScope) -> Result<(), String> {
        if self.worker.is_some() {
            return Ok(());
        }
        let (config_dir, transient_profile_data_dir) = config_dir_for_scope(&scope)?;
        let client_factory = self.client_factory;
        let worker = LiveRuntimeWorker::new(move || client_factory(config_dir))?;
        self.worker = Some(ScopedWorker { worker, transient_profile_data_dir });
        Ok(())
    }

    fn ensure_direct_client(&mut self, scope: WebSurfaceRuntimeScope) -> Result<(), String> {
        if self.direct_client.is_some() {
            return Ok(());
        }
        let (config_dir, transient_profile_data_dir) = config_dir_for_scope(&scope)?;
        let client = (self.client_factory)(config_dir)?;
        self.direct_client = Some(ScopedDirectClient { client, transient_profile_data_dir });
        Ok(())
    }

    fn ensure_direct(
        &mut self,
        request: ServoLiveEnsureRequest,
        tab_id: String,
    ) -> Result<Option<WorkerResponse>, String> {
        let Some(scoped) = self.direct_client.as_mut() else {
            return Err("Servo client was created but is no longer registered".to_string());
        };
        match scoped.client.ensure(request) {
            Ok(Some(frame)) => Ok(Some(WorkerResponse::Frame { tab_id, frame })),
            Ok(None) => Ok(None),
            Err(error) => {
                let message = error.to_string();
                if error.is_runtime_unavailable() {
                    self.remove_direct_client();
                }
                Err(message)
            }
        }
    }

    fn poll_direct(&mut self, tab_ids: Vec<String>) -> (Vec<WorkerResponse>, bool) {
        let Some(scoped) = self.direct_client.as_mut() else {
            return (Vec::new(), false);
        };
        let mut responses = Vec::new();
        let mut runtime_unavailable = false;
        for tab_id in tab_ids {
            match scoped.client.poll(tab_id.clone()) {
                Ok(Some(frame)) => responses.push(WorkerResponse::Frame { tab_id, frame }),
                Ok(None) => {}
                Err(error) if error.is_runtime_unavailable() => {
                    runtime_unavailable = true;
                    responses.push(WorkerResponse::RuntimeUnavailable);
                }
                Err(error) => {
                    responses.push(WorkerResponse::Failed { tab_id, message: error.to_string() })
                }
            }
        }
        (responses, runtime_unavailable)
    }

    fn collect_responses(
        &mut self,
        responses: Vec<WorkerResponse>,
        now: Instant,
        frames: &mut Vec<WebSurfaceRuntimeFrame>,
    ) -> bool {
        let mut runtime_unavailable = false;
        for response in responses {
            match response {
                WorkerResponse::Frame { tab_id, frame } => {
                    let Some(tab_id_obj) = self.lookup_session_tab_id(&tab_id) else {
                        continue;
                    };
                    let session = match self.sessions.get_mut(&tab_id_obj) {
                        Some(session) => session,
                        None => continue,
                    };
                    let requested_url = session.requested_url.clone();
                    let scroll_offset = session.scroll_offset;
                    let zoom_percent = session.zoom_percent;
                    session.cadence.note_frame(frame.render_state(), now);
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
                WorkerResponse::Failed { tab_id, message } => {
                    let Some(tab_id_obj) = self.lookup_session_tab_id(&tab_id) else {
                        continue;
                    };
                    frames.push(WebSurfaceRuntimeFrame::Failed { tab_id: tab_id_obj, message });
                }
                WorkerResponse::RuntimeUnavailable => {
                    runtime_unavailable = true;
                }
            }
        }
        runtime_unavailable
    }

    fn lookup_session_tab_id(&self, tab_id: &str) -> Option<TabId> {
        self.sessions.keys().find(|key| key.as_str() == tab_id).cloned()
    }

    #[cfg(test)]
    pub(super) fn client_count_for_test(&self) -> usize {
        usize::from(self.worker.is_some()) + usize::from(self.direct_client.is_some())
    }

    #[cfg(test)]
    pub(super) fn session_scope_for_test(&self, tab_id: &TabId) -> Option<&WebSurfaceRuntimeScope> {
        self.sessions.get(tab_id).map(|session| &session.scope)
    }

    #[cfg(test)]
    pub(super) fn flush_for_test(&self) {
        if let Some(scoped) = self.worker.as_ref() {
            scoped.worker.wait_until_idle();
        }
    }

    fn remove_worker(&mut self) {
        let Some(scoped) = self.worker.take() else {
            return;
        };
        if let Some(path) = scoped.transient_profile_data_dir {
            let _ = fs::remove_dir_all(path);
        }
    }

    fn remove_direct_client(&mut self) {
        let Some(scoped) = self.direct_client.take() else {
            return;
        };
        if let Some(path) = scoped.transient_profile_data_dir {
            let _ = fs::remove_dir_all(path);
        }
    }
}

impl Drop for WebSurfaceRuntime {
    fn drop(&mut self) {
        self.remove_worker();
        self.remove_direct_client();
    }
}

pub(super) type LiveRuntimeClientFactory =
    fn(PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String>;

fn new_servo_live_client(config_dir: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    ServoLiveClient::new(config_dir)
        .map(|client| Box::new(client) as Box<dyn LiveRuntimeClient>)
        .map_err(|error| error.to_string())
}

struct ScopedWorker {
    worker: LiveRuntimeWorker,
    transient_profile_data_dir: Option<PathBuf>,
}

struct ScopedDirectClient {
    client: Box<dyn LiveRuntimeClient>,
    transient_profile_data_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct WebSurfaceRuntimeScope {
    profile_id: ProfileId,
    profile_data_mode: ProfileDataMode,
}

impl WebSurfaceRuntimeScope {
    pub(super) fn new(profile_id: ProfileId, profile_data_mode: ProfileDataMode) -> Self {
        Self { profile_id, profile_data_mode }
    }
}

#[derive(Clone)]
pub(super) struct WebSurfaceSession {
    pub(super) scope: WebSurfaceRuntimeScope,
    pub(super) requested_url: String,
    pub(super) size: WebSurfaceSize,
    pub(super) zoom_percent: u16,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) pending_user_navigation: bool,
    pub(super) cadence: WebSurfacePollCadence,
}

impl WebSurfaceSession {
    fn new(scope: WebSurfaceRuntimeScope) -> Self {
        Self {
            scope,
            requested_url: String::new(),
            size: WebSurfaceSize::default(),
            zoom_percent: 0,
            scroll_offset: WebSurfaceScrollOffset::default(),
            pending_user_navigation: false,
            cadence: WebSurfacePollCadence::default(),
        }
    }

    fn started_loading(
        &self,
        requested_url: &str,
        size: WebSurfaceSize,
        zoom_percent: u16,
    ) -> bool {
        self.requested_url != requested_url
            || self.size != size
            || self.zoom_percent != zoom_percent
    }

    fn url_change_for(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        frame: &WebSurfaceFrame,
    ) -> Option<WebSurfaceUrlChange> {
        let loaded_url = frame.loaded_url()?;
        if loaded_url == requested_url {
            return None;
        }

        let kind = if self.pending_user_navigation {
            WebSurfaceUrlChangeKind::UserInitiated
        } else {
            WebSurfaceUrlChangeKind::Observed
        };
        self.pending_user_navigation = false;
        Some(WebSurfaceUrlChange {
            tab_id: tab_id.clone(),
            loaded_url: loaded_url.to_string(),
            kind,
        })
    }
}

pub(super) struct WebSurfaceEnsureResult {
    pub(super) requested_url: String,
    pub(super) started_loading: bool,
}

pub(super) enum WebSurfaceRuntimeFrame {
    Ready { tab_id: TabId, frame: Box<WebSurfaceFrame>, url_change: Option<WebSurfaceUrlChange> },
    Failed { tab_id: TabId, message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceUrlChange {
    pub(super) tab_id: TabId,
    pub(super) loaded_url: String,
    pub(super) kind: WebSurfaceUrlChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WebSurfaceUrlChangeKind {
    UserInitiated,
    Observed,
}

fn config_dir_for_scope(
    scope: &WebSurfaceRuntimeScope,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    match scope.profile_data_mode {
        ProfileDataMode::Persistent => {
            let root = default_profile_data_root()
                .ok_or_else(|| "Profile data root is unavailable".to_string())?;
            let config_dir = profile_data_dir(&root, &scope.profile_id);
            fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
            Ok((config_dir, None))
        }
        ProfileDataMode::Transient => {
            let config_dir =
                transient_profile_data_dir(&scope.profile_id).map_err(|error| error.to_string())?;
            fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
            Ok((config_dir.clone(), Some(config_dir)))
        }
    }
}

pub(super) fn session_for_scope<'a>(
    sessions: &'a mut BTreeMap<TabId, WebSurfaceSession>,
    tab_id: &TabId,
    scope: WebSurfaceRuntimeScope,
) -> &'a mut WebSurfaceSession {
    let session =
        sessions.entry(tab_id.clone()).or_insert_with(|| WebSurfaceSession::new(scope.clone()));
    if session.scope != scope {
        *session = WebSurfaceSession::new(scope);
    }
    session
}

fn scroll_wire_fields(
    delta: Option<super::web_surface_geometry::WebSurfaceScrollDelta>,
    point: Option<super::web_surface_geometry::WebSurfaceClickPoint>,
) -> Result<(i32, i32, Option<u32>, Option<u32>), String> {
    match delta {
        Some(delta) => {
            let point = point
                .ok_or_else(|| "Servo scroll input is missing a viewport point".to_string())?;
            Ok((delta.x(), delta.y(), Some(point.x()), Some(point.y())))
        }
        None => Ok((0, 0, None, None)),
    }
}

fn input_requests_history_navigation(input: &WebSurfacePendingInput) -> bool {
    input.click_point.is_some()
        || input.typed_text.as_deref().is_some_and(|text| text.contains('\n'))
}

fn pending_input_kind(input: &WebSurfacePendingInput) -> WebSurfaceInputKind {
    if input.scroll_delta.is_some() {
        WebSurfaceInputKind::Scroll
    } else if input.click_point.is_some() {
        WebSurfaceInputKind::Click
    } else if input.typed_text.is_some() {
        WebSurfaceInputKind::Text
    } else if input.hover_point.is_some() {
        WebSurfaceInputKind::Hover
    } else {
        WebSurfaceInputKind::Idle
    }
}

fn log_ensure_submitted(
    tab: &BrowserTab,
    size: WebSurfaceSize,
    input_kind: &'static str,
    enqueued_at: Option<Instant>,
    started_loading: bool,
) {
    if input_kind == "idle" && !started_loading {
        return;
    }
    let queued_us = enqueued_at.map(|started_at| started_at.elapsed().as_micros());
    tracing::info!(
        target: "ely::web_surface::latency",
        tab_id = %tab.id().as_str(),
        url = %tab.url().as_str(),
        input_kind,
        queued_us,
        started_loading,
        width = size.width,
        height = size.height,
        device_pixel_ratio = size.device_pixel_ratio_f32(),
        "web_surface_ensure_submitted",
    );
}

impl From<&WebSurfaceSitePermission> for ServoLiveSitePermission {
    fn from(permission: &WebSurfaceSitePermission) -> Self {
        Self::new(
            permission.origin().as_str(),
            permission.feature().as_str(),
            permission.decision(),
        )
    }
}

#[cfg(test)]
#[path = "web_surface_runtime_tests.rs"]
mod tests;
