//! Email + OTP sign-in flow plumbing for the shell.
//!
//! The HTTP work runs on a dedicated `ely-sync-auth` thread so the
//! GPUI render loop never blocks on the network — same invariant the
//! Servo IPC worker enforces. Results flow back to the shell through
//! `SyncStateUpdate` messages drained by `tick_external_web_surfaces`,
//! so the existing 8 ms tick is the single point that reconciles
//! background-task state with `BrowserCore`.

use std::{path::Path, sync::mpsc::Sender};

use ely_browser_core::SyncEngine;
use ely_domain::ProfileId;
use ely_sync_client::{
    ApiClientConfig, BearerToken, BearerTokenStore, SyncClientError, send_email_otp,
    verify_email_otp,
};
use gpui::Context;

use crate::services::servo_profile_data::{default_profile_data_root, sync_profile_data_dir};

use super::sync_state::{SyncStateUpdate, sync_platform_label};
use super::{ElyShell, ShellState};

/// Where the user is in the email OTP form. Tracked on `ElyShell` so
/// the Sync settings page can pick the right widget cluster (only the
/// email row, OTP row + email row, signed-in account chip, etc.) on
/// every render without re-deriving it from disk.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum AuthFlowPhase {
    /// No sign-in attempt in progress. The form shows just the email
    /// field and a "Send code" button.
    #[default]
    Idle,
    /// `send_email_otp` is in flight. UI disables the form so the
    /// user can't resend before the worker confirms acceptance.
    SendingCode { profile_id: ProfileId, email: String },
    /// The worker accepted the request and Cloudflare's `SEND_EMAIL`
    /// binding handed the message to the recipient's MTA. UI now
    /// reveals the OTP field.
    AwaitingOtp { profile_id: ProfileId, email: String },
    /// `verify_email_otp` is in flight. UI shows a transient
    /// "Verifying…" state.
    Verifying { profile_id: ProfileId, email: String },
    /// Last attempt failed. UI surfaces the message inline so the
    /// user knows what to retry.
    Error { profile_id: ProfileId, email: String, message: String },
}

impl AuthFlowPhase {
    pub(crate) fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error { message, .. } => Some(message.as_str()),
            _ => None,
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        matches!(self, Self::SendingCode { .. } | Self::Verifying { .. })
    }

    pub(crate) fn belongs_to(&self, profile_id: &ProfileId) -> bool {
        match self {
            Self::Idle => true,
            Self::SendingCode { profile_id: owner, .. }
            | Self::AwaitingOtp { profile_id: owner, .. }
            | Self::Verifying { profile_id: owner, .. }
            | Self::Error { profile_id: owner, .. } => owner == profile_id,
        }
    }
}

impl ElyShell {
    /// Hand the typed email to the worker thread that calls
    /// `send_email_otp`. The thread reports success / failure back
    /// through the shared `SyncStateUpdate` channel, which the next
    /// shell tick reconciles into the `auth_flow_phase`.
    pub(crate) fn submit_email_otp_request(&mut self, cx: &mut Context<Self>) {
        let Some(active_profile) = active_profile_sync_context_for(&self.state) else {
            return;
        };
        let email = self.read_auth_email_input(cx);
        let Some(email) = normalize_email(&email) else {
            self.auth_flow_phase = AuthFlowPhase::Error {
                profile_id: active_profile.id,
                email: String::new(),
                message: "Enter a valid email to receive a code.".to_string(),
            };
            return;
        };
        let profile_id = active_profile.id;
        self.auth_flow_phase =
            AuthFlowPhase::SendingCode { profile_id: profile_id.clone(), email: email.clone() };
        let tx = self.sync_inbox_tx.clone();
        spawn_send_otp(profile_id, email, tx);
    }

    /// Hand the typed OTP to the worker thread that calls
    /// `verify_email_otp`, persists the bearer token, and triggers
    /// the first snapshot upload on success.
    pub(crate) fn submit_email_otp_verify(&mut self, cx: &mut Context<Self>) {
        let (profile_id, email) = match self.auth_flow_phase.clone() {
            AuthFlowPhase::AwaitingOtp { profile_id, email }
            | AuthFlowPhase::Error { profile_id, email, .. }
            | AuthFlowPhase::Verifying { profile_id, email } => (profile_id, email),
            _ => return,
        };
        let otp = self.read_auth_otp_input(cx);
        let normalized_otp = otp.trim().replace(['-', ' '], "");
        if normalized_otp.is_empty() {
            self.auth_flow_phase = AuthFlowPhase::Error {
                profile_id,
                email,
                message: "Enter the code you received.".to_string(),
            };
            return;
        }
        let active_profile = match active_profile_sync_context_for(&self.state) {
            Some(profile) => profile,
            None => return,
        };
        if active_profile.id != profile_id {
            self.auth_flow_phase = AuthFlowPhase::Idle;
            return;
        }
        let Some(profile_root) = default_profile_data_root() else {
            self.auth_flow_phase = AuthFlowPhase::Error {
                profile_id,
                email,
                message: "Profile data root is unavailable on this machine.".to_string(),
            };
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &profile_id);
        self.auth_flow_phase =
            AuthFlowPhase::Verifying { profile_id: profile_id.clone(), email: email.clone() };
        let tx = self.sync_inbox_tx.clone();
        spawn_verify_otp(profile_id, email, normalized_otp, profile_dir, tx);
    }

    /// Drop the persisted bearer token and reset the local form.
    pub(crate) fn submit_sign_out(&mut self, _cx: &mut Context<Self>) {
        let active_profile_id = match active_profile_id_for(&self.state) {
            Some(profile_id) => profile_id,
            None => return,
        };
        self.auth_flow_phase = AuthFlowPhase::Idle;
        self.sync_devices.reset();
        self.sync_upload_scheduled = false;
        self.sync_retry_at = None;
        self.clear_pending_cloud_sync_upload();
        let Some(profile_root) = default_profile_data_root() else {
            self.set_sign_out_error(
                active_profile_id,
                "Profile data root is unavailable. Retry sign out.",
            );
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &active_profile_id);
        if let Err(error) = clear_persisted_bearer(
            &active_profile_id,
            &profile_dir,
            &profile_root,
            self.default_profile_id.as_ref(),
        ) {
            tracing::warn!(target: "ely::sync", error = %error, "sign-out failed to clear bearer");
            self.set_sign_out_error(
                active_profile_id,
                "System credential access failed. Retry sign out.",
            );
            return;
        }
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_connection_state(ely_domain::SyncConnectionState::SignedOut);
        }
    }

    fn set_sign_out_error(&mut self, profile_id: ProfileId, message: &str) {
        self.auth_flow_phase =
            AuthFlowPhase::Error { profile_id, email: String::new(), message: message.to_string() };
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_connection_state(
                ely_domain::SyncConnectionState::CredentialUnavailable {
                    message: message.to_string(),
                },
            );
        }
    }

    fn read_auth_email_input(&self, cx: &Context<Self>) -> String {
        self.auth_email_input.read(cx).value().to_string()
    }

    fn read_auth_otp_input(&self, cx: &Context<Self>) -> String {
        self.auth_otp_input.read(cx).value().to_string()
    }
}

fn normalize_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if !trimmed.contains('@') || trimmed.starts_with('@') || trimmed.ends_with('@') {
        return None;
    }
    Some(trimmed.to_lowercase())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveProfileSyncContext {
    id: ProfileId,
}

fn active_profile_sync_context_for(state: &ShellState) -> Option<ActiveProfileSyncContext> {
    let ShellState::Ready(core) = state else {
        return None;
    };
    if !core.active_profile_allows_sync() {
        return None;
    }
    active_profile_id_for(state).map(|id| ActiveProfileSyncContext { id })
}

fn active_profile_id_for(state: &ShellState) -> Option<ProfileId> {
    let ShellState::Ready(core) = state else {
        return None;
    };
    core.snapshot().ok().map(|snapshot| snapshot.active_profile_id)
}

pub(super) fn clear_persisted_bearer(
    profile_id: &ProfileId,
    profile_dir: &Path,
    profile_root: &Path,
    default_profile_id: Option<&ProfileId>,
) -> Result<(), SyncClientError> {
    bearer_store_for_profile(profile_id, profile_dir, profile_root, default_profile_id).clear()
}

pub(super) fn bearer_store_for_profile(
    profile_id: &ProfileId,
    profile_dir: &Path,
    profile_root: &Path,
    default_profile_id: Option<&ProfileId>,
) -> BearerTokenStore {
    let store = BearerTokenStore::new(profile_id, profile_dir);
    if default_profile_id != Some(profile_id) {
        return store;
    }
    store.with_legacy_path(
        profile_root.join("default").join("servo").join("sync").join("bearer.token"),
    )
}

fn spawn_send_otp(profile_id: ProfileId, email: String, tx: Sender<SyncStateUpdate>) {
    std::thread::Builder::new()
        .name("ely-sync-auth-send".to_string())
        .spawn(move || {
            let config = ApiClientConfig::production();
            match send_email_otp(&config, &email) {
                Ok(()) => {
                    let _ = tx.send(SyncStateUpdate::AuthOtpSent { profile_id, email });
                }
                Err(error) => {
                    let _ = tx.send(SyncStateUpdate::AuthError {
                        profile_id,
                        email,
                        message: error.to_string(),
                    });
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-send failed");
        });
}

fn spawn_verify_otp(
    profile_id: ProfileId,
    email: String,
    otp: String,
    profile_dir: std::path::PathBuf,
    tx: Sender<SyncStateUpdate>,
) {
    std::thread::Builder::new()
        .name("ely-sync-auth-verify".to_string())
        .spawn(move || {
            let config = ApiClientConfig::production();
            let token: BearerToken = match verify_email_otp(&config, &email, &otp) {
                Ok(token) => token,
                Err(error) => {
                    let _ = tx.send(SyncStateUpdate::AuthError {
                        profile_id,
                        email,
                        message: error.to_string(),
                    });
                    return;
                }
            };
            let mut engine =
                match SyncEngine::for_profile_dir(
                    &profile_id,
                    &profile_dir,
                    "ELY",
                    sync_platform_label(),
                ) {
                    Ok(engine) => engine,
                    Err(error) => {
                        let _ = tx.send(SyncStateUpdate::AuthError {
                            profile_id,
                            email,
                            message: error.to_string(),
                        });
                        return;
                    }
                };
            if let Err(error) = engine.install_bearer(token.as_str()) {
                let _ = tx.send(SyncStateUpdate::AuthError {
                    profile_id,
                    email,
                    message: error.to_string(),
                });
                return;
            }
            let _ = tx.send(SyncStateUpdate::AuthSucceeded { profile_id, email });
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-verify failed");
        });
}

#[cfg(test)]
mod tests {
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};
    use ely_domain::ProfileId;

    use super::{
        AuthFlowPhase, active_profile_sync_context_for, bearer_store_for_profile, normalize_email,
    };
    use crate::shell::ShellState;

    #[test]
    fn normalize_lowercases_and_trims() {
        assert_eq!(normalize_email("  User@Example.COM  "), Some("user@example.com".to_string()));
    }

    #[test]
    fn normalize_rejects_obviously_broken() {
        assert_eq!(normalize_email("noatsign"), None);
        assert_eq!(normalize_email("@no-local-part"), None);
        assert_eq!(normalize_email("missing-domain@"), None);
        assert_eq!(normalize_email(""), None);
    }

    #[test]
    fn auth_phase_helpers() {
        let profile_id = ProfileId::new();
        let phase = AuthFlowPhase::Verifying {
            profile_id: profile_id.clone(),
            email: "you@there".to_string(),
        };
        assert!(phase.is_busy());
        assert!(phase.belongs_to(&profile_id));
        assert!(!phase.belongs_to(&ProfileId::new()));
        assert_eq!(phase.error_message(), None);

        let phase = AuthFlowPhase::Error {
            profile_id,
            email: "you@there".to_string(),
            message: "rate limited".to_string(),
        };
        assert_eq!(phase.error_message(), Some("rate limited"));
        assert!(!phase.is_busy());
    }

    #[test]
    fn private_profile_has_no_sync_auth_context() -> Result<(), Box<dyn std::error::Error>> {
        let state =
            ShellState::Ready(Box::new(BrowserCore::new(InitialBrowserConfig::private_window()?)?));

        assert_eq!(active_profile_sync_context_for(&state), None);

        let state =
            ShellState::Ready(Box::new(BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?));
        assert!(active_profile_sync_context_for(&state).is_some());

        Ok(())
    }

    #[test]
    fn default_profile_store_cleans_stable_and_old_legacy_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let profile_dir = directory.path().join(profile_id.as_str()).join("servo");
        let stable = profile_dir.join("sync/bearer.token");
        let old = directory.path().join("default/servo/sync/bearer.token");
        std::fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
        std::fs::create_dir_all(old.parent().ok_or("missing old parent")?)?;
        std::fs::write(&stable, "a".repeat(64))?;
        std::fs::write(&old, "b".repeat(64))?;
        let store = bearer_store_for_profile(
            &profile_id,
            &profile_dir,
            directory.path(),
            Some(&profile_id),
        );

        store.clear_legacy_files()?;

        assert!(!stable.exists());
        assert!(!old.exists());
        Ok(())
    }

    #[test]
    fn custom_profile_store_leaves_default_legacy_credentials_untouched()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let default_profile_id = ProfileId::new();
        let profile_dir = directory.path().join(profile_id.as_str()).join("servo");
        let stable = profile_dir.join("sync/bearer.token");
        let old_default = directory.path().join("default/servo/sync/bearer.token");
        std::fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
        std::fs::create_dir_all(old_default.parent().ok_or("missing default parent")?)?;
        std::fs::write(&stable, "a".repeat(64))?;
        std::fs::write(&old_default, "b".repeat(64))?;
        let store = bearer_store_for_profile(
            &profile_id,
            &profile_dir,
            directory.path(),
            Some(&default_profile_id),
        );

        store.clear_legacy_files()?;

        assert!(!stable.exists());
        assert!(old_default.exists());
        Ok(())
    }
}
