use std::{
    num::NonZeroU32,
    time::{Duration, UNIX_EPOCH},
};

use ely_domain::{HistoryEntry, SyncObjectKind, SyncObjectPolicy, TabId, UrlText};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error, sync_context::SyncSnapshotApplyContext};
use crate::{sync_engine::SyncSnapshotApplySummary, sync_records::HistorySyncRecord};

impl BrowserCore {
    pub(crate) fn visible_history_for_sync(&self) -> Vec<&HistoryEntry> {
        if self.sync_object_policy(SyncObjectKind::History) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.history_entries
            .iter()
            .filter(|entry| {
                self.profile_allows_cloud_sync(entry.profile_id())
                    && self.space_allows_sync(entry.space_id())
            })
            .collect()
    }

    pub(super) fn apply_history_sync_record(
        &mut self,
        record: HistorySyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let profile_id = self.sync_profile_id(&record.profile_id, context)?;
        let space_id = self.sync_space_id(&record.space_id, record.space_name.as_deref())?;
        let url = UrlText::parse(&record.url).map_err(snapshot_schema_error)?;
        let source_tab_id = self.resolve_history_source_tab_id(
            &record.source_tab_id,
            &profile_id,
            &space_id,
            &url,
        )?;
        let visited_at = UNIX_EPOCH + Duration::from_secs(record.visited_at_secs);
        let visit_count = NonZeroU32::new(record.visit_count).ok_or_else(|| {
            snapshot_schema_error("history visit_count must be greater than zero")
        })?;
        let existing_index = self.history_entries.iter().position(|entry| {
            entry.profile_id() == &profile_id
                && entry.space_id() == &space_id
                && entry.url() == &url
        });
        let entry = HistoryEntry::new(
            profile_id,
            space_id,
            source_tab_id,
            record.title,
            url,
            record.favicon_key,
            visited_at,
        );
        let entry = HistoryEntry::restore(entry, visit_count);

        match existing_index {
            Some(index) if self.history_entries[index] == entry => summary.record_skipped(),
            Some(index) => {
                self.history_entries[index] = entry;
                summary.record_updated();
            }
            None => {
                self.history_entries.push(entry);
                summary.record_imported();
            }
        }
        Ok(())
    }

    fn resolve_history_source_tab_id(
        &self,
        raw: &str,
        profile_id: &ely_domain::ProfileId,
        space_id: &ely_domain::SpaceId,
        url: &UrlText,
    ) -> Result<TabId, SyncClientError> {
        let remote_tab_id = TabId::parse(raw).map_err(snapshot_schema_error)?;
        if self.tabs.iter().any(|tab| tab.id() == &remote_tab_id) {
            return Ok(remote_tab_id);
        }
        if let Some(tab) = self.tabs.iter().find(|tab| {
            tab.profile_id() == profile_id && tab.space_id() == space_id && tab.url() == url
        }) {
            return Ok(tab.id().clone());
        }
        Ok(remote_tab_id)
    }
}
