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
    SendingCode { email: String },
    /// The worker accepted the request and Cloudflare's `SEND_EMAIL`
    /// binding handed the message to the recipient's MTA. UI now
    /// reveals the OTP field.
    AwaitingOtp { email: String },
    /// `verify_email_otp` is in flight. UI shows a transient
    /// "Verifying…" state.
    Verifying { email: String },
    /// Last attempt failed. UI surfaces the message inline so the
    /// user knows what to retry.
    Error { email: String, message: String },
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
}

impl ElyShell {
    /// Hand the typed email to the worker thread that calls
    /// `send_email_otp`. The thread reports success / failure back
    /// through the shared `SyncStateUpdate` channel, which the next
    /// shell tick reconciles into the `auth_flow_phase`.
    pub(crate) fn submit_email_otp_request(&mut self, cx: &mut Context<Self>) {
        if active_profile_sync_context_for(&self.state).is_none() {
            return;
        }
        let email = self.read_auth_email_input(cx);
        let Some(email) = normalize_email(&email) else {
            self.auth_flow_phase = AuthFlowPhase::Error {
                email: String::new(),
                message: "Enter a valid email to receive a code.".to_string(),
            };
            return;
        };
        self.auth_flow_phase = AuthFlowPhase::SendingCode { email: email.clone() };
        let tx = self.sync_inbox_tx.clone();
        spawn_send_otp(email, tx);
    }

    /// Hand the typed OTP to the worker thread that calls
    /// `verify_email_otp`, persists the bearer token, and triggers
    /// the first snapshot upload on success.
    pub(crate) fn submit_email_otp_verify(&mut self, cx: &mut Context<Self>) {
        let email = match self.auth_flow_phase.clone() {
            AuthFlowPhase::AwaitingOtp { email }
            | AuthFlowPhase::Error { email, .. }
            | AuthFlowPhase::Verifying { email } => email,
            _ => return,
        };
        let otp = self.read_auth_otp_input(cx);
        let normalized_otp = otp.trim().replace(['-', ' '], "");
        if normalized_otp.is_empty() {
            self.auth_flow_phase =
                AuthFlowPhase::Error { email, message: "Enter the code you received.".to_string() };
            return;
        }
        let active_profile = match active_profile_sync_context_for(&self.state) {
            Some(profile) => profile,
            None => return,
        };
        let Some(profile_root) = default_profile_data_root() else {
            self.auth_flow_phase = AuthFlowPhase::Error {
                email,
                message: "Profile data root is unavailable on this machine.".to_string(),
            };
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &active_profile.id);
        self.auth_flow_phase = AuthFlowPhase::Verifying { email: email.clone() };
        let tx = self.sync_inbox_tx.clone();
        spawn_verify_otp(email, normalized_otp, profile_dir, tx);
    }

    /// Drop the persisted bearer token and reset the local form.
    /// The bearer file is removed synchronously — there is no network
    /// call to make, the token is the only artefact we own.
    pub(crate) fn submit_sign_out(&mut self, _cx: &mut Context<Self>) {
        self.auth_flow_phase = AuthFlowPhase::Idle;
        let active_profile_id = match active_profile_id_for(&self.state) {
            Some(profile_id) => profile_id,
            None => return,
        };
        let Some(profile_root) = default_profile_data_root() else {
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &active_profile_id);
        if let Err(error) = clear_persisted_bearer(&profile_dir) {
            tracing::warn!(target: "ely::sync", error = %error, "sign-out failed to clear bearer");
        }
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_connection_state(ely_domain::SyncConnectionState::SignedOut);
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

pub(super) fn clear_persisted_bearer(profile_dir: &Path) -> Result<(), SyncClientError> {
    BearerTokenStore::new(profile_dir.join("sync").join("bearer.token")).clear()
}

fn spawn_send_otp(email: String, tx: Sender<SyncStateUpdate>) {
    std::thread::Builder::new()
        .name("ely-sync-auth-send".to_string())
        .spawn(move || {
            let config = ApiClientConfig::production();
            match send_email_otp(&config, &email) {
                Ok(()) => {
                    let _ = tx.send(SyncStateUpdate::AuthOtpSent { email });
                }
                Err(error) => {
                    let _ =
                        tx.send(SyncStateUpdate::AuthError { email, message: error.to_string() });
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-send failed");
        });
}

fn spawn_verify_otp(
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
                        email,
                        message: error.to_string(),
                    });
                    return;
                }
            };
            let mut engine =
                match SyncEngine::for_profile_dir(&profile_dir, "ELY", sync_platform_label()) {
                    Ok(engine) => engine,
                    Err(error) => {
                        let _ = tx.send(SyncStateUpdate::AuthError {
                            email,
                            message: error.to_string(),
                        });
                        return;
                    }
                };
            if let Err(error) = engine.install_bearer(token.as_str()) {
                let _ = tx.send(SyncStateUpdate::AuthError { email, message: error.to_string() });
                return;
            }
            let _ = tx.send(SyncStateUpdate::AuthSucceeded { email });
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-verify failed");
        });
}

#[cfg(test)]
mod tests {
    use ely_browser_core::{BrowserCore, InitialBrowserConfig};

    use super::{AuthFlowPhase, active_profile_sync_context_for, normalize_email};
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
        let phase = AuthFlowPhase::Verifying { email: "you@there".to_string() };
        assert!(phase.is_busy());
        assert_eq!(phase.error_message(), None);

        let phase = AuthFlowPhase::Error {
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
}
