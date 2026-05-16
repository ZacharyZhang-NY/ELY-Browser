use ely_domain::{
    SiteOrigin, SitePermissionDecision, SitePermissionEntry, SitePermissionFeature, SyncObjectKind,
    SyncObjectPolicy,
};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error, sync_context::SyncSnapshotApplyContext};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::SitePermissionSyncRecord};

impl BrowserCore {
    pub(crate) fn visible_site_permissions_for_sync(&self) -> Vec<&SitePermissionEntry> {
        if self.sync_object_policy(SyncObjectKind::SitePermissions) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.site_permissions
            .iter()
            .filter(|entry| self.profile_allows_cloud_sync(entry.profile_id()))
            .collect()
    }

    pub(super) fn apply_site_permission_sync_record(
        &mut self,
        record: SitePermissionSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let profile_id = self.sync_profile_id(&record.profile_id, context)?;
        let origin = SiteOrigin::parse(record.origin).map_err(snapshot_schema_error)?;
        let feature =
            SitePermissionFeature::parse(&record.feature).map_err(snapshot_schema_error)?;
        let decision =
            SitePermissionDecision::parse(&record.decision).map_err(snapshot_schema_error)?;
        let existing_index = self.site_permissions.iter().position(|entry| {
            entry.profile_id() == &profile_id
                && entry.origin() == &origin
                && entry.feature() == feature
        });
        let entry = SitePermissionEntry::new(profile_id, origin, feature, decision);

        match existing_index {
            Some(index) if self.site_permissions[index] == entry => summary.record_skipped(),
            Some(index) => {
                self.site_permissions[index] = entry;
                summary.record_updated();
            }
            None => {
                self.site_permissions.push(entry);
                summary.record_imported();
            }
        }
        Ok(())
    }
}
