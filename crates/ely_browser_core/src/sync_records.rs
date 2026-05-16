use std::time::{SystemTime, UNIX_EPOCH};

use ely_domain::{
    ArchivePolicy, BookmarkEntry, BrowserTab, NoteEntry, NoteTarget, Profile, ProfileKind,
    ProfileSyncPolicy, ReadingListEntry, ReadingProgress, SitePermissionEntry, Space, TabFlags,
};
use serde::{Deserialize, Serialize};

use crate::state::BrowserCore;

pub(crate) const SNAPSHOT_SCHEMA_REV: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(crate) struct SyncSnapshotBody {
    pub(crate) schema_rev: u32,
    #[serde(default)]
    pub(crate) profiles: Vec<ProfileSyncRecord>,
    #[serde(default)]
    pub(crate) spaces: Vec<SpaceSyncRecord>,
    pub(crate) bookmarks: Vec<BookmarkSyncRecord>,
    #[serde(default)]
    pub(crate) tabs: Vec<TabSyncRecord>,
    #[serde(default)]
    pub(crate) notes: Vec<NoteSyncRecord>,
    #[serde(default)]
    pub(crate) reading_list: Vec<ReadingListSyncRecord>,
    #[serde(default)]
    pub(crate) site_permissions: Vec<SitePermissionSyncRecord>,
}

impl SyncSnapshotBody {
    pub(crate) fn from_core(core: &BrowserCore) -> Self {
        Self {
            schema_rev: SNAPSHOT_SCHEMA_REV,
            profiles: core
                .visible_profiles_for_sync()
                .into_iter()
                .map(ProfileSyncRecord::from_profile)
                .collect(),
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
            notes: core
                .visible_notes_for_sync()
                .into_iter()
                .map(|entry| {
                    NoteSyncRecord::from_entry(entry, core.sync_space_name_for(entry.space_id()))
                })
                .collect(),
            reading_list: core
                .visible_reading_list_for_sync()
                .into_iter()
                .map(|entry| {
                    ReadingListSyncRecord::from_entry(
                        entry,
                        core.sync_space_name_for(entry.space_id()),
                    )
                })
                .collect(),
            site_permissions: core
                .visible_site_permissions_for_sync()
                .into_iter()
                .map(SitePermissionSyncRecord::from_entry)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProfileSyncRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) color_hex: u32,
    pub(crate) kind: ProfileKindSyncRecord,
    pub(crate) sync_policy: ProfileSyncPolicySyncRecord,
}

impl ProfileSyncRecord {
    fn from_profile(profile: &Profile) -> Self {
        Self {
            id: profile.id().as_str().to_string(),
            name: profile.name().to_string(),
            color_hex: profile.color_hex(),
            kind: ProfileKindSyncRecord::from_profile_kind(profile.kind()),
            sync_policy: ProfileSyncPolicySyncRecord::from_profile_sync_policy(
                profile.sync_policy(),
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProfileKindSyncRecord {
    Standard,
    Private,
}

impl ProfileKindSyncRecord {
    fn from_profile_kind(kind: &ProfileKind) -> Self {
        match kind {
            ProfileKind::Standard => Self::Standard,
            ProfileKind::Private => Self::Private,
        }
    }
}

impl From<ProfileKindSyncRecord> for ProfileKind {
    fn from(kind: ProfileKindSyncRecord) -> Self {
        match kind {
            ProfileKindSyncRecord::Standard => Self::Standard,
            ProfileKindSyncRecord::Private => Self::Private,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProfileSyncPolicySyncRecord {
    Enabled,
    Paused,
}

impl ProfileSyncPolicySyncRecord {
    fn from_profile_sync_policy(policy: ProfileSyncPolicy) -> Self {
        match policy {
            ProfileSyncPolicy::Enabled => Self::Enabled,
            ProfileSyncPolicy::Paused => Self::Paused,
        }
    }
}

impl From<ProfileSyncPolicySyncRecord> for ProfileSyncPolicy {
    fn from(policy: ProfileSyncPolicySyncRecord) -> Self {
        match policy {
            ProfileSyncPolicySyncRecord::Enabled => Self::Enabled,
            ProfileSyncPolicySyncRecord::Paused => Self::Paused,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NoteSyncRecord {
    pub(crate) id: String,
    pub(crate) profile_id: String,
    pub(crate) space_id: String,
    #[serde(default)]
    pub(crate) space_name: Option<String>,
    pub(crate) target: NoteTargetSyncRecord,
    pub(crate) title: String,
    pub(crate) source_url: String,
    pub(crate) body: String,
    pub(crate) created_at_secs: u64,
    pub(crate) updated_at_secs: u64,
}

impl NoteSyncRecord {
    fn from_entry(entry: &NoteEntry, space_name: Option<String>) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            target: NoteTargetSyncRecord::from_note_target(entry.target()),
            title: entry.title().to_string(),
            source_url: entry.source_url().as_str().to_string(),
            body: entry.body().to_string(),
            created_at_secs: system_time_secs(entry.created_at()),
            updated_at_secs: system_time_secs(entry.updated_at()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum NoteTargetSyncRecord {
    Url { url: String },
    Tab { tab_id: String },
}

impl NoteTargetSyncRecord {
    fn from_note_target(target: &NoteTarget) -> Self {
        match target {
            NoteTarget::Url(url) => Self::Url { url: url.as_str().to_string() },
            NoteTarget::Tab(tab_id) => Self::Tab { tab_id: tab_id.as_str().to_string() },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ReadingListSyncRecord {
    pub(crate) id: String,
    pub(crate) profile_id: String,
    pub(crate) space_id: String,
    #[serde(default)]
    pub(crate) space_name: Option<String>,
    pub(crate) title: String,
    pub(crate) source_url: String,
    pub(crate) progress: ReadingProgressSyncRecord,
    pub(crate) added_at_secs: u64,
}

impl ReadingListSyncRecord {
    fn from_entry(entry: &ReadingListEntry, space_name: Option<String>) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            title: entry.title().to_string(),
            source_url: entry.source_url().as_str().to_string(),
            progress: ReadingProgressSyncRecord::from_progress(*entry.progress()),
            added_at_secs: system_time_secs(entry.added_at()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ReadingProgressSyncRecord {
    Unread,
    InProgress { percent: u8 },
    Finished,
}

impl ReadingProgressSyncRecord {
    fn from_progress(progress: ReadingProgress) -> Self {
        match progress {
            ReadingProgress::Unread => Self::Unread,
            ReadingProgress::InProgress(percent) => Self::InProgress { percent: percent.value() },
            ReadingProgress::Finished => Self::Finished,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SitePermissionSyncRecord {
    pub(crate) profile_id: String,
    pub(crate) origin: String,
    pub(crate) feature: String,
    pub(crate) decision: String,
}

impl SitePermissionSyncRecord {
    fn from_entry(entry: &SitePermissionEntry) -> Self {
        Self {
            profile_id: entry.profile_id().as_str().to_string(),
            origin: entry.origin().as_str().to_string(),
            feature: entry.feature().as_str().to_string(),
            decision: entry.decision().as_str().to_string(),
        }
    }
}

fn default_sync_enabled() -> bool {
    true
}

fn system_time_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}
