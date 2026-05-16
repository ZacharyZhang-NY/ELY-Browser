use super::{ElyShell, ShellState};

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
    /// Inspect the on-disk bearer token and seed `SyncConnectionState`
    /// so the Sync settings page reads the startup state on first render.
    pub(super) fn probe_initial_sync_state(&mut self) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };
        let Some(snapshot) = core.snapshot().ok() else {
            return;
        };
        let active_profile_id = snapshot.active_profile_id.clone();
        let Some(profile_root) = crate::services::servo_profile_data::default_profile_data_root()
        else {
            return;
        };
        let profile_dir = crate::services::servo_profile_data::profile_data_dir(
            &profile_root,
            &active_profile_id,
        );
        let bearer_path = profile_dir.join("sync").join("bearer.token");
        let bearer_present = std::fs::metadata(&bearer_path).map(|m| m.len() > 0).unwrap_or(false);
        let state = if bearer_present {
            ely_domain::SyncConnectionState::SignedIn
        } else {
            ely_domain::SyncConnectionState::SignedOut
        };
        core.set_sync_connection_state(state);
    }
}
