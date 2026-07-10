//! Versioned on-disk browser state. The wire shape reuses the sync
//! snapshot records and appliers — one canonical serialization with two
//! consumers — but visibility is local: cloud sync policies never
//! reduce what survives a restart, and Private-profile data never
//! reaches disk.

use serde::{Deserialize, Serialize};

use crate::{
    CoreError,
    state::BrowserCore,
    sync_records::{
        BookmarkSyncRecord, HistorySyncRecord, NoteSyncRecord, PluginSettingsSyncRecord,
        ProfileSyncRecord, ReadingListSyncRecord, SNAPSHOT_SCHEMA_REV, SitePermissionSyncRecord,
        SpaceSyncRecord, SyncSnapshotBody, TabSyncRecord,
    },
};

pub(crate) const LOCAL_STATE_REV: u32 = 1;

#[derive(Serialize, Deserialize)]
struct LocalStateDocument {
    local_rev: u32,
    body: SyncSnapshotBody,
}

impl BrowserCore {
    pub fn build_local_state_bytes(&self) -> Result<Vec<u8>, CoreError> {
        let document =
            LocalStateDocument { local_rev: LOCAL_STATE_REV, body: local_body_from_core(self) };
        serde_json::to_vec(&document)
            .map_err(|error| CoreError::LocalState { reason: error.to_string() })
    }

    pub fn apply_local_state_bytes(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        let document: LocalStateDocument = serde_json::from_slice(bytes)
            .map_err(|error| CoreError::LocalState { reason: error.to_string() })?;
        if document.local_rev != LOCAL_STATE_REV {
            return Err(CoreError::LocalState {
                reason: format!("unsupported local_rev {}", document.local_rev),
            });
        }
        if document.body.schema_rev != SNAPSHOT_SCHEMA_REV {
            return Err(CoreError::LocalState {
                reason: format!("unsupported schema_rev {}", document.body.schema_rev),
            });
        }
        self.apply_sync_snapshot_body(document.body)
            .map_err(|error| CoreError::LocalState { reason: error.to_string() })?;
        Ok(())
    }
}

fn local_body_from_core(core: &BrowserCore) -> SyncSnapshotBody {
    SyncSnapshotBody {
        schema_rev: SNAPSHOT_SCHEMA_REV,
        profiles: core
            .visible_profiles_for_local()
            .into_iter()
            .map(ProfileSyncRecord::from_profile)
            .collect(),
        spaces: core
            .visible_spaces_for_local()
            .into_iter()
            .map(SpaceSyncRecord::from_space)
            .collect(),
        bookmarks: core
            .visible_bookmarks_for_local()
            .into_iter()
            .map(|entry| {
                BookmarkSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
            })
            .collect(),
        tabs: core
            .visible_tabs_for_local()
            .into_iter()
            .map(|entry| {
                TabSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
            })
            .collect(),
        notes: core
            .visible_notes_for_local()
            .into_iter()
            .map(|entry| {
                NoteSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
            })
            .collect(),
        reading_list: core
            .visible_reading_list_for_local()
            .into_iter()
            .map(|entry| {
                ReadingListSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
            })
            .collect(),
        site_permissions: core
            .visible_site_permissions_for_local()
            .into_iter()
            .map(SitePermissionSyncRecord::from_entry)
            .collect(),
        history: core
            .visible_history_for_local()
            .into_iter()
            .map(|entry| {
                HistorySyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
            })
            .collect(),
        plugin_settings: core
            .visible_plugin_settings_for_local()
            .into_iter()
            .map(PluginSettingsSyncRecord::from_plugin)
            .collect(),
    }
}
