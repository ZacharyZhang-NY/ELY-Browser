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
    state: RuntimeState,
    sessions: BTreeMap<TabId, WebSurfaceSession>,
}

impl WebSurfaceRuntime {
    pub(super) fn new() -> Self {
        Self { state: RuntimeState::Empty, sessions: BTreeMap::new() }
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
        let (state, sessions) = (&mut self.state, &mut self.sessions);
        let RuntimeState::Ready { scope: active_scope, client, .. } = state else {
            return Err("Servo live runtime is unavailable".to_string());
        };
        if active_scope != &scope {
            return Err(active_scope.error_for(&scope));
        }
        let (scroll_delta_x, scroll_delta_y, scroll_point_x, scroll_point_y) =
            scroll_wire_fields(input.scroll_delta, input.scroll_point)?;

        let session = sessions.entry(tab.id().clone()).or_insert_with(WebSurfaceSession::default);
        let next_scroll_offset = input.scroll_offset;
        let started_loading = session.started_loading(&requested_url, size, zoom_percent);
        let frame = client
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
            })
            .map_err(|error| error.to_string())?
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

        session.requested_url = requested_url.clone();
        session.size = size;
        session.zoom_percent = zoom_percent;
        session.scroll_offset = next_scroll_offset;

        Ok(WebSurfaceEnsureResult { requested_url, started_loading, frame })
    }

    pub(super) fn tick(&mut self) -> Vec<WebSurfaceRuntimeFrame> {
        let (state, sessions) = (&mut self.state, &self.sessions);
        let RuntimeState::Ready { client, .. } = state else {
            return Vec::new();
        };

        let mut frames = Vec::new();
        for (tab_id, session) in sessions {
            match client.poll(tab_id.as_str().to_string()) {
                Ok(Some(frame)) => match WebSurfaceFrame::from_live_frame(
                    session.requested_url.clone(),
                    session.scroll_offset,
                    session.zoom_percent,
                    frame,
                ) {
                    Ok(frame) => {
                        frames.push(WebSurfaceRuntimeFrame::Ready { tab_id: tab_id.clone(), frame })
                    }
                    Err(error) => frames.push(WebSurfaceRuntimeFrame::Failed {
                        tab_id: tab_id.clone(),
                        message: error.to_string(),
                    }),
                },
                Ok(None) => {}
                Err(error) => frames.push(WebSurfaceRuntimeFrame::Failed {
                    tab_id: tab_id.clone(),
                    message: error.to_string(),
                }),
            }
        }

        frames
    }

    fn ensure_runtime(&mut self, scope: WebSurfaceRuntimeScope) -> Result<(), String> {
        match &self.state {
            RuntimeState::Empty => {
                let (config_dir, transient_profile_data_dir) = config_dir_for_scope(&scope)?;
                let client = ServoLiveClient::new(config_dir).map_err(|error| error.to_string())?;
                self.state = RuntimeState::Ready { scope, client, transient_profile_data_dir };
                Ok(())
            }
            RuntimeState::Ready { scope: active_scope, .. } if active_scope == &scope => Ok(()),
            RuntimeState::Ready { scope: active_scope, .. } => Err(active_scope.error_for(&scope)),
        }
    }
}

impl Drop for WebSurfaceRuntime {
    fn drop(&mut self) {
        let RuntimeState::Ready { transient_profile_data_dir: Some(path), .. } = &self.state else {
            return;
        };
        let _ = fs::remove_dir_all(path);
    }
}

enum RuntimeState {
    Empty,
    Ready {
        scope: WebSurfaceRuntimeScope,
        client: ServoLiveClient,
        transient_profile_data_dir: Option<PathBuf>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebSurfaceRuntimeScope {
    profile_id: ProfileId,
    profile_data_mode: ProfileDataMode,
}

impl WebSurfaceRuntimeScope {
    fn new(profile_id: ProfileId, profile_data_mode: ProfileDataMode) -> Self {
        Self { profile_id, profile_data_mode }
    }

    fn error_for(&self, requested: &Self) -> String {
        format!(
            "Servo live runtime is already attached to profile {} ({:?}); requested profile {} ({:?})",
            self.profile_id.as_str(),
            self.profile_data_mode,
            requested.profile_id.as_str(),
            requested.profile_data_mode
        )
    }
}

#[derive(Clone, Default)]
struct WebSurfaceSession {
    requested_url: String,
    size: WebSurfaceSize,
    zoom_percent: u16,
    scroll_offset: WebSurfaceScrollOffset,
}

impl WebSurfaceSession {
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
}

pub(super) struct WebSurfaceEnsureResult {
    pub(super) requested_url: String,
    pub(super) started_loading: bool,
    pub(super) frame: Option<WebSurfaceFrame>,
}

pub(super) enum WebSurfaceRuntimeFrame {
    Ready { tab_id: TabId, frame: WebSurfaceFrame },
    Failed { tab_id: TabId, message: String },
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

impl From<&WebSurfaceSitePermission> for ServoLiveSitePermission {
    fn from(permission: &WebSurfaceSitePermission) -> Self {
        Self::new(
            permission.origin().as_str(),
            permission.feature().as_str(),
            permission.decision(),
        )
    }
}
