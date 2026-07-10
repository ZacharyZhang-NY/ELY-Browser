use std::{collections::BTreeMap, path::PathBuf};

use crate::services::{
    ProfileDataMode,
    servo_profile_data::{
        TransientProfileDataDir, create_profile_data_dir, default_profile_data_root,
        transient_profile_data_dir,
    },
};
use ely_domain::{ProfileId, TabId};

use super::{
    web_surface_cadence::WebSurfacePollCadence,
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceScrollOffset, WebSurfaceSize},
    web_surface_worker::RequestGeneration,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct WebSurfaceRuntimeScope {
    profile_id: ProfileId,
    profile_data_mode: ProfileDataMode,
}

impl WebSurfaceRuntimeScope {
    pub(super) fn new(profile_id: ProfileId, profile_data_mode: ProfileDataMode) -> Self {
        Self { profile_id, profile_data_mode }
    }

    pub(super) fn is_transient(&self) -> bool {
        self.profile_data_mode == ProfileDataMode::Transient
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
    pub(super) generation: Option<RequestGeneration>,
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
            generation: None,
            cadence: WebSurfacePollCadence::default(),
        }
    }

    pub(super) fn started_loading(
        &self,
        requested_url: &str,
        size: WebSurfaceSize,
        zoom_percent: u16,
    ) -> bool {
        self.requested_url != requested_url
            || self.size != size
            || self.zoom_percent != zoom_percent
    }

    pub(super) fn url_change_for(
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

pub(super) fn config_dir_for_scope(
    scope: &WebSurfaceRuntimeScope,
) -> Result<(PathBuf, Option<TransientProfileDataDir>), String> {
    match scope.profile_data_mode {
        ProfileDataMode::Persistent => {
            let root = default_profile_data_root()
                .ok_or_else(|| "Profile data root is unavailable".to_string())?;
            let config_dir = create_profile_data_dir(&root, &scope.profile_id)
                .map_err(|error| error.to_string())?;
            Ok((config_dir, None))
        }
        ProfileDataMode::Transient => {
            let transient_dir =
                transient_profile_data_dir(&scope.profile_id).map_err(|error| error.to_string())?;
            let config_dir = transient_dir.path().to_path_buf();
            Ok((config_dir, Some(transient_dir)))
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
