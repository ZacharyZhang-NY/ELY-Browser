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
            .filter(|entry| entry.decision() != SitePermissionDecision::AllowOnce)
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
        if decision == SitePermissionDecision::AllowOnce {
            summary.record_skipped();
            return Ok(());
        }
        self.transferred_site_permissions.retain(|permission| {
            permission.entry.profile_id() != &profile_id
                || permission.entry.origin() != &origin
                || permission.entry.feature() != feature
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InitialBrowserConfig, sync_engine::SyncSnapshotApplySummary};

    #[test]
    fn legacy_allow_once_sync_record_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
        let profile_id = core.snapshot()?.active_profile_id;
        let record = SitePermissionSyncRecord {
            profile_id: profile_id.as_str().to_string(),
            origin: "https://example.com".to_string(),
            feature: "camera".to_string(),
            decision: "allow-once".to_string(),
        };
        let mut summary = SyncSnapshotApplySummary::default();

        core.apply_site_permission_sync_record(
            record,
            &mut summary,
            &SyncSnapshotApplyContext::default(),
        )?;

        assert_eq!(summary.skipped(), 1);
        assert!(core.snapshot()?.site_permissions.is_empty());
        Ok(())
    }
}
