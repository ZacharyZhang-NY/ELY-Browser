use ely_domain::{Profile, ProfileId, ProfileKind, SpaceId, SyncObjectKind, SyncObjectPolicy};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error, sync_context::SyncSnapshotApplyContext};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::ProfileSyncRecord};

impl BrowserCore {
    #[must_use]
    pub fn active_profile_allows_sync(&self) -> bool {
        self.profile_allows_sync(&self.active_profile_id)
    }

    pub(super) fn profile_allows_sync(&self, profile_id: &ProfileId) -> bool {
        self.profiles
            .iter()
            .find(|profile| profile.id() == profile_id)
            .is_some_and(|profile| profile.allows_sync())
    }

    pub(super) fn profile_allows_cloud_sync(&self, profile_id: &ProfileId) -> bool {
        self.profiles.iter().any(|profile| {
            profile.id() == profile_id
                && profile.allows_sync()
                && profile.sync_policy() == ely_domain::ProfileSyncPolicy::Enabled
        })
    }

    pub(super) fn space_allows_sync(&self, space_id: &SpaceId) -> bool {
        self.spaces
            .iter()
            .find(|space| space.id() == space_id)
            .is_some_and(|space| self.profile_allows_sync(space.default_profile_id()))
    }

    pub(crate) fn visible_profiles_for_sync(&self) -> Vec<&Profile> {
        if self.sync_object_policy(SyncObjectKind::Profiles) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.profiles.iter().filter(|profile| profile.allows_sync()).collect()
    }

    pub(super) fn apply_profile_sync_record(
        &mut self,
        record: ProfileSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &mut SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let profile_id = ProfileId::parse(&record.id).map_err(snapshot_schema_error)?;
        let kind = ProfileKind::from(record.kind);
        if kind == ProfileKind::Private {
            return Err(SyncClientError::SyncPolicy {
                reason: "snapshot contains a private profile".to_string(),
            });
        }
        let name = record.name.trim().to_string();

        let existing_index = self
            .profiles
            .iter()
            .position(|profile| profile.id() == &profile_id && profile.kind() == &kind)
            .or_else(|| {
                self.profiles.iter().position(|profile| {
                    profile.kind() == &kind && profile.name().eq_ignore_ascii_case(&name)
                })
            });
        let id = existing_index
            .and_then(|index| self.profiles.get(index).map(|profile| profile.id().clone()))
            .unwrap_or_else(|| {
                if self.profiles.iter().any(|profile| profile.id() == &profile_id) {
                    ProfileId::new()
                } else {
                    profile_id.clone()
                }
            });
        context.register_profile_alias(profile_id, id.clone());
        let mut profile = Profile::new(name, record.color_hex, kind);
        profile.set_sync_policy(record.sync_policy.into());
        let profile = Profile::restore(id, profile);

        match existing_index {
            Some(index) if self.profiles[index] == profile => {}
            Some(index) => {
                self.profiles[index] = profile;
                summary.record_updated();
            }
            None => {
                self.profiles.push(profile);
                summary.record_imported();
            }
        }
        Ok(())
    }
}
