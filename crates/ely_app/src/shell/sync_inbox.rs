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
        self.sync_upload_scheduled = false;
        self.sync_upload_in_flight = false;
        self.sync_retry_at = None;
        self.clear_pending_cloud_sync_upload();
        self.sync_devices.reset();
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

        sender.send(SyncStateUpdate::SignedOut { profile_id: ProfileId::new() })?;

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

        old_sender.send(SyncStateUpdate::SignedOut { profile_id: ProfileId::new() })?;

        assert!(!receiver.recv()?.belongs_to(current));
        Ok(())
    }
}
