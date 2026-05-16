use std::time::{Duration, UNIX_EPOCH};

use ely_domain::{
    ReadingListEntry, ReadingListId, ReadingProgress, ReadingProgressPercent, SyncObjectKind,
    SyncObjectPolicy, UrlText,
};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error};
use crate::{
    sync_engine::SyncSnapshotApplySummary,
    sync_records::{ReadingListSyncRecord, ReadingProgressSyncRecord},
};

impl BrowserCore {
    pub(crate) fn visible_reading_list_for_sync(&self) -> Vec<&ReadingListEntry> {
        if self.sync_object_policy(SyncObjectKind::ReadingList) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.reading_list
            .iter()
            .filter(|entry| {
                self.profiles
                    .iter()
                    .any(|profile| profile.id() == entry.profile_id() && profile.allows_sync())
            })
            .collect()
    }

    pub(super) fn apply_reading_list_sync_record(
        &mut self,
        record: ReadingListSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
    ) -> Result<(), SyncClientError> {
        let entry_id = ReadingListId::parse(&record.id).map_err(snapshot_schema_error)?;
        let profile_id = self.sync_profile_id(&record.profile_id)?;
        let space_id = self.sync_space_id(&record.space_id, record.space_name.as_deref())?;
        let source_url = UrlText::parse(&record.source_url).map_err(snapshot_schema_error)?;
        let progress = reading_progress_from_sync_record(record.progress)?;
        let added_at = UNIX_EPOCH + Duration::from_secs(record.added_at_secs);
        let existing_index = self.reading_list_sync_index(&entry_id, &profile_id, &source_url);
        let id = existing_index
            .and_then(|index| self.reading_list.get(index).map(|entry| entry.id().clone()))
            .unwrap_or(entry_id);
        let mut entry =
            ReadingListEntry::new(profile_id, space_id, record.title, source_url, added_at)
                .map_err(snapshot_schema_error)?;
        entry.set_progress(progress);
        let entry = ReadingListEntry::restore(id, entry);

        match existing_index {
            Some(index) if self.reading_list[index] == entry => summary.record_skipped(),
            Some(index) => {
                self.reading_list[index] = entry;
                summary.record_updated();
            }
            None => {
                self.reading_list.push(entry);
                summary.record_imported();
            }
        }
        Ok(())
    }

    fn reading_list_sync_index(
        &self,
        entry_id: &ReadingListId,
        profile_id: &ely_domain::ProfileId,
        source_url: &UrlText,
    ) -> Option<usize> {
        self.reading_list.iter().position(|entry| entry.id() == entry_id).or_else(|| {
            self.reading_list.iter().position(|entry| {
                entry.profile_id() == profile_id && entry.source_url() == source_url
            })
        })
    }
}

fn reading_progress_from_sync_record(
    record: ReadingProgressSyncRecord,
) -> Result<ReadingProgress, SyncClientError> {
    match record {
        ReadingProgressSyncRecord::Unread => Ok(ReadingProgress::Unread),
        ReadingProgressSyncRecord::InProgress { percent } => ReadingProgressPercent::new(percent)
            .map(ReadingProgress::InProgress)
            .map_err(snapshot_schema_error),
        ReadingProgressSyncRecord::Finished => Ok(ReadingProgress::Finished),
    }
}
