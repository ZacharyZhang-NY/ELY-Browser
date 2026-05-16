use std::collections::BTreeMap;

use ely_domain::ProfileId;

#[derive(Default)]
pub(super) struct SyncSnapshotApplyContext {
    profile_aliases: BTreeMap<ProfileId, ProfileId>,
}

impl SyncSnapshotApplyContext {
    pub(super) fn register_profile_alias(&mut self, remote_id: ProfileId, local_id: ProfileId) {
        self.profile_aliases.insert(remote_id, local_id);
    }

    pub(super) fn profile_alias(&self, remote_id: &ProfileId) -> Option<ProfileId> {
        self.profile_aliases.get(remote_id).cloned()
    }
}
