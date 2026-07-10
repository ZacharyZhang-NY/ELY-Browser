use std::{path::Path, time::Duration};

use ely_domain::{ProfileId, ProfileKind, SyncConnectionState};
use ely_sync_client::{AuthenticatedSnapshotHead, BearerToken, BearerTokenStore, DeviceRecord};
use gpui::{Context, Timer};

use super::{
    ElyShell, ShellState, auth,
    sync_inbox::{early_reconciliation_messages, early_reconciliation_priority},
};

mod failure;
mod legacy_sync_migration;
mod session_expiry;
mod sign_out;

pub(super) use failure::{device_failure_update, sync_failure_update};

const CLOUD_SYNC_UPLOAD_DEBOUNCE: Duration = Duration::from_millis(750);
const CAS_RETRY_LIMIT: u8 = 3;
const CAS_RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub(crate) struct PendingMergeUpload {
    pub(super) profile_id: ProfileId,
    pub(super) base: AuthenticatedSnapshotHead,
    pub(super) conflict_count: u8,
}

/// Messages the off-thread sync workers push back to the shell so
/// `SyncConnectionState` on `BrowserCore` and the in-flight auth
/// form reflect live state without the UI thread ever touching the
/// network. Startup seeds `SignedIn` outside this channel.
#[derive(Clone, Debug)]
pub(crate) enum SyncStateUpdate {
    AuthenticationExpired { profile_id: ProfileId },
    AwaitingDeviceApproval { profile_id: ProfileId, finishes_upload: bool },
    RemoteSnapshot { profile_id: ProfileId, bytes: Vec<u8>, merge: PendingMergeUpload },
    SyncReady { profile_id: ProfileId, last_synced_at_secs: u64 },
    SyncBusy { profile_id: ProfileId },
    SyncError { profile_id: ProfileId, message: String },
    CredentialUnavailable { profile_id: ProfileId, message: String },
    DevicesLoaded { profile_id: ProfileId, devices: Vec<DeviceRecord>, current_code: String },
    DevicesError { profile_id: ProfileId, message: String },
    SignOutSucceeded { profile_id: ProfileId },
    SignOutFailed { profile_id: ProfileId, message: String },
    AuthOtpSent { profile_id: ProfileId, email: String },
    AuthVerified { profile_id: ProfileId, email: String, user_id: String, token: BearerToken },
    AuthError { profile_id: ProfileId, email: String, message: String },
}

/// Stable label for the current OS used by the device registration
/// payload. Defined once here so every off-thread call site agrees.
pub(crate) const fn sync_platform_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

impl ElyShell {
    pub(crate) fn schedule_cloud_sync_upload(&mut self, cx: &mut Context<Self>) {
        self.schedule_local_state_save(cx);
        self.reconcile_active_sync_profile();
        if !self.can_schedule_cloud_sync_upload() {
            return;
        }
        if self.sync_upload_in_flight {
            self.queue_cloud_sync_upload(None);
            return;
        }
        if self.sync_upload_scheduled {
            return;
        }

        self.sync_upload_scheduled = true;
        cx.spawn(async move |shell, cx| {
            Timer::after(CLOUD_SYNC_UPLOAD_DEBOUNCE).await;
            let _ = shell.update(cx, |shell, _| {
                if shell.sync_upload_scheduled {
                    shell.sync_upload_scheduled = false;
                    shell.trigger_cloud_sync_upload();
                }
            });
        })
        .detach();
    }

    pub(super) fn queue_cloud_sync_upload(&mut self, merge: Option<PendingMergeUpload>) {
        self.sync_upload_pending = true;
        if let Some(candidate) = merge {
            let replace = self.sync_upload_pending_merge.as_ref().is_none_or(|current| {
                candidate.profile_id != current.profile_id
                    || candidate.base.revision() > current.base.revision()
                    || candidate.base.revision() == current.base.revision()
                        && candidate.conflict_count > current.conflict_count
            });
            if replace {
                self.sync_upload_pending_merge = Some(candidate);
            }
        }
    }

    fn trigger_pending_cloud_sync_upload(&mut self) -> bool {
        if !self.sync_upload_pending {
            return false;
        }

        self.sync_upload_pending = false;
        let merge = self.sync_upload_pending_merge.take();
        match merge {
            Some(merge) => self.trigger_cloud_sync_upload_after_remote(merge),
            None => self.trigger_cloud_sync_upload(),
        }
        true
    }

    pub(super) fn clear_pending_cloud_sync_upload(&mut self) {
        self.sync_upload_pending = false;
        self.sync_upload_pending_merge = None;
    }

    fn can_schedule_cloud_sync_upload(&self) -> bool {
        let ShellState::Ready(core) = &self.state else {
            return false;
        };
        core.cloud_sync_upload_enabled()
    }

    /// Inspect the on-disk bearer token and seed `SyncConnectionState`
    /// so the Sync settings page reads the startup state on first render.
    pub(super) fn probe_initial_sync_state(&mut self) -> bool {
        let Some(profile_root) = crate::services::servo_profile_data::default_profile_data_root()
        else {
            return false;
        };
        let sign_out_phases = &self.sign_out_phases;
        let (bearer_present, sign_out_active) = match &mut self.state {
            ShellState::Ready(core) => {
                let bearer_present = probe_initial_sync_state_at(
                    core,
                    &profile_root,
                    self.default_profile_id.as_ref(),
                );
                let sign_out_active =
                    sign_out::overlay_active_sign_out_connection(core, sign_out_phases);
                (bearer_present, sign_out_active)
            }
            ShellState::StartupError(_) => return false,
        };
        if sign_out_active {
            return false;
        }
        bearer_present
    }

    /// Drain any sync upload outcomes the off-thread worker pushed
    /// since the previous tick and stamp the resulting connection
    /// state on `BrowserCore`. Returns `true` when at least one
    /// update was applied so callers can `cx.notify()` accordingly.
    pub(super) fn drain_sync_updates(&mut self) -> bool {
        let profile_needs_initial_sync = self.reconcile_active_sync_profile();
        let retry_due =
            self.sync_retry_at.is_some_and(|deadline| deadline <= std::time::Instant::now());
        if retry_due {
            self.sync_retry_at = None;
        }
        let mut latest_connection: Option<SyncConnectionState> = None;
        let mut auth_changed = false;
        let mut trigger_initial_sync = profile_needs_initial_sync;
        let mut trigger_merged_upload = None;
        let mut upload_finished = false;
        let mut devices_changed = false;
        let messages = self.sync_inbox_rx.try_iter().collect::<Vec<_>>();
        for state_message in early_reconciliation_messages(&messages) {
            match &state_message.update {
                SyncStateUpdate::AuthenticationExpired { profile_id } => {
                    let current = state_message.belongs_to(self.sync_generation);
                    match self.reconcile_authentication_expiry(profile_id, current) {
                        session_expiry::AuthenticationExpiryResolution::Ignored => {}
                        session_expiry::AuthenticationExpiryResolution::ReplacementCredential => {
                            latest_connection = Some(SyncConnectionState::SignedIn);
                            trigger_initial_sync = true;
                            devices_changed = true;
                            auth_changed = true;
                        }
                        session_expiry::AuthenticationExpiryResolution::Connection(connection) => {
                            latest_connection = Some(connection);
                            trigger_initial_sync = false;
                            trigger_merged_upload = None;
                            devices_changed = true;
                            auth_changed = true;
                        }
                    }
                }
                SyncStateUpdate::CredentialUnavailable { profile_id, message }
                    if state_message.belongs_to(self.sync_generation)
                        && active_profile_id(&self.state).as_ref() == Some(profile_id) =>
                {
                    self.reconcile_credential_unavailable(profile_id);
                    latest_connection = Some(SyncConnectionState::CredentialUnavailable {
                        message: message.clone(),
                    });
                    trigger_initial_sync = false;
                    trigger_merged_upload = None;
                    devices_changed = true;
                    auth_changed = true;
                }
                _ => {}
            }
        }
        for message in messages {
            if early_reconciliation_priority(&message.update).is_some() {
                continue;
            }
            if let Some((profile_id, connection)) =
                self.reconcile_sign_out_update(message.generation, &message.update)
            {
                if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                    self.auth_flow_phase = auth::AuthFlowPhase::Idle;
                    latest_connection = Some(connection);
                } else {
                    trigger_initial_sync |= self.can_schedule_cloud_sync_upload();
                }
                auth_changed = true;
                continue;
            }
            if !message.belongs_to(self.sync_generation) {
                auth::retire_stale_auth_update(message.update);
                continue;
            }
            let update = message.update;
            match update {
                SyncStateUpdate::AuthenticationExpired { .. }
                | SyncStateUpdate::CredentialUnavailable { .. } => {}
                SyncStateUpdate::AwaitingDeviceApproval { profile_id, finishes_upload } => {
                    upload_finished |= finishes_upload;
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        latest_connection = Some(SyncConnectionState::AwaitingDeviceApproval);
                    }
                }
                SyncStateUpdate::RemoteSnapshot { profile_id, bytes, merge } => {
                    upload_finished = true;
                    if active_profile_id(&self.state).as_ref() != Some(&profile_id) {
                        continue;
                    }
                    if let ShellState::Ready(core) = &mut self.state {
                        match core.apply_sync_snapshot_bytes(&bytes) {
                            Ok(summary) => {
                                tracing::info!(
                                    target: "ely::sync",
                                    imported = summary.imported(),
                                    updated = summary.updated(),
                                    skipped = summary.skipped(),
                                    "remote snapshot applied",
                                );
                                if merge.conflict_count >= CAS_RETRY_LIMIT {
                                    latest_connection = Some(SyncConnectionState::SyncError {
                                        message: "Cloud Sync is busy; retrying shortly".to_string(),
                                    });
                                    self.sync_retry_at =
                                        Some(std::time::Instant::now() + CAS_RETRY_DELAY);
                                } else {
                                    trigger_merged_upload = Some(merge);
                                }
                            }
                            Err(error) => {
                                latest_connection = Some(SyncConnectionState::SyncError {
                                    message: error.to_string(),
                                });
                            }
                        }
                    }
                }
                SyncStateUpdate::SyncReady { profile_id, last_synced_at_secs } => {
                    upload_finished = true;
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        latest_connection =
                            Some(SyncConnectionState::SyncReady { last_synced_at_secs });
                    }
                }
                SyncStateUpdate::SyncBusy { profile_id } => {
                    upload_finished = true;
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        latest_connection = Some(SyncConnectionState::SyncError {
                            message: "Cloud Sync is busy; retrying shortly".to_string(),
                        });
                        self.sync_retry_at = Some(std::time::Instant::now() + CAS_RETRY_DELAY);
                    }
                }
                SyncStateUpdate::SyncError { profile_id, message } => {
                    upload_finished = true;
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        latest_connection = Some(SyncConnectionState::SyncError { message });
                    }
                }
                SyncStateUpdate::DevicesLoaded { profile_id, devices, current_code } => {
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        self.sync_devices.set_ready(profile_id, devices, current_code);
                        devices_changed = true;
                    }
                }
                SyncStateUpdate::DevicesError { profile_id, message } => {
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        self.sync_devices.set_error(profile_id, message);
                        devices_changed = true;
                    }
                }
                SyncStateUpdate::SignOutSucceeded { .. }
                | SyncStateUpdate::SignOutFailed { .. } => {}
                SyncStateUpdate::AuthOtpSent { profile_id, email } => {
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        self.auth_flow_phase =
                            auth::AuthFlowPhase::AwaitingOtp { profile_id, email };
                        auth_changed = true;
                    } else {
                        self.release_auth_flow_barrier();
                    }
                }
                SyncStateUpdate::AuthVerified { profile_id, email, user_id, token } => {
                    let attempt_matches = active_profile_id(&self.state).as_ref()
                        == Some(&profile_id)
                        && matches!(
                            &self.auth_flow_phase,
                            auth::AuthFlowPhase::Verifying {
                                profile_id: owner,
                                email: expected_email,
                            } if owner == &profile_id && expected_email == &email
                        );
                    self.release_auth_flow_barrier();
                    if !attempt_matches {
                        auth::retire_stale_auth_update(SyncStateUpdate::AuthVerified {
                            profile_id,
                            email,
                            user_id,
                            token,
                        });
                        continue;
                    }
                    match auth::save_verified_bearer(
                        &profile_id,
                        &user_id,
                        &token,
                        self.default_profile_id.as_ref(),
                    ) {
                        Ok(()) => {
                            self.sign_out_phases.remove(&profile_id);
                            self.auth_flow_phase = auth::AuthFlowPhase::Idle;
                            latest_connection = Some(SyncConnectionState::SignedIn);
                            trigger_initial_sync = true;
                            auth_changed = true;
                        }
                        Err(error) => {
                            tracing::warn!(target: "ely::sync", error = %error, "bearer credential save failed");
                            auth::retire_stale_auth_update(SyncStateUpdate::AuthVerified {
                                profile_id: profile_id.clone(),
                                email: email.clone(),
                                user_id,
                                token,
                            });
                            let message = auth::verified_session_persistence_message(&error);
                            self.auth_flow_phase = auth::AuthFlowPhase::Error {
                                profile_id,
                                email,
                                message: message.clone(),
                            };
                            latest_connection =
                                Some(SyncConnectionState::CredentialUnavailable { message });
                            auth_changed = true;
                        }
                    }
                }
                SyncStateUpdate::AuthError { profile_id, email, message } => {
                    self.release_auth_flow_barrier();
                    if active_profile_id(&self.state).as_ref() == Some(&profile_id) {
                        self.auth_flow_phase =
                            auth::AuthFlowPhase::Error { profile_id, email, message };
                        auth_changed = true;
                    }
                }
            }
        }

        let connection_changed = latest_connection.is_some();
        if let (Some(state), ShellState::Ready(core)) = (latest_connection, &mut self.state) {
            core.set_sync_connection_state(state);
        }
        if upload_finished {
            self.sync_upload_in_flight = false;
        }
        if trigger_initial_sync {
            self.trigger_cloud_sync_upload();
        }

        let merged_upload_requested = trigger_merged_upload.is_some();
        if let Some(merge) = trigger_merged_upload {
            self.clear_pending_cloud_sync_upload();
            self.trigger_cloud_sync_upload_after_remote(merge);
        } else if upload_finished {
            self.trigger_pending_cloud_sync_upload();
        }
        if retry_due {
            self.trigger_cloud_sync_upload();
        }

        auth_changed
            || devices_changed
            || trigger_initial_sync
            || merged_upload_requested
            || retry_due
            || connection_changed
    }

    fn reconcile_active_sync_profile(&mut self) -> bool {
        let current_profile_id = active_profile_id(&self.state);
        if current_profile_id == self.sync_profile_id {
            return false;
        }
        self.invalidate_sync_work();
        self.sync_profile_id = current_profile_id;
        self.auth_flow_phase = auth::AuthFlowPhase::Idle;
        self.probe_initial_sync_state()
    }
}

fn active_profile_id(state: &ShellState) -> Option<ProfileId> {
    let ShellState::Ready(core) = state else {
        return None;
    };
    core.snapshot().ok().map(|snapshot| snapshot.active_profile_id)
}

fn probe_initial_sync_state_at(
    core: &mut ely_browser_core::BrowserCore,
    profile_root: &Path,
    default_profile_id: Option<&ProfileId>,
) -> bool {
    let Some(snapshot) = core.snapshot().ok() else {
        return false;
    };
    let profile_dir = crate::services::servo_profile_data::sync_profile_data_dir(
        profile_root,
        &snapshot.active_profile_id,
    );
    if !core.active_profile_allows_sync() {
        let store = BearerTokenStore::new(&snapshot.active_profile_id, &profile_dir);
        if let Err(error) = store.clear_legacy_files() {
            tracing::warn!(target: "ely::sync", error = %error, "private bearer cleanup failed");
        }
        core.set_sync_connection_state(SyncConnectionState::SignedOut);
        return false;
    }
    let is_default_profile = matches!(snapshot.active_profile_kind, ProfileKind::Standard)
        && default_profile_id == Some(&snapshot.active_profile_id);
    if is_default_profile
        && let Err(error) =
            legacy_sync_migration::migrate_default_sync_device(profile_root, &profile_dir)
    {
        tracing::warn!(
            target: "ely::sync",
            error = %error,
            "legacy sync profile migration failed",
        );
    }
    let store = auth::bearer_store_for_profile(
        &snapshot.active_profile_id,
        &profile_dir,
        profile_root,
        default_profile_id,
    );
    let (bearer_present, state) = match store.load() {
        Ok(Some(_)) => (true, SyncConnectionState::SignedIn),
        Ok(None) => (false, SyncConnectionState::SignedOut),
        Err(error) => {
            tracing::warn!(target: "ely::sync", error = %error, "bearer credential probe failed");
            (
                false,
                SyncConnectionState::CredentialUnavailable {
                    message: "System credential access failed.".to_string(),
                },
            )
        }
    };
    core.set_sync_connection_state(state);
    bearer_present
}

#[cfg(test)]
#[path = "sync_state/sync_state_tests.rs"]
mod tests;
