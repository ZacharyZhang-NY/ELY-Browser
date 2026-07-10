use ely_sync_client::{
    SyncClientError, SyncLatestSnapshotDocument, SyncSnapshotHeadConflictDocument,
};

pub(super) fn conflict_head(
    conflict: SyncSnapshotHeadConflictDocument,
) -> Result<SyncLatestSnapshotDocument, SyncClientError> {
    conflict.current_head.ok_or(SyncClientError::SnapshotBusy)
}

pub(super) fn ensure_remote_generation_is_available(
    remote_generation: u64,
    resolved_generation: u64,
) -> Result<(), SyncClientError> {
    if remote_generation > resolved_generation {
        return Err(SyncClientError::SnapshotBusy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_conflict_head_requests_a_bounded_retry() {
        let conflict = SyncSnapshotHeadConflictDocument {
            version: 1,
            error: "sync_snapshot_head_conflict".to_string(),
            current_head: None,
        };
        assert!(matches!(conflict_head(conflict), Err(SyncClientError::SnapshotBusy)));
    }

    #[test]
    fn remote_vault_generation_ahead_requests_a_bounded_retry() {
        assert!(matches!(
            ensure_remote_generation_is_available(2, 1),
            Err(SyncClientError::SnapshotBusy)
        ));
        assert!(ensure_remote_generation_is_available(2, 2).is_ok());
    }
}
