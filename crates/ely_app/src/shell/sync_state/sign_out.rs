use ely_domain::{ProfileId, SyncConnectionState};
use std::collections::HashMap;

use super::super::{ElyShell, auth::SignOutPhase};
use crate::shell::{sync_inbox::SyncWorkGeneration, sync_state::SyncStateUpdate};

impl ElyShell {
    pub(super) fn reconcile_sign_out_update(
        &mut self,
        generation: SyncWorkGeneration,
        update: &SyncStateUpdate,
    ) -> Option<(ProfileId, SyncConnectionState)> {
        reconcile_phase(&mut self.sign_out_phases, generation, update)
    }
}

pub(super) fn overlay_active_sign_out_connection(
    core: &mut ely_browser_core::BrowserCore,
    phases: &HashMap<ProfileId, SignOutPhase>,
) -> bool {
    let Some(profile_id) = core.snapshot().ok().map(|snapshot| snapshot.active_profile_id) else {
        return false;
    };
    let Some(phase) = phases.get(&profile_id) else {
        return false;
    };
    core.set_sync_connection_state(connection_for_phase(phase));
    true
}

fn reconcile_phase(
    phases: &mut HashMap<ProfileId, SignOutPhase>,
    generation: SyncWorkGeneration,
    update: &SyncStateUpdate,
) -> Option<(ProfileId, SyncConnectionState)> {
    let (profile_id, failure) = match update {
        SyncStateUpdate::SignOutSucceeded { profile_id } => (profile_id, None),
        SyncStateUpdate::SignOutFailed { profile_id, message } => {
            (profile_id, Some(message.clone()))
        }
        _ => return None,
    };
    let owns_phase = phases.get(profile_id).is_some_and(|phase| phase.generation() == generation);
    if !owns_phase {
        return None;
    }

    let connection = match failure {
        Some(message) => {
            phases.insert(
                profile_id.clone(),
                SignOutPhase::Error { generation, message: message.clone() },
            );
            SyncConnectionState::SignOutError { message }
        }
        None => {
            phases.remove(profile_id);
            SyncConnectionState::SignedOut
        }
    };
    Some((profile_id.clone(), connection))
}

fn connection_for_phase(phase: &SignOutPhase) -> SyncConnectionState {
    match phase {
        SignOutPhase::SigningOut { .. } => SyncConnectionState::SigningOut,
        SignOutPhase::Error { message, .. } => {
            SyncConnectionState::SignOutError { message: message.clone() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completions_only_update_their_profile_and_generation() {
        let profile_a = ProfileId::new();
        let profile_b = ProfileId::new();
        let generation_a = SyncWorkGeneration::default();
        let mut generation_b = generation_a;
        generation_b.advance();
        let mut phases = HashMap::from([
            (profile_a.clone(), SignOutPhase::SigningOut { generation: generation_a }),
            (profile_b.clone(), SignOutPhase::SigningOut { generation: generation_b }),
        ]);

        let success = SyncStateUpdate::SignOutSucceeded { profile_id: profile_a.clone() };
        assert!(reconcile_phase(&mut phases, generation_a, &success).is_some());
        assert!(!phases.contains_key(&profile_a));
        assert_eq!(phases.get(&profile_b).map(SignOutPhase::generation), Some(generation_b));

        let mut newer_generation_a = generation_b;
        newer_generation_a.advance();
        phases
            .insert(profile_a.clone(), SignOutPhase::SigningOut { generation: newer_generation_a });
        let stale_failure = SyncStateUpdate::SignOutFailed {
            profile_id: profile_a.clone(),
            message: "stale".to_string(),
        };
        assert!(reconcile_phase(&mut phases, generation_a, &stale_failure).is_none());
        assert_eq!(phases.get(&profile_a).map(SignOutPhase::generation), Some(newer_generation_a));
    }

    #[test]
    fn failure_phase_preserves_retry_state() -> Result<(), &'static str> {
        let profile_id = ProfileId::new();
        let generation = SyncWorkGeneration::default();
        let mut phases =
            HashMap::from([(profile_id.clone(), SignOutPhase::SigningOut { generation })]);
        let failure = SyncStateUpdate::SignOutFailed {
            profile_id: profile_id.clone(),
            message: "Session revoke failed.".to_string(),
        };

        let (_, connection) =
            reconcile_phase(&mut phases, generation, &failure).ok_or("missing owned result")?;

        assert!(matches!(connection, SyncConnectionState::SignOutError { .. }));
        assert!(matches!(
            phases.get(&profile_id),
            Some(SignOutPhase::Error { message, .. }) if message == "Session revoke failed."
        ));
        Ok(())
    }

    #[test]
    fn profile_switches_reapply_the_owned_sign_out_connection()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut core = ely_browser_core::BrowserCore::new(
            ely_browser_core::InitialBrowserConfig::ely_defaults()?,
        )?;
        let profile_a = core.snapshot()?.active_profile_id;
        let profile_b =
            core.create_profile("Profile B", 0x26251e, ely_domain::ProfileKind::Standard)?;
        let generation = SyncWorkGeneration::default();
        let mut phases =
            HashMap::from([(profile_a.clone(), SignOutPhase::SigningOut { generation })]);

        core.select_profile(&profile_a)?;
        core.set_sync_connection_state(SyncConnectionState::SignedIn);
        assert!(overlay_active_sign_out_connection(&mut core, &phases));
        assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SigningOut);
        assert!(!core.cloud_sync_upload_enabled());

        core.select_profile(&profile_b)?;
        core.set_sync_connection_state(SyncConnectionState::SignedIn);
        assert!(!overlay_active_sign_out_connection(&mut core, &phases));
        assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SignedIn);
        assert!(core.cloud_sync_upload_enabled());

        phases.insert(
            profile_a.clone(),
            SignOutPhase::Error { generation, message: "Session revoke failed.".to_string() },
        );
        core.select_profile(&profile_a)?;
        core.set_sync_connection_state(SyncConnectionState::SignedIn);
        assert!(overlay_active_sign_out_connection(&mut core, &phases));
        assert!(matches!(
            core.snapshot()?.sync_status.connection(),
            SyncConnectionState::SignOutError { .. }
        ));
        assert!(!core.cloud_sync_upload_enabled());
        Ok(())
    }
}
