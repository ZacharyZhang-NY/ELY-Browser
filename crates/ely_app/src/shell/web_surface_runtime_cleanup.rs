use std::path::PathBuf;

use ely_domain::{ProfileId, TabId};

use crate::services::{
    ProfileDataMode,
    servo_live::{ServoLiveClient, ServoLivePermissionGrant},
    servo_profile_data::TransientProfileDataDir,
};

use super::{
    LiveRuntimeClient, LiveRuntimeWorker, WebSurfaceRuntime, WebSurfaceRuntimeFrame,
    WebSurfaceRuntimeScope,
};

pub(super) type LiveRuntimeClientFactory =
    fn(PathBuf) -> Result<Box<dyn LiveRuntimeClient>, String>;

pub(super) fn new_servo_live_client(
    config_dir: PathBuf,
) -> Result<Box<dyn LiveRuntimeClient>, String> {
    ServoLiveClient::new(config_dir)
        .map(|client| Box::new(client) as Box<dyn LiveRuntimeClient>)
        .map_err(|error| error.to_string())
}

pub(super) struct ScopedWorker {
    pub(super) worker: LiveRuntimeWorker,
    pub(super) transient_profile_data_dir: Option<TransientProfileDataDir>,
    allow_once_grants: Vec<ServoLivePermissionGrant>,
}

impl ScopedWorker {
    pub(super) fn new(
        worker: LiveRuntimeWorker,
        transient_profile_data_dir: Option<TransientProfileDataDir>,
    ) -> Self {
        Self { worker, transient_profile_data_dir, allow_once_grants: Vec::new() }
    }

    pub(super) fn track_allow_once_grants(&mut self, grants: &[ServoLivePermissionGrant]) {
        for grant in grants {
            if !self.allow_once_grants.contains(grant) {
                self.allow_once_grants.push(grant.clone());
            }
        }
    }

    pub(super) fn mark_permission_consumed(&mut self, consumed: &ServoLivePermissionGrant) {
        self.allow_once_grants.retain(|grant| grant != consumed);
    }

    fn take_allow_once_grants(&mut self) -> Vec<ServoLivePermissionGrant> {
        std::mem::take(&mut self.allow_once_grants)
    }
}

pub(super) fn shutdown_scoped_worker(scoped: ScopedWorker) -> Result<(), String> {
    let ScopedWorker { worker, transient_profile_data_dir, .. } = scoped;
    drop(worker);
    let Some(directory) = transient_profile_data_dir else {
        return Ok(());
    };
    directory
        .close()
        .map_err(|error| format!("failed to remove transient Servo profile data: {error}"))
}

impl WebSurfaceRuntime {
    pub(in crate::shell) fn reconcile_tab_scope(
        &mut self,
        tab_id: &TabId,
        profile_id: &ProfileId,
        profile_data_mode: ProfileDataMode,
    ) -> Vec<ServoLivePermissionGrant> {
        let scope = WebSurfaceRuntimeScope::new(profile_id.clone(), profile_data_mode);
        self.detach_tab_from_previous_scope(tab_id, &scope);
        self.take_retired_permission_consumptions()
    }

    pub(in crate::shell) fn reload_tab(&mut self, tab_id: &TabId) {
        let Some(session) = self.sessions.get(tab_id) else {
            return;
        };
        if let Some(scoped) = self.workers.get(&session.scope) {
            scoped.worker.submit_reload(tab_id.as_str().to_string());
        }
    }

    pub(in crate::shell) fn close_tab(&mut self, tab_id: &TabId) {
        let Some(session) = self.sessions.remove(tab_id) else {
            return;
        };
        let has_remaining_session =
            self.sessions.values().any(|candidate| candidate.scope == session.scope);
        if session.scope.is_transient() && !has_remaining_session {
            self.remove_worker(&session.scope);
        } else if let Some(scoped) = self.workers.get(&session.scope) {
            scoped.worker.submit_close(tab_id.as_str().to_string());
        }
    }

    pub(super) fn retire_scoped_worker_permissions(&mut self, scoped: &mut ScopedWorker) {
        for grant in scoped.take_allow_once_grants() {
            if !self.retired_permission_consumptions.contains(&grant) {
                self.retired_permission_consumptions.push(grant);
            }
        }
    }

    pub(in crate::shell) fn take_retired_permission_consumptions(
        &mut self,
    ) -> Vec<ServoLivePermissionGrant> {
        std::mem::take(&mut self.retired_permission_consumptions)
    }

    pub(super) fn take_retired_permission_frames(&mut self) -> Vec<WebSurfaceRuntimeFrame> {
        self.take_retired_permission_consumptions()
            .into_iter()
            .map(WebSurfaceRuntimeFrame::PermissionConsumed)
            .collect()
    }

    pub(super) fn detach_tab_from_previous_scope(
        &mut self,
        tab_id: &TabId,
        scope: &WebSurfaceRuntimeScope,
    ) {
        let Some(previous_scope) = self.sessions.get(tab_id).map(|session| session.scope.clone())
        else {
            return;
        };
        if &previous_scope == scope {
            return;
        }
        self.sessions.remove(tab_id);
        let has_remaining_session =
            self.sessions.values().any(|candidate| candidate.scope == previous_scope);
        if previous_scope.is_transient() && !has_remaining_session {
            self.remove_worker(&previous_scope);
        } else if let Some(scoped) = self.workers.get(&previous_scope) {
            scoped.worker.submit_close(tab_id.as_str().to_string());
        }
    }
}
