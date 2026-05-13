use std::{collections::BTreeMap, fs, path::PathBuf};

use ely_domain::{BrowserTab, ProfileId, TabId};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveClient, ServoLiveEnsureRequest, ServoLiveSitePermission},
    servo_profile_data::{default_profile_data_root, profile_data_dir, transient_profile_data_dir},
};

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_state::WebSurfacePendingInput,
};

pub(super) struct WebSurfaceRuntime {
    clients: BTreeMap<WebSurfaceRuntimeScope, ScopedRuntimeClient>,
    sessions: BTreeMap<TabId, WebSurfaceSession>,
    client_factory: LiveRuntimeClientFactory,
}

impl WebSurfaceRuntime {
    pub(super) fn new() -> Self {
        Self {
            clients: BTreeMap::new(),
            sessions: BTreeMap::new(),
            client_factory: new_servo_live_client,
        }
    }

    #[cfg(test)]
    fn new_with_client_factory(client_factory: LiveRuntimeClientFactory) -> Self {
        Self { clients: BTreeMap::new(), sessions: BTreeMap::new(), client_factory }
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
        self.ensure_runtime(scope.clone())?;

        let requested_url = tab.url().as_str().to_string();
        let zoom_percent = tab.zoom_percent();
        let (scroll_delta_x, scroll_delta_y, scroll_point_x, scroll_point_y) =
            scroll_wire_fields(input.scroll_delta, input.scroll_point)?;
        let user_navigation_input = input_requests_history_navigation(&input);

        let next_scroll_offset = input.scroll_offset;
        let started_loading = {
            let session = session_for_scope(&mut self.sessions, tab.id(), scope.clone());
            let started_loading = session.started_loading(&requested_url, size, zoom_percent);
            if started_loading {
                session.pending_user_navigation = false;
            }
            if user_navigation_input {
                session.pending_user_navigation = true;
            }
            started_loading
        };

        let frame = self
            .client_for_scope(&scope)?
            .ensure(ServoLiveEnsureRequest {
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
                site_permissions: permissions.iter().map(ServoLiveSitePermission::from).collect(),
            })?
            .map(|frame| {
                WebSurfaceFrame::from_live_frame(
                    requested_url.clone(),
                    next_scroll_offset,
                    zoom_percent,
                    frame,
                )
                .map_err(|error| error.to_string())
            })
            .transpose()?;
        let url_change = frame.as_ref().and_then(|frame| {
            self.sessions
                .get_mut(tab.id())
                .and_then(|session| session.url_change_for(tab.id(), requested_url.as_str(), frame))
        });

        let session = session_for_scope(&mut self.sessions, tab.id(), scope);
        session.requested_url = requested_url.clone();
        session.size = size;
        session.zoom_percent = zoom_percent;
        session.scroll_offset = next_scroll_offset;

        Ok(WebSurfaceEnsureResult { requested_url, started_loading, frame, url_change })
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> Vec<WebSurfaceRuntimeFrame> {
        let mut frames = Vec::new();
        for job in visible_poll_jobs(&self.sessions, visible_tab_ids) {
            let result = match self.clients.get_mut(&job.scope) {
                Some(client) => client.client.poll(job.tab_id.as_str().to_string()),
                None => Err(missing_runtime_message(&job.scope)),
            };
            match result {
                Ok(Some(frame)) => match WebSurfaceFrame::from_live_frame(
                    job.requested_url.clone(),
                    job.scroll_offset,
                    job.zoom_percent,
                    frame,
                ) {
                    Ok(frame) => {
                        let url_change = self.sessions.get_mut(&job.tab_id).and_then(|session| {
                            session.url_change_for(&job.tab_id, job.requested_url.as_str(), &frame)
                        });
                        frames.push(WebSurfaceRuntimeFrame::Ready {
                            tab_id: job.tab_id,
                            frame: Box::new(frame),
                            url_change,
                        })
                    }
                    Err(error) => frames.push(WebSurfaceRuntimeFrame::Failed {
                        tab_id: job.tab_id,
                        message: error.to_string(),
                    }),
                },
                Ok(None) => {}
                Err(error) => frames
                    .push(WebSurfaceRuntimeFrame::Failed { tab_id: job.tab_id, message: error }),
            }
        }

        frames
    }

    fn ensure_runtime(&mut self, scope: WebSurfaceRuntimeScope) -> Result<(), String> {
        if self.clients.contains_key(&scope) {
            return Ok(());
        }
        let (config_dir, transient_profile_data_dir) = config_dir_for_scope(&scope)?;
        let client = (self.client_factory)(config_dir)?;
        self.clients.insert(scope, ScopedRuntimeClient { client, transient_profile_data_dir });
        Ok(())
    }

    fn client_for_scope(
        &mut self,
        scope: &WebSurfaceRuntimeScope,
    ) -> Result<&mut dyn LiveRuntimeClient, String> {
        match self.clients.get_mut(scope) {
            Some(client) => Ok(client.client.as_mut()),
            None => Err(missing_runtime_message(scope)),
        }
    }

    #[cfg(test)]
    fn client_count_for_test(&self) -> usize {
        self.clients.len()
    }

    #[cfg(test)]
    fn session_scope_for_test(&self, tab_id: &TabId) -> Option<&WebSurfaceRuntimeScope> {
        self.sessions.get(tab_id).map(|session| &session.scope)
    }
}

impl Drop for WebSurfaceRuntime {
    fn drop(&mut self) {
        let transient_profile_data_dirs = self
            .clients
            .values()
            .filter_map(|client| client.transient_profile_data_dir.clone())
            .collect::<Vec<_>>();
        self.clients.clear();
        for path in transient_profile_data_dirs {
            let _ = fs::remove_dir_all(path);
        }
    }
}

type LiveRuntimeClientFactory = fn(PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String>;

trait LiveRuntimeClient {
    fn ensure(&mut self, request: ServoLiveEnsureRequest) -> Result<Option<WebLiveFrame>, String>;

    fn poll(&mut self, tab_id: String) -> Result<Option<WebLiveFrame>, String>;
}

type WebLiveFrame = crate::services::servo_live::ServoLiveFrame;

impl LiveRuntimeClient for ServoLiveClient {
    fn ensure(&mut self, request: ServoLiveEnsureRequest) -> Result<Option<WebLiveFrame>, String> {
        ServoLiveClient::ensure(self, request).map_err(|error| error.to_string())
    }

    fn poll(&mut self, tab_id: String) -> Result<Option<WebLiveFrame>, String> {
        ServoLiveClient::poll(self, tab_id).map_err(|error| error.to_string())
    }
}

fn new_servo_live_client(config_dir: PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String> {
    ServoLiveClient::new(config_dir)
        .map(|client| Box::new(client) as Box<dyn LiveRuntimeClient>)
        .map_err(|error| error.to_string())
}

struct ScopedRuntimeClient {
    client: Box<dyn LiveRuntimeClient>,
    transient_profile_data_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WebSurfaceRuntimeScope {
    profile_id: ProfileId,
    profile_data_mode: ProfileDataMode,
}

impl WebSurfaceRuntimeScope {
    fn new(profile_id: ProfileId, profile_data_mode: ProfileDataMode) -> Self {
        Self { profile_id, profile_data_mode }
    }
}

#[derive(Clone)]
struct WebSurfaceSession {
    scope: WebSurfaceRuntimeScope,
    requested_url: String,
    size: WebSurfaceSize,
    zoom_percent: u16,
    scroll_offset: WebSurfaceScrollOffset,
    pending_user_navigation: bool,
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

#[derive(Clone)]
struct WebSurfacePollJob {
    tab_id: TabId,
    scope: WebSurfaceRuntimeScope,
    requested_url: String,
    scroll_offset: WebSurfaceScrollOffset,
    zoom_percent: u16,
}

pub(super) struct WebSurfaceEnsureResult {
    pub(super) requested_url: String,
    pub(super) started_loading: bool,
    pub(super) frame: Option<WebSurfaceFrame>,
    pub(super) url_change: Option<WebSurfaceUrlChange>,
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

fn session_for_scope<'a>(
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

fn visible_poll_jobs(
    sessions: &BTreeMap<TabId, WebSurfaceSession>,
    visible_tab_ids: &[TabId],
) -> Vec<WebSurfacePollJob> {
    sessions
        .iter()
        .filter(|(tab_id, _)| {
            visible_tab_ids.iter().any(|visible_tab_id| visible_tab_id == *tab_id)
        })
        .map(|(tab_id, session)| WebSurfacePollJob {
            tab_id: tab_id.clone(),
            scope: session.scope.clone(),
            requested_url: session.requested_url.clone(),
            scroll_offset: session.scroll_offset,
            zoom_percent: session.zoom_percent,
        })
        .collect()
}

fn missing_runtime_message(scope: &WebSurfaceRuntimeScope) -> String {
    format!(
        "Servo live runtime is unavailable for profile {} ({:?})",
        scope.profile_id.as_str(),
        scope.profile_data_mode
    )
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
