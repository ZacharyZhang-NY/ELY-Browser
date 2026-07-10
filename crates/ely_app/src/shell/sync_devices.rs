use std::path::PathBuf;

use ely_browser_core::SyncEngine;
use ely_domain::ProfileId;
use ely_sync_client::DeviceRecord;
use gpui::Context;

use crate::services::servo_profile_data::{default_profile_data_root, sync_profile_data_dir};

use super::sync_state::{device_failure_update, sync_platform_label};
use super::{ElyShell, ShellState, sync_state::SyncStateUpdate};

#[derive(Clone, Debug, Default)]
enum DeviceUiPhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Acting {
        device_id: String,
    },
    Error,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SyncDeviceUiState {
    profile_id: Option<ProfileId>,
    phase: DeviceUiPhase,
    devices: Vec<DeviceRecord>,
    current_code: Option<String>,
    revoke_confirmation: Option<String>,
    error: Option<String>,
}

impl SyncDeviceUiState {
    pub(crate) fn devices(&self) -> &[DeviceRecord] {
        &self.devices
    }

    pub(crate) fn current_code(&self) -> Option<&str> {
        self.current_code.as_deref()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn is_loading(&self) -> bool {
        matches!(self.phase, DeviceUiPhase::Loading)
    }

    pub(crate) fn is_acting_on(&self, device_id: &str) -> bool {
        matches!(&self.phase, DeviceUiPhase::Acting { device_id: active } if active == device_id)
    }

    pub(crate) fn is_revoke_confirmation(&self, device_id: &str) -> bool {
        self.revoke_confirmation.as_deref() == Some(device_id)
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn set_ready(
        &mut self,
        profile_id: ProfileId,
        devices: Vec<DeviceRecord>,
        current_code: String,
    ) {
        self.profile_id = Some(profile_id);
        self.phase = DeviceUiPhase::Ready;
        self.devices = devices;
        self.current_code = Some(current_code);
        self.revoke_confirmation = None;
        self.error = None;
    }

    pub(crate) fn set_error(&mut self, profile_id: ProfileId, message: String) {
        self.profile_id = Some(profile_id);
        self.phase = DeviceUiPhase::Error;
        self.revoke_confirmation = None;
        self.error = Some(message);
    }

    fn prepare_profile(&mut self, profile_id: &ProfileId) {
        if self.profile_id.as_ref() != Some(profile_id) {
            self.reset();
            self.profile_id = Some(profile_id.clone());
        }
    }

    fn begin_load(&mut self, force: bool) -> bool {
        if !force && !matches!(self.phase, DeviceUiPhase::Idle) {
            return false;
        }
        if matches!(self.phase, DeviceUiPhase::Loading | DeviceUiPhase::Acting { .. }) {
            return false;
        }
        self.phase = DeviceUiPhase::Loading;
        self.error = None;
        true
    }

    fn begin_action(&mut self, device_id: String) -> bool {
        if !matches!(self.phase, DeviceUiPhase::Ready | DeviceUiPhase::Error) {
            return false;
        }
        self.phase = DeviceUiPhase::Acting { device_id };
        self.revoke_confirmation = None;
        self.error = None;
        true
    }

    fn request_revoke_confirmation(&mut self, device_id: &str) -> bool {
        if self.revoke_confirmation.as_deref() == Some(device_id) {
            return true;
        }
        self.revoke_confirmation = Some(device_id.to_string());
        false
    }
}

impl ElyShell {
    pub(crate) fn ensure_sync_devices_loaded(&mut self, cx: &mut Context<Self>) {
        self.load_sync_devices(false, cx);
    }

    pub(crate) fn refresh_sync_devices(&mut self, cx: &mut Context<Self>) {
        self.load_sync_devices(true, cx);
    }

    pub(crate) fn approve_sync_device(&mut self, device_id: String, cx: &mut Context<Self>) {
        let verification_code = self.sync_verification_input.read(cx).value().to_string();
        let Some((profile_id, profile_dir, device_name)) = self.sync_device_context() else {
            self.sync_devices.reset();
            return;
        };
        self.sync_devices.prepare_profile(&profile_id);
        if !self.sync_devices.begin_action(device_id.clone()) {
            return;
        }
        let tx = self.sync_inbox_tx.clone();
        spawn_device_task("ely-sync-device-approve", profile_id.clone(), tx, move || {
            let engine = SyncEngine::for_profile_dir(
                &profile_id,
                &profile_dir,
                device_name,
                sync_platform_label(),
            )?;
            engine.approve_cloud_device(&device_id, &verification_code)?;
            load_devices(profile_id, engine)
        });
    }

    pub(crate) fn revoke_sync_device(&mut self, device_id: String, cx: &mut Context<Self>) {
        let Some((profile_id, profile_dir, device_name)) = self.sync_device_context() else {
            self.sync_devices.reset();
            return;
        };
        self.sync_devices.prepare_profile(&profile_id);
        if !self.sync_devices.request_revoke_confirmation(&device_id) {
            cx.notify();
            return;
        }
        if !self.sync_devices.begin_action(device_id.clone()) {
            return;
        }
        let tx = self.sync_inbox_tx.clone();
        spawn_device_task("ely-sync-device-revoke", profile_id.clone(), tx, move || {
            let engine = SyncEngine::for_profile_dir(
                &profile_id,
                &profile_dir,
                device_name,
                sync_platform_label(),
            )?;
            engine.revoke_cloud_device(&device_id)?;
            load_devices(profile_id, engine)
        });
    }

    fn load_sync_devices(&mut self, force: bool, _cx: &mut Context<Self>) {
        let Some((profile_id, profile_dir, device_name)) = self.sync_device_context() else {
            self.sync_devices.reset();
            return;
        };
        self.sync_devices.prepare_profile(&profile_id);
        if !self.sync_devices.begin_load(force) {
            return;
        }
        let tx = self.sync_inbox_tx.clone();
        spawn_device_task("ely-sync-device-list", profile_id.clone(), tx, move || {
            let engine = SyncEngine::for_profile_dir(
                &profile_id,
                &profile_dir,
                device_name,
                sync_platform_label(),
            )?;
            load_devices(profile_id, engine)
        });
    }

    fn sync_device_context(&self) -> Option<(ProfileId, PathBuf, String)> {
        let ShellState::Ready(core) = &self.state else {
            return None;
        };
        if !core.cloud_sync_upload_enabled() {
            return None;
        }
        let snapshot = core.snapshot().ok()?;
        let root = default_profile_data_root()?;
        Some((
            snapshot.active_profile_id.clone(),
            sync_profile_data_dir(&root, &snapshot.active_profile_id),
            format!("ELY · {}", snapshot.active_profile_name),
        ))
    }
}

fn load_devices(
    profile_id: ProfileId,
    engine: SyncEngine,
) -> Result<SyncStateUpdate, ely_sync_client::SyncClientError> {
    let current_code = engine.identity().verification_code()?;
    let devices = engine.cloud_devices()?.devices;
    Ok(SyncStateUpdate::DevicesLoaded { profile_id, devices, current_code })
}

fn spawn_device_task<F>(
    name: &str,
    profile_id: ProfileId,
    tx: std::sync::mpsc::Sender<SyncStateUpdate>,
    task: F,
) where
    F: FnOnce() -> Result<SyncStateUpdate, ely_sync_client::SyncClientError> + Send + 'static,
{
    let thread_name = name.to_string();
    let worker_tx = tx.clone();
    let worker_profile_id = profile_id.clone();
    let spawn_result = std::thread::Builder::new().name(thread_name).spawn(move || {
        let update = task().unwrap_or_else(|error| device_failure_update(worker_profile_id, error));
        let _ = worker_tx.send(update);
    });
    if let Err(error) = spawn_result {
        tracing::warn!(target: "ely::sync", error = %error, "device task spawn failed");
        let _ = tx.send(SyncStateUpdate::DevicesError { profile_id, message: error.to_string() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_profiles_clears_device_ui_state() {
        let first_profile = ProfileId::new();
        let second_profile = ProfileId::new();
        let mut state = SyncDeviceUiState::default();

        state.prepare_profile(&first_profile);
        state.set_error(first_profile.clone(), "first profile error".to_string());
        state.prepare_profile(&first_profile);
        assert_eq!(state.error(), Some("first profile error"));

        state.prepare_profile(&second_profile);
        assert!(state.devices().is_empty());
        assert!(state.current_code().is_none());
        assert!(state.error().is_none());
    }

    #[test]
    fn credential_failures_do_not_finish_an_upload() {
        let profile_id = ProfileId::new();
        let update = device_failure_update(
            profile_id.clone(),
            ely_sync_client::SyncClientError::BearerCredentialStorage("locked".to_string()),
        );

        assert!(matches!(
            update,
            SyncStateUpdate::CredentialUnavailable {
                profile_id: owner,
                finishes_upload: false,
                ..
            } if owner == profile_id
        ));
    }

    #[test]
    fn approval_failures_do_not_finish_an_upload() {
        let profile_id = ProfileId::new();
        let errors = [
            ely_sync_client::SyncClientError::DeviceApprovalStatus {
                device_id: "device-id".to_string(),
                status: "pending".to_string(),
            },
            ely_sync_client::SyncClientError::HttpStatus {
                endpoint: "/api/sync/devices".to_string(),
                status: 403,
                body: r#"{"error":"device_not_approved"}"#.to_string(),
            },
        ];

        for error in errors {
            let update = device_failure_update(profile_id.clone(), error);
            assert!(matches!(
                update,
                SyncStateUpdate::AwaitingDeviceApproval {
                    profile_id: owner,
                    finishes_upload: false,
                } if owner == profile_id
            ));
        }
    }

    #[test]
    fn operational_failures_stay_in_device_ui_state() {
        let profile_id = ProfileId::new();
        let update = device_failure_update(
            profile_id.clone(),
            ely_sync_client::SyncClientError::HttpStatus {
                endpoint: "/api/sync/devices".to_string(),
                status: 503,
                body: "unavailable".to_string(),
            },
        );

        assert!(matches!(
            update,
            SyncStateUpdate::DevicesError { profile_id: owner, .. } if owner == profile_id
        ));
    }
}
