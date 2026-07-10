use std::sync::mpsc::{SendError, Sender};

use super::{ElyShell, sync_state::SyncStateUpdate};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SyncWorkGeneration(u64);

impl SyncWorkGeneration {
    pub(super) fn advance(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

#[derive(Clone, Debug)]
pub(super) struct SyncStateMessage {
    pub(super) generation: SyncWorkGeneration,
    pub(super) update: SyncStateUpdate,
}

impl SyncStateMessage {
    pub(super) fn belongs_to(&self, generation: SyncWorkGeneration) -> bool {
        self.generation == generation
    }
}

pub(super) fn early_reconciliation_priority(update: &SyncStateUpdate) -> Option<u8> {
    match update {
        SyncStateUpdate::AuthenticationExpired { .. } => Some(0),
        SyncStateUpdate::CredentialUnavailable { .. } => Some(1),
        _ => None,
    }
}

pub(super) fn early_reconciliation_messages(
    messages: &[SyncStateMessage],
) -> impl Iterator<Item = &SyncStateMessage> {
    [0_u8, 1].into_iter().flat_map(move |priority| {
        messages
            .iter()
            .filter(move |message| early_reconciliation_priority(&message.update) == Some(priority))
    })
}

#[derive(Clone, Debug)]
pub(super) struct SyncStateSender {
    generation: SyncWorkGeneration,
    sender: Sender<SyncStateMessage>,
}

impl SyncStateSender {
    pub(super) fn send(
        &self,
        update: SyncStateUpdate,
    ) -> Result<(), Box<SendError<SyncStateMessage>>> {
        self.sender.send(SyncStateMessage { generation: self.generation, update }).map_err(Box::new)
    }
}

impl ElyShell {
    pub(super) fn sync_state_sender(&self) -> SyncStateSender {
        SyncStateSender { generation: self.sync_generation, sender: self.sync_inbox_tx.clone() }
    }

    pub(super) fn invalidate_sync_work(&mut self) {
        self.release_auth_flow_barrier();
        self.sync_generation.advance();
        self.reset_sync_work_state();
    }

    pub(super) fn reset_sync_work_state(&mut self) {
        self.sync_upload_scheduled = false;
        self.sync_upload_in_flight = false;
        self.sync_retry_at = None;
        self.clear_pending_cloud_sync_upload();
        self.sync_devices.reset();
    }

    pub(super) fn expire_authenticated_work(&mut self) {
        self.invalidate_sync_work();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ely_domain::ProfileId;

    #[test]
    fn sender_stamps_the_captured_generation() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let generation = SyncWorkGeneration(7);
        let sender = SyncStateSender { generation, sender };

        sender.send(SyncStateUpdate::AuthenticationExpired { profile_id: ProfileId::new() })?;

        assert_eq!(receiver.recv()?.generation, generation);
        Ok(())
    }

    #[test]
    fn old_generation_stays_stale_after_an_aba_profile_sequence()
    -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut current = SyncWorkGeneration::default();
        let old_sender = SyncStateSender { generation: current, sender };
        current.advance();
        current.advance();

        old_sender.send(SyncStateUpdate::AuthenticationExpired { profile_id: ProfileId::new() })?;

        assert!(!receiver.recv()?.belongs_to(current));
        Ok(())
    }

    #[test]
    fn early_reconciliation_order_is_queue_independent() {
        let profile_id = ProfileId::new();
        let generation = SyncWorkGeneration::default();
        let expired = SyncStateMessage {
            generation,
            update: SyncStateUpdate::AuthenticationExpired { profile_id: profile_id.clone() },
        };
        let unavailable = SyncStateMessage {
            generation,
            update: SyncStateUpdate::CredentialUnavailable {
                profile_id,
                message: "locked".to_string(),
            },
        };

        for messages in
            [vec![expired.clone(), unavailable.clone()], vec![unavailable.clone(), expired.clone()]]
        {
            let ordered = early_reconciliation_messages(&messages)
                .map(|message| early_reconciliation_priority(&message.update))
                .collect::<Vec<_>>();
            assert_eq!(ordered, [Some(0), Some(1)]);
        }
    }

    #[gpui::test]
    async fn authentication_expiry_resets_scoped_work(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);
        let (shell, cx) = cx.add_window_view(ElyShell::new);

        cx.update(|_, app_cx| {
            shell.update(app_cx, |shell, _| {
                let generation = shell.sync_generation;
                shell.sync_upload_scheduled = true;
                shell.sync_upload_in_flight = true;
                shell.sync_upload_pending = true;
                shell.sync_retry_at = Some(std::time::Instant::now());
                shell
                    .sync_devices
                    .set_error(ProfileId::new(), "expired device session".to_string());

                shell.expire_authenticated_work();

                assert_ne!(shell.sync_generation, generation);
                assert!(!shell.sync_upload_scheduled);
                assert!(!shell.sync_upload_in_flight);
                assert!(!shell.sync_upload_pending);
                assert!(shell.sync_retry_at.is_none());
                assert!(shell.sync_devices.error().is_none());
            });
        });
    }
}
