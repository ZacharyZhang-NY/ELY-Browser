use std::{path::Path, time::Duration};

use ely_domain::SyncConnectionState;
use gpui::{Context, Timer};

use super::{ElyShell, ShellState, auth};

const CLOUD_SYNC_UPLOAD_DEBOUNCE: Duration = Duration::from_millis(750);

/// Messages the off-thread sync workers push back to the shell so
/// `SyncConnectionState` on `BrowserCore` and the in-flight auth
/// form reflect live state without the UI thread ever touching the
/// network. `SignedIn` is the initial-probe state set synchronously
/// on shell startup and does not flow through this channel.
#[derive(Clone, Debug)]
pub(crate) enum SyncStateUpdate {
    SignedOut,
    AwaitingDeviceApproval,
    RemoteSnapshot { bytes: Vec<u8>, logical_clock: u64 },
    SyncReady { last_synced_at_secs: u64 },
    SyncError { message: String },
    AuthOtpSent { email: String },
    AuthSucceeded { email: String },
    AuthError { email: String, message: String },
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

    pub(super) fn queue_cloud_sync_upload(&mut self, logical_clock_floor: Option<u64>) {
        self.sync_upload_pending = true;
        if let Some(floor) = logical_clock_floor {
            self.sync_upload_pending_logical_clock_floor = Some(
                self.sync_upload_pending_logical_clock_floor
                    .map_or(floor, |current| current.max(floor)),
            );
        }
    }

    fn trigger_pending_cloud_sync_upload(&mut self) -> bool {
        if !self.sync_upload_pending {
            return false;
        }

        self.sync_upload_pending = false;
        let logical_clock_floor = self.sync_upload_pending_logical_clock_floor.take();
        match logical_clock_floor {
            Some(floor) => self.trigger_cloud_sync_upload_after_remote(floor),
            None => self.trigger_cloud_sync_upload(),
        }
        true
    }

    fn clear_pending_cloud_sync_upload(&mut self) {
        self.sync_upload_pending = false;
        self.sync_upload_pending_logical_clock_floor = None;
    }

    fn can_schedule_cloud_sync_upload(&self) -> bool {
        let ShellState::Ready(core) = &self.state else {
            return false;
        };
        let Some(snapshot) = core.snapshot().ok() else {
            return false;
        };
        matches!(
            snapshot.sync_status.connection(),
            SyncConnectionState::SignedIn
                | SyncConnectionState::AwaitingDeviceApproval
                | SyncConnectionState::SyncReady { .. }
                | SyncConnectionState::SyncError { .. }
        )
    }

    /// Inspect the on-disk bearer token and seed `SyncConnectionState`
    /// so the Sync settings page reads the startup state on first render.
    pub(super) fn probe_initial_sync_state(&mut self) -> bool {
        let ShellState::Ready(core) = &mut self.state else {
            return false;
        };
        let Some(snapshot) = core.snapshot().ok() else {
            return false;
        };
        let active_profile_id = snapshot.active_profile_id.clone();
        let Some(profile_root) = crate::services::servo_profile_data::default_profile_data_root()
        else {
            return false;
        };
        let profile_dir = crate::services::servo_profile_data::profile_data_dir(
            &profile_root,
            &active_profile_id,
        );
        let bearer_path = profile_dir.join("sync").join("bearer.token");
        let bearer_present = bearer_token_file_present(&bearer_path);
        let state = if bearer_present {
            ely_domain::SyncConnectionState::SignedIn
        } else {
            ely_domain::SyncConnectionState::SignedOut
        };
        core.set_sync_connection_state(state);
        bearer_present
    }

    /// Drain any sync upload outcomes the off-thread worker pushed
    /// since the previous tick and stamp the resulting connection
    /// state on `BrowserCore`. Returns `true` when at least one
    /// update was applied so callers can `cx.notify()` accordingly.
    pub(super) fn drain_sync_updates(&mut self) -> bool {
        let mut latest_connection: Option<SyncConnectionState> = None;
        let mut auth_changed = false;
        let mut trigger_initial_sync = false;
        let mut trigger_merged_upload = None;
        let mut upload_finished = false;
        while let Ok(update) = self.sync_inbox_rx.try_recv() {
            match update {
                SyncStateUpdate::SignedOut => {
                    latest_connection = Some(SyncConnectionState::SignedOut);
                    upload_finished = true;
                }
                SyncStateUpdate::AwaitingDeviceApproval => {
                    latest_connection = Some(SyncConnectionState::AwaitingDeviceApproval);
                    upload_finished = true;
                }
                SyncStateUpdate::RemoteSnapshot { bytes, logical_clock } => {
                    upload_finished = true;
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
                                trigger_merged_upload = Some(logical_clock);
                            }
                            Err(error) => {
                                latest_connection = Some(SyncConnectionState::SyncError {
                                    message: error.to_string(),
                                });
                            }
                        }
                    }
                }
                SyncStateUpdate::SyncReady { last_synced_at_secs } => {
                    latest_connection =
                        Some(SyncConnectionState::SyncReady { last_synced_at_secs });
                    upload_finished = true;
                }
                SyncStateUpdate::SyncError { message } => {
                    latest_connection = Some(SyncConnectionState::SyncError { message });
                    upload_finished = true;
                }
                SyncStateUpdate::AuthOtpSent { email } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::AwaitingOtp { email };
                    auth_changed = true;
                }
                SyncStateUpdate::AuthSucceeded { email } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::Idle;
                    latest_connection = Some(SyncConnectionState::SignedIn);
                    trigger_initial_sync = true;
                    tracing::info!(target: "ely::sync", email = %email, "email OTP sign-in succeeded");
                    auth_changed = true;
                }
                SyncStateUpdate::AuthError { email, message } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::Error { email, message };
                    auth_changed = true;
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
        if let Some(logical_clock_floor) = trigger_merged_upload {
            self.clear_pending_cloud_sync_upload();
            self.trigger_cloud_sync_upload_after_remote(logical_clock_floor);
        } else if upload_finished {
            self.trigger_pending_cloud_sync_upload();
        }

        auth_changed || trigger_initial_sync || merged_upload_requested || connection_changed
    }
}

fn bearer_token_file_present(path: &Path) -> bool {
    std::fs::metadata(path).map(|metadata| metadata.len() > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::bearer_token_file_present;

    #[test]
    fn bearer_token_file_presence_requires_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let suffix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("ely-sync-token-probe-{}", suffix));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("bearer.token");

        assert!(!bearer_token_file_present(&path));

        std::fs::write(&path, "")?;
        assert!(!bearer_token_file_present(&path));

        std::fs::write(&path, "session-token")?;
        assert!(bearer_token_file_present(&path));

        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
}
