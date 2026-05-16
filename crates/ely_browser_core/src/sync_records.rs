use std::time::{SystemTime, UNIX_EPOCH};

use ely_domain::{ArchivePolicy, BookmarkEntry, BrowserTab, Space, TabFlags};
use serde::{Deserialize, Serialize};

use crate::state::BrowserCore;

pub(crate) const SNAPSHOT_SCHEMA_REV: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(crate) struct SyncSnapshotBody {
    pub(crate) schema_rev: u32,
    #[serde(default)]
    pub(crate) spaces: Vec<SpaceSyncRecord>,
    pub(crate) bookmarks: Vec<BookmarkSyncRecord>,
    #[serde(default)]
    pub(crate) tabs: Vec<TabSyncRecord>,
}

impl SyncSnapshotBody {
    pub(crate) fn from_core(core: &BrowserCore) -> Self {
        Self {
            schema_rev: SNAPSHOT_SCHEMA_REV,
            spaces: core
                .visible_spaces_for_sync()
                .into_iter()
                .map(SpaceSyncRecord::from_space)
                .collect(),
            bookmarks: core
                .visible_bookmarks_for_sync()
                .into_iter()
                .map(|entry| {
                    BookmarkSyncRecord::from_entry(
                        entry,
                        core.sync_space_name_for(entry.space_id()),
                    )
                })
                .collect(),
            tabs: core
                .visible_tabs_for_sync()
                .into_iter()
                .map(|entry| {
                    TabSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SpaceSyncRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) accent_hex: u32,
    pub(crate) default_profile_id: String,
    pub(crate) archive_policy: SpaceArchivePolicySyncRecord,
    pub(crate) sidebar_width_px: u16,
    pub(crate) sort_key: u64,
}

impl SpaceSyncRecord {
    fn from_space(space: &Space) -> Self {
        Self {
            id: space.id().as_str().to_string(),
            name: space.name().to_string(),
            icon: space.icon().to_string(),
            accent_hex: space.accent_hex(),
            default_profile_id: space.default_profile_id().as_str().to_string(),
            archive_policy: SpaceArchivePolicySyncRecord::from(space.archive_policy()),
            sidebar_width_px: space.sidebar_width_px(),
            sort_key: space.sort_key(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SpaceArchivePolicySyncRecord {
    Manual,
    IdleDays { days: u16 },
}

impl From<&ArchivePolicy> for SpaceArchivePolicySyncRecord {
    fn from(policy: &ArchivePolicy) -> Self {
        match policy {
            ArchivePolicy::Manual => Self::Manual,
            ArchivePolicy::IdleDays(days) => Self::IdleDays { days: *days },
        }
    }
}

impl From<SpaceArchivePolicySyncRecord> for ArchivePolicy {
    fn from(policy: SpaceArchivePolicySyncRecord) -> Self {
        match policy {
            SpaceArchivePolicySyncRecord::Manual => Self::Manual,
            SpaceArchivePolicySyncRecord::IdleDays { days } => Self::IdleDays(days),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct BookmarkSyncRecord {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) profile_id: String,
    pub(crate) space_id: String,
    #[serde(default)]
    pub(crate) space_name: Option<String>,
    pub(crate) collection_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) note: Option<String>,
    #[serde(default)]
    pub(crate) thumbnail_key: Option<String>,
    pub(crate) added_at_secs: u64,
}

impl BookmarkSyncRecord {
    fn from_entry(entry: &BookmarkEntry, space_name: Option<String>) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            title: entry.title().to_string(),
            url: entry.url().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            collection_name: entry.collection_name().to_string(),
            tags: entry.tags().to_vec(),
            note: entry.note().map(str::to_string),
            thumbnail_key: entry.thumbnail_key().map(str::to_string),
            added_at_secs: system_time_secs(entry.added_at()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TabSyncRecord {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) profile_id: String,
    pub(crate) space_id: String,
    #[serde(default)]
    pub(crate) space_name: Option<String>,
    #[serde(default)]
    pub(crate) favicon_key: Option<String>,
    #[serde(default)]
    pub(crate) flags: TabFlags,
    pub(crate) sort_key: u64,
    #[serde(default = "default_sync_enabled")]
    pub(crate) sync_enabled: bool,
    pub(crate) zoom_percent: u16,
    pub(crate) created_at_secs: u64,
    pub(crate) last_active_at_secs: u64,
}

impl TabSyncRecord {
    fn from_entry(entry: &BrowserTab, space_name: Option<String>) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            title: entry.title().to_string(),
            url: entry.url().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            favicon_key: entry.favicon_key().map(str::to_string),
            flags: entry.flags().clone(),
            sort_key: entry.sort_key(),
            sync_enabled: entry.sync_enabled(),
            zoom_percent: entry.zoom_percent(),
            created_at_secs: system_time_secs(entry.created_at()),
            last_active_at_secs: system_time_secs(entry.last_active_at()),
        }
    }
}

fn default_sync_enabled() -> bool {
    true
}

fn system_time_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}
