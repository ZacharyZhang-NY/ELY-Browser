use std::time::SystemTime;

use ely_domain::{ArchivedTab, BookmarkEntry, BrowserTab, HistoryEntry, NoteEntry, SpaceId};

use super::BrowserCore;
pub use super::local_data_export_records::ElyLocalDataPackage;
use super::local_data_export_records::{
    ElyLocalArchivedTabRecord, ElyLocalBookmarkRecord, ElyLocalDownloadRecord,
    ElyLocalHistoryRecord, ElyLocalNoteRecord, ElyLocalProfileRecord, ElyLocalReadingListRecord,
    ElyLocalSitePermissionAuditRecord, ElyLocalSitePermissionRecord, ElyLocalTabRecord,
};
use crate::CoreError;

pub const ELYDATA_SCHEMA_VERSION: u16 = 1;
pub const ELYDATA_FILE_EXTENSION: &str = "elydata";

impl BrowserCore {
    pub fn export_local_data_package_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&self.export_local_data_package()?)
            .map_err(invalid_local_data_package)
    }

    pub fn export_local_data_package(&self) -> Result<ElyLocalDataPackage, CoreError> {
        let profile = self.active_profile()?;
        let profile_id = profile.id();

        Ok(ElyLocalDataPackage {
            version: ELYDATA_SCHEMA_VERSION,
            exported_at_unix_seconds: ElyLocalDataPackage::unix_seconds(SystemTime::now())?,
            profile: ElyLocalProfileRecord::from_profile(profile),
            inventory: self.active_profile_local_data_inventory(),
            open_tabs: self
                .tabs
                .iter()
                .filter(|tab| tab.profile_id() == profile_id)
                .map(|tab| self.local_tab_record(tab))
                .collect::<Result<Vec<_>, _>>()?,
            archived_tabs: self
                .archived_tabs
                .iter()
                .filter(|archived| archived.tab().profile_id() == profile_id)
                .map(|archived| self.local_archived_tab_record(archived))
                .collect::<Result<Vec<_>, _>>()?,
            bookmarks: self
                .bookmarks
                .iter()
                .filter(|bookmark| bookmark.profile_id() == profile_id)
                .map(|bookmark| self.local_bookmark_record(bookmark))
                .collect::<Result<Vec<_>, _>>()?,
            notes: self
                .notes
                .iter()
                .filter(|note| note.profile_id() == profile_id)
                .map(|note| self.local_note_record(note))
                .collect::<Result<Vec<_>, _>>()?,
            reading_list: self
                .reading_list
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .map(|entry| {
                    self.space_name(entry.space_id()).and_then(|space_name| {
                        ElyLocalReadingListRecord::from_entry(entry, space_name)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            history: self
                .history_entries
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .map(|entry| self.local_history_record(entry))
                .collect::<Result<Vec<_>, _>>()?,
            downloads: self
                .download_entries
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .map(ElyLocalDownloadRecord::from_download)
                .collect::<Result<Vec<_>, _>>()?,
            site_permissions: self
                .site_permissions
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .map(ElyLocalSitePermissionRecord::from_site_permission)
                .collect(),
            site_permission_audit_events: self
                .site_permission_audit_events
                .iter()
                .filter(|event| event.profile_id() == profile_id)
                .map(ElyLocalSitePermissionAuditRecord::from_audit_event)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn local_tab_record(&self, tab: &BrowserTab) -> Result<ElyLocalTabRecord, CoreError> {
        ElyLocalTabRecord::from_tab(tab, self.space_name(tab.space_id())?)
    }

    fn local_archived_tab_record(
        &self,
        archived: &ArchivedTab,
    ) -> Result<ElyLocalArchivedTabRecord, CoreError> {
        ElyLocalArchivedTabRecord::from_archived_tab(
            archived,
            self.local_tab_record(archived.tab())?,
        )
    }

    fn local_bookmark_record(
        &self,
        bookmark: &BookmarkEntry,
    ) -> Result<ElyLocalBookmarkRecord, CoreError> {
        ElyLocalBookmarkRecord::from_bookmark(bookmark, self.space_name(bookmark.space_id())?)
    }

    fn local_note_record(&self, note: &NoteEntry) -> Result<ElyLocalNoteRecord, CoreError> {
        ElyLocalNoteRecord::from_note(note, self.space_name(note.space_id())?)
    }

    fn local_history_record(
        &self,
        entry: &HistoryEntry,
    ) -> Result<ElyLocalHistoryRecord, CoreError> {
        ElyLocalHistoryRecord::from_entry(entry, self.space_name(entry.space_id())?)
    }

    fn space_name(&self, space_id: &SpaceId) -> Result<String, CoreError> {
        self.spaces
            .iter()
            .find(|space| space.id() == space_id)
            .map(|space| space.name().to_string())
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })
    }
}

fn invalid_local_data_package(error: serde_json::Error) -> CoreError {
    CoreError::InvalidLocalDataPackage { reason: error.to_string() }
}
