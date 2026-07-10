//! Email + OTP sign-in flow plumbing for the shell.
//!
//! The HTTP work runs on a dedicated `ely-sync-auth` thread so the
//! GPUI render loop never blocks on the network — same invariant the
//! Servo IPC worker enforces. Results flow back to the shell through
//! `SyncStateUpdate` messages drained by `tick_external_web_surfaces`,
//! so the existing 8 ms tick is the single point that reconciles
//! background-task state with `BrowserCore`.

use std::path::Path;

use ely_domain::ProfileId;
use ely_sync_client::{
    ApiClientConfig, BearerToken, BearerTokenStore, SyncApiClient, SyncClientError, send_email_otp,
    verify_email_otp,
};
use gpui::Context;

use crate::services::servo_profile_data::{default_profile_data_root, sync_profile_data_dir};

use super::auth_operation_gate::AuthenticatedOperationBarrier;
use super::sync_inbox::{SyncStateSender, SyncWorkGeneration};
use super::sync_state::SyncStateUpdate;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SignOutPhase {
    SigningOut { generation: SyncWorkGeneration },
    Error { generation: SyncWorkGeneration, message: String },
}

impl SignOutPhase {
    pub(super) fn generation(&self) -> SyncWorkGeneration {
        match self {
            Self::SigningOut { generation } | Self::Error { generation, .. } => *generation,
        }
    }
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
        self.invalidate_sync_work();
        self.hold_auth_flow_barrier();
        self.auth_flow_phase =
            AuthFlowPhase::SendingCode { profile_id: profile_id.clone(), email: email.clone() };
        let tx = self.sync_state_sender();
        spawn_send_otp(profile_id, email, tx);
    }

    /// Hand the typed OTP to the worker thread that calls
    /// `verify_email_otp`; the UI generation validates its result
    /// before persisting the bearer and starting the first sync.
    pub(crate) fn submit_email_otp_verify(&mut self, cx: &mut Context<Self>) {
        let (profile_id, email) = match self.auth_flow_phase.clone() {
            AuthFlowPhase::AwaitingOtp { profile_id, email }
            | AuthFlowPhase::Error { profile_id, email, .. } => (profile_id, email),
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
            self.release_auth_flow_barrier();
            self.auth_flow_phase = AuthFlowPhase::Idle;
            return;
        }
        if default_profile_data_root().is_none() {
            self.release_auth_flow_barrier();
            self.auth_flow_phase = AuthFlowPhase::Error {
                profile_id,
                email,
                message: "Profile data root is unavailable on this machine.".to_string(),
            };
            return;
        };
        self.invalidate_sync_work();
        self.hold_auth_flow_barrier();
        self.auth_flow_phase =
            AuthFlowPhase::Verifying { profile_id: profile_id.clone(), email: email.clone() };
        let tx = self.sync_state_sender();
        spawn_verify_otp(profile_id, email, normalized_otp, tx);
    }

    /// Revoke the remote bearer session, then conditionally clear the
    /// exact local credential used by that request.
    pub(crate) fn submit_sign_out(&mut self, cx: &mut Context<Self>) {
        let active_profile_id = match active_profile_id_for(&self.state) {
            Some(profile_id) => profile_id,
            None => return,
        };
        if matches!(
            self.sign_out_phases.get(&active_profile_id),
            Some(SignOutPhase::SigningOut { .. })
        ) {
            return;
        }
        self.invalidate_sync_work();
        let generation = self.sync_generation;
        let draining_barrier = self.block_authenticated_operations();
        self.sign_out_phases
            .insert(active_profile_id.clone(), SignOutPhase::SigningOut { generation });
        self.auth_flow_phase = AuthFlowPhase::Idle;
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_connection_state(ely_domain::SyncConnectionState::SigningOut);
        }
        cx.notify();
        let Some(profile_root) = default_profile_data_root() else {
            self.set_sign_out_error(
                active_profile_id,
                generation,
                "Profile data root is unavailable. Retry sign out.",
            );
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &active_profile_id);
        let store = bearer_store_for_profile(
            &active_profile_id,
            &profile_dir,
            &profile_root,
            self.default_profile_id.as_ref(),
        );
        let tx = self.sync_state_sender();
        spawn_sign_out(active_profile_id, store, draining_barrier, tx);
    }

    fn set_sign_out_error(
        &mut self,
        profile_id: ProfileId,
        generation: SyncWorkGeneration,
        message: &str,
    ) {
        self.sign_out_phases
            .insert(profile_id, SignOutPhase::Error { generation, message: message.to_string() });
        self.auth_flow_phase = AuthFlowPhase::Idle;
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_connection_state(ely_domain::SyncConnectionState::SignOutError {
                message: message.to_string(),
            });
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

fn spawn_sign_out(
    profile_id: ProfileId,
    store: BearerTokenStore,
    draining_barrier: AuthenticatedOperationBarrier,
    tx: SyncStateSender,
) {
    let worker_tx = tx.clone();
    let worker_profile_id = profile_id.clone();
    let spawn_result = std::thread::Builder::new()
        .name("ely-sync-auth-sign-out".to_string())
        .spawn(move || {
            let update = match store.load() {
                Ok(token) => {
                    draining_barrier.wait_until_idle();
                    sign_out_loaded_token(worker_profile_id, store, token)
                }
                Err(error) => {
                    tracing::warn!(target: "ely::sync", error = %error, "sign-out credential load failed");
                    SyncStateUpdate::SignOutFailed {
                        profile_id: worker_profile_id,
                        message: "System credential access failed. Retry sign out.".to_string(),
                    }
                }
            };
            draining_barrier.release();
            let _ = worker_tx.send(update);
        });
    if let Err(error) = spawn_result {
        tracing::warn!(target: "ely::sync", error = %error, "spawn sign-out worker failed");
        let _ = tx.send(SyncStateUpdate::SignOutFailed {
            profile_id,
            message: "Session revoke failed.".to_string(),
        });
    }
}

fn sign_out_loaded_token(
    profile_id: ProfileId,
    store: BearerTokenStore,
    token: Option<BearerToken>,
) -> SyncStateUpdate {
    let Some(token) = token else {
        return SyncStateUpdate::SignOutSucceeded { profile_id };
    };
    if let Err(error) = SyncApiClient::new(ApiClientConfig::production(), token.clone())
        .and_then(|client| client.sign_out())
    {
        tracing::warn!(target: "ely::sync", error = %error, "remote sign-out failed");
        return SyncStateUpdate::SignOutFailed {
            profile_id,
            message: "Session revoke failed.".to_string(),
        };
    }
    match store.clear_if_matches(&token) {
        Ok(true) => SyncStateUpdate::SignOutSucceeded { profile_id },
        Ok(false) => SyncStateUpdate::SignOutFailed {
            profile_id,
            message: "Session changed. Retry sign out.".to_string(),
        },
        Err(error) => {
            tracing::warn!(target: "ely::sync", error = %error, "sign-out credential clear failed");
            SyncStateUpdate::SignOutFailed {
                profile_id,
                message: "System credential access failed. Retry sign out.".to_string(),
            }
        }
    }
}

fn spawn_send_otp(profile_id: ProfileId, email: String, tx: SyncStateSender) {
    let failure_profile_id = profile_id.clone();
    let failure_email = email.clone();
    let failure_tx = tx.clone();
    let spawn_result =
        std::thread::Builder::new().name("ely-sync-auth-send".to_string()).spawn(move || {
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
        });
    if let Err(error) = spawn_result {
        tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-send failed");
        let _ = failure_tx.send(SyncStateUpdate::AuthError {
            profile_id: failure_profile_id,
            email: failure_email,
            message: "Unable to start sign-in. Retry.".to_string(),
        });
    }
}

fn spawn_verify_otp(profile_id: ProfileId, email: String, otp: String, tx: SyncStateSender) {
    let failure_profile_id = profile_id.clone();
    let failure_email = email.clone();
    let failure_tx = tx.clone();
    let spawn_result =
        std::thread::Builder::new().name("ely-sync-auth-verify".to_string()).spawn(move || {
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
            let update = SyncStateUpdate::AuthVerified { profile_id, email, token };
            if let Err(error) = tx.send(update) {
                retire_stale_auth_update(error.0.update);
            }
        });
    if let Err(error) = spawn_result {
        tracing::warn!(target: "ely::sync", error = %error, "spawn ely-sync-auth-verify failed");
        let _ = failure_tx.send(SyncStateUpdate::AuthError {
            profile_id: failure_profile_id,
            email: failure_email,
            message: "Unable to start sign-in. Retry.".to_string(),
        });
    }
}

pub(super) fn retire_stale_auth_update(update: SyncStateUpdate) {
    let Some(token) = verified_bearer_from(update) else {
        return;
    };
    std::thread::Builder::new()
        .name("ely-sync-auth-retire".to_string())
        .spawn(move || {
            let client = SyncApiClient::new(ApiClientConfig::production(), token);
            if let Ok(client) = client {
                let _ = client.sign_out();
            }
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::warn!(target: "ely::sync", error = %error, "spawn auth retire failed");
        });
}

fn verified_bearer_from(update: SyncStateUpdate) -> Option<BearerToken> {
    match update {
        SyncStateUpdate::AuthVerified { token, .. } => Some(token),
        _ => None,
    }
}

pub(super) fn save_verified_bearer(
    profile_id: &ProfileId,
    token: &BearerToken,
    default_profile_id: Option<&ProfileId>,
) -> Result<(), SyncClientError> {
    let profile_root = default_profile_data_root().ok_or_else(|| {
        SyncClientError::BearerCredentialStorage("profile data root is unavailable".to_string())
    })?;
    let profile_dir = sync_profile_data_dir(&profile_root, profile_id);
    bearer_store_for_profile(profile_id, &profile_dir, &profile_root, default_profile_id)
        .save(token)
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
