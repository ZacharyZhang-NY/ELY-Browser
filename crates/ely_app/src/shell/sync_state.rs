use std::{path::Path, time::Duration};

use ely_domain::{ProfileKind, SyncConnectionState};
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

    pub(super) fn clear_pending_cloud_sync_upload(&mut self) {
        self.sync_upload_pending = false;
        self.sync_upload_pending_logical_clock_floor = None;
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
        let ShellState::Ready(core) = &mut self.state else {
            return false;
        };
        probe_initial_sync_state_at(core, &profile_root)
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

fn probe_initial_sync_state_at(
    core: &mut ely_browser_core::BrowserCore,
    profile_root: &Path,
) -> bool {
    let Some(snapshot) = core.snapshot().ok() else {
        return false;
    };
    let profile_dir = crate::services::servo_profile_data::sync_profile_data_dir(
        profile_root,
        &snapshot.active_profile_id,
    );
    if !core.active_profile_allows_sync() {
        if let Err(error) = auth::clear_persisted_bearer(&profile_dir) {
            tracing::warn!(target: "ely::sync", error = %error, "private bearer cleanup failed");
        }
        core.set_sync_connection_state(SyncConnectionState::SignedOut);
        return false;
    }
    if snapshot.active_profile_name == "Default"
        && matches!(snapshot.active_profile_kind, ProfileKind::Standard)
    {
        migrate_legacy_default_sync_dir(profile_root, &profile_dir);
    }
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

fn bearer_token_file_present(path: &Path) -> bool {
    std::fs::metadata(path).map(|metadata| metadata.len() > 0).unwrap_or(false)
}

fn migrate_legacy_default_sync_dir(profile_root: &Path, stable_profile_dir: &Path) {
    let stable_sync_dir = stable_profile_dir.join("sync");
    if stable_sync_dir.exists() {
        return;
    }

    let candidate = profile_root.join("default").join("servo").join("sync");
    if !bearer_token_file_present(&candidate.join("bearer.token")) {
        return;
    }
    if let Err(error) = copy_dir_recursive(&candidate, &stable_sync_dir) {
        tracing::warn!(
            target: "ely::sync",
            error = %error,
            source = %candidate.display(),
            "legacy sync profile migration failed",
        );
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), destination_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};
    use ely_domain::SyncConnectionState;

    use super::{
        bearer_token_file_present, migrate_legacy_default_sync_dir, probe_initial_sync_state_at,
    };
    use crate::services::servo_profile_data::sync_profile_data_dir;

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

    #[test]
    fn known_default_sync_directory_is_migrated() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let legacy = directory.path().join("default/servo/sync");
        let stable = directory.path().join("profile_stable/servo");
        std::fs::create_dir_all(&legacy)?;
        std::fs::write(legacy.join("bearer.token"), "default-token")?;

        migrate_legacy_default_sync_dir(directory.path(), &stable);

        assert_eq!(std::fs::read_to_string(stable.join("sync/bearer.token"))?, "default-token");
        Ok(())
    }

    #[test]
    fn custom_profile_bearer_is_ignored_during_default_migration()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let custom = directory.path().join("profile_custom/servo/sync");
        let stable = directory.path().join("profile_stable/servo");
        std::fs::create_dir_all(&custom)?;
        std::fs::write(custom.join("bearer.token"), "custom-token")?;

        migrate_legacy_default_sync_dir(directory.path(), &stable);

        assert!(!stable.join("sync/bearer.token").exists());
        Ok(())
    }

    #[test]
    fn private_startup_clears_a_persisted_bearer() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let mut core = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
        let profile_id = core.snapshot()?.active_profile_id;
        let profile_dir = sync_profile_data_dir(directory.path(), &profile_id);
        let bearer_path = profile_dir.join("sync/bearer.token");
        std::fs::create_dir_all(bearer_path.parent().ok_or("missing bearer parent")?)?;
        std::fs::write(&bearer_path, "a".repeat(64))?;
        std::fs::write(bearer_path.with_extension("tmp"), "b".repeat(64))?;
        core.set_sync_connection_state(SyncConnectionState::SignedIn);

        assert!(!probe_initial_sync_state_at(&mut core, directory.path()));
        assert!(!bearer_path.exists());
        assert!(!bearer_path.with_extension("tmp").exists());
        assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SignedOut);

        Ok(())
    }

    #[test]
    fn standard_startup_preserves_a_persisted_bearer() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let profile_id = core.snapshot()?.active_profile_id;
        let profile_dir = sync_profile_data_dir(directory.path(), &profile_id);
        let bearer_path = profile_dir.join("sync/bearer.token");
        std::fs::create_dir_all(bearer_path.parent().ok_or("missing bearer parent")?)?;
        std::fs::write(&bearer_path, "a".repeat(64))?;

        assert!(probe_initial_sync_state_at(&mut core, directory.path()));
        assert!(bearer_path.exists());
        assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SignedIn);

        Ok(())
    }
}
