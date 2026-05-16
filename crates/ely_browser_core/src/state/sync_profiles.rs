use ely_domain::{Profile, ProfileId, ProfileKind, SyncObjectKind, SyncObjectPolicy};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error, sync_context::SyncSnapshotApplyContext};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::ProfileSyncRecord};

impl BrowserCore {
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
            summary.record_skipped();
            return Ok(());
        }

        let existing_index =
            self.profiles.iter().position(|profile| profile.id() == &profile_id).or_else(|| {
                self.profiles
                    .iter()
                    .position(|profile| profile.name().eq_ignore_ascii_case(record.name.trim()))
            });
        let id = existing_index
            .and_then(|index| self.profiles.get(index).map(|profile| profile.id().clone()))
            .unwrap_or_else(|| profile_id.clone());
        context.register_profile_alias(profile_id, id.clone());
        let mut profile = Profile::new(record.name, record.color_hex, kind);
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
