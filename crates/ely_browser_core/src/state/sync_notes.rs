use std::time::{Duration, UNIX_EPOCH};

use ely_domain::{
    NoteEntry, NoteId, NoteTarget, ProfileId, SpaceId, SyncObjectKind, SyncObjectPolicy, TabId,
    UrlText,
};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync::snapshot_schema_error, sync_context::SyncSnapshotApplyContext};
use crate::{
    sync_engine::SyncSnapshotApplySummary,
    sync_records::{NoteSyncRecord, NoteTargetSyncRecord},
};

impl BrowserCore {
    pub(crate) fn visible_notes_for_sync(&self) -> Vec<&NoteEntry> {
        if self.sync_object_policy(SyncObjectKind::Notes) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.notes
            .iter()
            .filter(|note| {
                self.profile_allows_cloud_sync(note.profile_id())
                    && self.space_allows_sync(note.space_id())
            })
            .collect()
    }

    pub(super) fn apply_note_sync_record(
        &mut self,
        record: NoteSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let note_id = NoteId::parse(&record.id).map_err(snapshot_schema_error)?;
        let profile_id = self.sync_profile_id(&record.profile_id, context)?;
        let space_id = self.sync_space_id(&record.space_id, record.space_name.as_deref())?;
        let source_url = UrlText::parse(&record.source_url).map_err(snapshot_schema_error)?;
        let target =
            self.resolve_note_sync_target(record.target, &profile_id, &space_id, &source_url)?;
        let created_at = UNIX_EPOCH + Duration::from_secs(record.created_at_secs);
        let updated_at = UNIX_EPOCH + Duration::from_secs(record.updated_at_secs);
        let existing_index = self.note_sync_index(&note_id, &profile_id, &target);
        let id = existing_index
            .and_then(|index| self.notes.get(index).map(|note| note.id().clone()))
            .unwrap_or(note_id);
        let note = NoteEntry::new(
            profile_id,
            space_id,
            target,
            record.title,
            source_url,
            record.body,
            created_at,
        )
        .map_err(snapshot_schema_error)?;
        let note = NoteEntry::restore(id, note, updated_at);

        match existing_index {
            Some(index) if self.notes[index] == note => summary.record_skipped(),
            Some(index) => {
                self.notes[index] = note;
                summary.record_updated();
            }
            None => {
                self.notes.push(note);
                summary.record_imported();
            }
        }
        Ok(())
    }

    fn resolve_note_sync_target(
        &self,
        target: NoteTargetSyncRecord,
        profile_id: &ProfileId,
        space_id: &SpaceId,
        source_url: &UrlText,
    ) -> Result<NoteTarget, SyncClientError> {
        match target {
            NoteTargetSyncRecord::Url { url } => {
                UrlText::parse(&url).map(NoteTarget::Url).map_err(snapshot_schema_error)
            }
            NoteTargetSyncRecord::Tab { tab_id } => {
                let remote_tab_id = TabId::parse(tab_id).map_err(snapshot_schema_error)?;
                if let Some(tab) = self.tabs.iter().find(|tab| tab.id() == &remote_tab_id) {
                    return Ok(NoteTarget::Tab(tab.id().clone()));
                }
                if let Some(tab) = self.tabs.iter().find(|tab| {
                    tab.profile_id() == profile_id
                        && tab.space_id() == space_id
                        && tab.url() == source_url
                }) {
                    return Ok(NoteTarget::Tab(tab.id().clone()));
                }
                Ok(NoteTarget::Url(source_url.clone()))
            }
        }
    }

    fn note_sync_index(
        &self,
        note_id: &NoteId,
        profile_id: &ProfileId,
        target: &NoteTarget,
    ) -> Option<usize> {
        self.notes.iter().position(|note| note.id() == note_id).or_else(|| {
            self.notes.iter().position(|note| {
                note.profile_id() == profile_id
                    && note_targets_match_for_sync(note.target(), target)
            })
        })
    }
}

fn note_targets_match_for_sync(left: &NoteTarget, right: &NoteTarget) -> bool {
    match (left, right) {
        (NoteTarget::Url(left_url), NoteTarget::Url(right_url)) => left_url == right_url,
        (NoteTarget::Tab(left_tab), NoteTarget::Tab(right_tab)) => left_tab == right_tab,
        _ => false,
    }
}
