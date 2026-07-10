use ely_domain::{ProfileId, SyncConnectionState};

use super::super::{ElyShell, ShellState, auth};
use crate::services::servo_profile_data::{default_profile_data_root, sync_profile_data_dir};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialState {
    Present,
    Missing,
    Unavailable,
}

pub(super) enum AuthenticationExpiryResolution {
    Ignored,
    ReplacementCredential,
    Connection(SyncConnectionState),
}

#[derive(Debug, Eq, PartialEq)]
enum ExpiryDecision {
    Ignore,
    RefreshReplacement { preserve_generation: bool },
    Reconcile { connection: SyncConnectionState, preserve_generation: bool, finish_sign_out: bool },
}

impl ElyShell {
    pub(super) fn reconcile_authentication_expiry(
        &mut self,
        profile_id: &ProfileId,
        is_current_generation: bool,
    ) -> AuthenticationExpiryResolution {
        let ShellState::Ready(core) = &self.state else {
            return AuthenticationExpiryResolution::Ignored;
        };
        let active_profile_id = match core.snapshot() {
            Ok(snapshot) => snapshot.active_profile_id,
            Err(_) => return AuthenticationExpiryResolution::Ignored,
        };
        let active_matches = &active_profile_id == profile_id;
        if !active_matches {
            return AuthenticationExpiryResolution::Ignored;
        }

        let decision = expiry_decision(
            active_matches,
            is_current_generation,
            self.sign_out_phases.contains_key(profile_id),
            self.credential_state(profile_id),
            preserves_auth_attempt(&self.auth_flow_phase, profile_id),
        );
        match decision {
            ExpiryDecision::Ignore => AuthenticationExpiryResolution::Ignored,
            ExpiryDecision::RefreshReplacement { preserve_generation } => {
                if preserve_generation {
                    self.reset_sync_work_state();
                } else {
                    self.invalidate_sync_work();
                }
                AuthenticationExpiryResolution::ReplacementCredential
            }
            ExpiryDecision::Reconcile { connection, preserve_generation, finish_sign_out } => {
                if finish_sign_out {
                    self.sign_out_phases.remove(profile_id);
                }
                self.reset_after_credential_loss(preserve_generation);
                AuthenticationExpiryResolution::Connection(connection)
            }
        }
    }

    pub(super) fn reconcile_credential_unavailable(&mut self, profile_id: &ProfileId) {
        let preserve_generation = preserves_auth_attempt(&self.auth_flow_phase, profile_id);
        self.reset_after_credential_loss(preserve_generation);
    }

    fn reset_after_credential_loss(&mut self, preserve_generation: bool) {
        if preserve_generation {
            self.reset_sync_work_state();
        } else {
            self.expire_authenticated_work();
            self.auth_flow_phase = auth::AuthFlowPhase::Idle;
        }
    }

    fn credential_state(&self, profile_id: &ProfileId) -> CredentialState {
        let profile_root = match default_profile_data_root() {
            Some(root) => root,
            None => return CredentialState::Unavailable,
        };
        let profile_dir = sync_profile_data_dir(&profile_root, profile_id);
        let store = auth::bearer_store_for_profile(
            profile_id,
            &profile_dir,
            &profile_root,
            self.default_profile_id.as_ref(),
        );
        match store.load() {
            Ok(Some(_)) => CredentialState::Present,
            Ok(None) => CredentialState::Missing,
            Err(error) => {
                tracing::warn!(target: "ely::sync", error = %error, "expired session credential probe failed");
                CredentialState::Unavailable
            }
        }
    }
}

fn expiry_decision(
    active_matches: bool,
    is_current_generation: bool,
    sign_out_active: bool,
    credential: CredentialState,
    preserve_auth_attempt: bool,
) -> ExpiryDecision {
    if !active_matches {
        return ExpiryDecision::Ignore;
    }
    if credential == CredentialState::Present {
        return if is_current_generation && !sign_out_active {
            ExpiryDecision::RefreshReplacement { preserve_generation: preserve_auth_attempt }
        } else {
            ExpiryDecision::Ignore
        };
    }
    if sign_out_active && credential == CredentialState::Unavailable {
        return ExpiryDecision::Ignore;
    }
    let connection = match credential {
        CredentialState::Present => return ExpiryDecision::Ignore,
        CredentialState::Missing => SyncConnectionState::SignedOut,
        CredentialState::Unavailable => SyncConnectionState::CredentialUnavailable {
            message: "System credential access failed.".to_string(),
        },
    };
    ExpiryDecision::Reconcile {
        connection,
        preserve_generation: preserve_auth_attempt,
        finish_sign_out: sign_out_active,
    }
}

fn preserves_auth_attempt(phase: &auth::AuthFlowPhase, profile_id: &ProfileId) -> bool {
    !matches!(phase, auth::AuthFlowPhase::Idle) && phase.belongs_to(profile_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_decision_covers_profile_aba_replacement_and_sign_out() {
        assert_eq!(
            expiry_decision(false, false, false, CredentialState::Missing, false),
            ExpiryDecision::Ignore
        );
        assert_eq!(
            expiry_decision(true, true, false, CredentialState::Present, false),
            ExpiryDecision::RefreshReplacement { preserve_generation: false }
        );
        assert_eq!(
            expiry_decision(true, false, false, CredentialState::Present, false),
            ExpiryDecision::Ignore
        );
        assert_eq!(
            expiry_decision(true, true, true, CredentialState::Unavailable, false),
            ExpiryDecision::Ignore
        );
        assert_eq!(
            expiry_decision(true, false, true, CredentialState::Missing, false),
            ExpiryDecision::Reconcile {
                connection: SyncConnectionState::SignedOut,
                preserve_generation: false,
                finish_sign_out: true,
            }
        );
    }

    #[test]
    fn missing_credential_preserves_a_current_otp_generation() {
        assert_eq!(
            expiry_decision(true, true, false, CredentialState::Present, true),
            ExpiryDecision::RefreshReplacement { preserve_generation: true }
        );
        assert_eq!(
            expiry_decision(true, false, false, CredentialState::Missing, true),
            ExpiryDecision::Reconcile {
                connection: SyncConnectionState::SignedOut,
                preserve_generation: true,
                finish_sign_out: false,
            }
        );
        assert!(matches!(
            expiry_decision(true, false, false, CredentialState::Unavailable, true),
            ExpiryDecision::Reconcile {
                connection: SyncConnectionState::CredentialUnavailable { .. },
                preserve_generation: true,
                finish_sign_out: false,
            }
        ));
    }

    #[test]
    fn current_otp_attempt_is_profile_scoped() {
        let profile_id = ProfileId::new();
        let phase = auth::AuthFlowPhase::Verifying {
            profile_id: profile_id.clone(),
            email: "user@example.com".to_string(),
        };

        assert!(preserves_auth_attempt(&phase, &profile_id));
        assert!(!preserves_auth_attempt(&phase, &ProfileId::new()));
        assert!(!preserves_auth_attempt(&auth::AuthFlowPhase::Idle, &profile_id));
    }
}
