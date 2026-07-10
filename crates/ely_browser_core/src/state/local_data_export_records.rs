use std::time::{SystemTime, UNIX_EPOCH};

use ely_domain::{
    ArchiveSource, ArchivedTab, BookmarkEntry, BrowserTab, DownloadChecksum, DownloadDestination,
    DownloadEntry, DownloadSecurity, DownloadState, HistoryEntry, NoteEntry, NoteTarget, Profile,
    ProfileKind, ReadingListEntry, ReadingProgress, SitePermissionAuditAction,
    SitePermissionAuditEvent, SitePermissionEntry, TabFlags, TabState,
};
use serde::{Deserialize, Serialize};

use super::LocalDataInventory;
use crate::CoreError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElyLocalDataPackage {
    pub(super) version: u16,
    pub(super) exported_at_unix_seconds: u64,
    pub(super) profile: ElyLocalProfileRecord,
    pub(super) inventory: LocalDataInventory,
    pub(super) open_tabs: Vec<ElyLocalTabRecord>,
    pub(super) archived_tabs: Vec<ElyLocalArchivedTabRecord>,
    pub(super) bookmarks: Vec<ElyLocalBookmarkRecord>,
    pub(super) notes: Vec<ElyLocalNoteRecord>,
    pub(super) reading_list: Vec<ElyLocalReadingListRecord>,
    pub(super) history: Vec<ElyLocalHistoryRecord>,
    pub(super) downloads: Vec<ElyLocalDownloadRecord>,
    pub(super) site_permissions: Vec<ElyLocalSitePermissionRecord>,
    pub(super) site_permission_audit_events: Vec<ElyLocalSitePermissionAuditRecord>,
}

impl ElyLocalDataPackage {
    #[must_use]
    pub fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile.id
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile.name
    }

    #[must_use]
    pub fn inventory(&self) -> LocalDataInventory {
        self.inventory
    }

    pub(super) fn unix_seconds(time: SystemTime) -> Result<u64, CoreError> {
        time.duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .map_err(|error| CoreError::InvalidLocalDataPackage { reason: error.to_string() })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalProfileRecord {
    id: String,
    name: String,
    kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalTabRecord {
    id: String,
    space_id: String,
    space_name: String,
    title: String,
    url: String,
    favicon_key: Option<String>,
    parent_tab_id: Option<String>,
    state: String,
    flags: ElyLocalTabFlagsRecord,
    group_id: Option<String>,
    split_id: Option<String>,
    sort_key: u64,
    sync_enabled: bool,
    created_at_unix_seconds: u64,
    last_active_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ElyLocalTabFlagsRecord {
    pinned: bool,
    favorite: bool,
    muted: bool,
    unread: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalArchivedTabRecord {
    tab: ElyLocalTabRecord,
    archived_at_unix_seconds: u64,
    source: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalBookmarkRecord {
    id: String,
    space_id: String,
    space_name: String,
    collection_name: String,
    title: String,
    url: String,
    tags: Vec<String>,
    note: Option<String>,
    thumbnail_key: Option<String>,
    added_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalNoteRecord {
    id: String,
    space_id: String,
    space_name: String,
    target: ElyLocalNoteTargetRecord,
    title: String,
    source_url: String,
    body: String,
    created_at_unix_seconds: u64,
    updated_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ElyLocalNoteTargetRecord {
    Url { url: String },
    Tab { tab_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalReadingListRecord {
    id: String,
    space_id: String,
    space_name: String,
    title: String,
    source_url: String,
    progress: ElyLocalReadingProgressRecord,
    added_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum ElyLocalReadingProgressRecord {
    Unread,
    InProgress { percent: u8 },
    Finished,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalHistoryRecord {
    space_id: String,
    space_name: String,
    source_tab_id: String,
    title: String,
    url: String,
    favicon_key: Option<String>,
    visited_at_unix_seconds: u64,
    visit_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalDownloadRecord {
    id: String,
    source_url: String,
    file_name: String,
    destination: ElyLocalDownloadDestinationRecord,
    target_file_path: Option<String>,
    security: String,
    state: String,
    received_bytes: u64,
    total_bytes: Option<u64>,
    checksum: Option<ElyLocalDownloadChecksumRecord>,
    security_prompt_confirmed: bool,
    started_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ElyLocalDownloadDestinationRecord {
    AskEveryTime,
    FixedDirectory { path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ElyLocalDownloadChecksumRecord {
    algorithm: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalSitePermissionRecord {
    origin: String,
    feature: String,
    decision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ElyLocalSitePermissionAuditRecord {
    origin: String,
    feature: String,
    action: ElyLocalSitePermissionAuditActionRecord,
    created_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ElyLocalSitePermissionAuditActionRecord {
    Set { decision: String },
    Revoked,
    Transferred,
    Consumed,
}

impl ElyLocalProfileRecord {
    pub(super) fn from_profile(profile: &Profile) -> Self {
        Self {
            id: profile.id().as_str().to_string(),
            name: profile.name().to_string(),
            kind: profile_kind(profile.kind()).to_string(),
        }
    }
}

impl ElyLocalTabRecord {
    pub(super) fn from_tab(tab: &BrowserTab, space_name: String) -> Result<Self, CoreError> {
        Ok(Self {
            id: tab.id().as_str().to_string(),
            space_id: tab.space_id().as_str().to_string(),
            space_name,
            title: tab.title().to_string(),
            url: tab.url().as_str().to_string(),
            favicon_key: tab.favicon_key().map(str::to_string),
            parent_tab_id: tab.parent_tab_id().map(|id| id.as_str().to_string()),
            state: tab_state(tab.state()).to_string(),
            flags: ElyLocalTabFlagsRecord::from_flags(tab.flags()),
            group_id: tab.group_id().map(|id| id.as_str().to_string()),
            split_id: tab.split_id().map(|id| id.as_str().to_string()),
            sort_key: tab.sort_key(),
            sync_enabled: tab.sync_enabled(),
            created_at_unix_seconds: ElyLocalDataPackage::unix_seconds(tab.created_at())?,
            last_active_at_unix_seconds: ElyLocalDataPackage::unix_seconds(tab.last_active_at())?,
        })
    }
}

impl ElyLocalTabFlagsRecord {
    fn from_flags(flags: &TabFlags) -> Self {
        Self {
            pinned: flags.pinned,
            favorite: flags.favorite,
            muted: flags.muted,
            unread: flags.unread,
        }
    }
}

impl ElyLocalArchivedTabRecord {
    pub(super) fn from_archived_tab(
        archived: &ArchivedTab,
        tab: ElyLocalTabRecord,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            tab,
            archived_at_unix_seconds: ElyLocalDataPackage::unix_seconds(archived.archived_at())?,
            source: archive_source(archived.source()).to_string(),
        })
    }
}

impl ElyLocalBookmarkRecord {
    pub(super) fn from_bookmark(
        bookmark: &BookmarkEntry,
        space_name: String,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            id: bookmark.id().as_str().to_string(),
            space_id: bookmark.space_id().as_str().to_string(),
            space_name,
            collection_name: bookmark.collection_name().to_string(),
            title: bookmark.title().to_string(),
            url: bookmark.url().as_str().to_string(),
            tags: bookmark.tags().to_vec(),
            note: bookmark.note().map(str::to_string),
            thumbnail_key: bookmark.thumbnail_key().map(str::to_string),
            added_at_unix_seconds: ElyLocalDataPackage::unix_seconds(bookmark.added_at())?,
        })
    }
}

impl ElyLocalNoteRecord {
    pub(super) fn from_note(note: &NoteEntry, space_name: String) -> Result<Self, CoreError> {
        Ok(Self {
            id: note.id().as_str().to_string(),
            space_id: note.space_id().as_str().to_string(),
            space_name,
            target: ElyLocalNoteTargetRecord::from_note_target(note.target()),
            title: note.title().to_string(),
            source_url: note.source_url().as_str().to_string(),
            body: note.body().to_string(),
            created_at_unix_seconds: ElyLocalDataPackage::unix_seconds(note.created_at())?,
            updated_at_unix_seconds: ElyLocalDataPackage::unix_seconds(note.updated_at())?,
        })
    }
}

impl ElyLocalReadingListRecord {
    pub(super) fn from_entry(
        entry: &ReadingListEntry,
        space_name: String,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            id: entry.id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            title: entry.title().to_string(),
            source_url: entry.source_url().as_str().to_string(),
            progress: ElyLocalReadingProgressRecord::from_progress(*entry.progress()),
            added_at_unix_seconds: ElyLocalDataPackage::unix_seconds(entry.added_at())?,
        })
    }
}

impl ElyLocalHistoryRecord {
    pub(super) fn from_entry(entry: &HistoryEntry, space_name: String) -> Result<Self, CoreError> {
        Ok(Self {
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            source_tab_id: entry.source_tab_id().as_str().to_string(),
            title: entry.title().to_string(),
            url: entry.url().as_str().to_string(),
            favicon_key: entry.favicon_key().map(str::to_string),
            visited_at_unix_seconds: ElyLocalDataPackage::unix_seconds(entry.visited_at())?,
            visit_count: entry.visit_count(),
        })
    }
}

impl ElyLocalDownloadRecord {
    pub(super) fn from_download(entry: &DownloadEntry) -> Result<Self, CoreError> {
        Ok(Self {
            id: entry.id().as_str().to_string(),
            source_url: entry.source_url().as_str().to_string(),
            file_name: entry.file_name().to_string(),
            destination: ElyLocalDownloadDestinationRecord::from_destination(entry.destination()),
            target_file_path: entry.target_file_path().map(|path| path.display().to_string()),
            security: download_security(entry.security()).to_string(),
            state: download_state(entry.state()).to_string(),
            received_bytes: entry.received_bytes(),
            total_bytes: entry.total_bytes(),
            checksum: entry.checksum().map(ElyLocalDownloadChecksumRecord::from_checksum),
            security_prompt_confirmed: entry.security_prompt_confirmed(),
            started_at_unix_seconds: ElyLocalDataPackage::unix_seconds(entry.started_at())?,
        })
    }
}

impl ElyLocalDownloadDestinationRecord {
    fn from_destination(destination: &DownloadDestination) -> Self {
        match destination {
            DownloadDestination::AskEveryTime => Self::AskEveryTime,
            DownloadDestination::FixedDirectory(path) => {
                Self::FixedDirectory { path: path.display().to_string() }
            }
        }
    }
}

impl ElyLocalDownloadChecksumRecord {
    fn from_checksum(checksum: &DownloadChecksum) -> Self {
        Self {
            algorithm: checksum.algorithm().as_str().to_string(),
            value: checksum.value().to_string(),
        }
    }
}

impl ElyLocalSitePermissionRecord {
    pub(super) fn from_site_permission(entry: &SitePermissionEntry) -> Self {
        Self {
            origin: entry.origin().as_str().to_string(),
            feature: entry.feature().as_str().to_string(),
            decision: entry.decision().as_str().to_string(),
        }
    }
}

impl ElyLocalSitePermissionAuditRecord {
    pub(super) fn from_audit_event(event: &SitePermissionAuditEvent) -> Result<Self, CoreError> {
        Ok(Self {
            origin: event.origin().as_str().to_string(),
            feature: event.feature().as_str().to_string(),
            action: ElyLocalSitePermissionAuditActionRecord::from_action(event.action()),
            created_at_unix_seconds: ElyLocalDataPackage::unix_seconds(event.created_at())?,
        })
    }
}

impl ElyLocalNoteTargetRecord {
    fn from_note_target(target: &NoteTarget) -> Self {
        match target {
            NoteTarget::Url(url) => Self::Url { url: url.as_str().to_string() },
            NoteTarget::Tab(tab_id) => Self::Tab { tab_id: tab_id.as_str().to_string() },
        }
    }
}

impl ElyLocalReadingProgressRecord {
    fn from_progress(progress: ReadingProgress) -> Self {
        match progress {
            ReadingProgress::Unread => Self::Unread,
            ReadingProgress::InProgress(percent) => Self::InProgress { percent: percent.value() },
            ReadingProgress::Finished => Self::Finished,
        }
    }
}

impl ElyLocalSitePermissionAuditActionRecord {
    fn from_action(action: &SitePermissionAuditAction) -> Self {
        match action {
            SitePermissionAuditAction::Set(decision) => {
                Self::Set { decision: decision.as_str().to_string() }
            }
            SitePermissionAuditAction::Revoked => Self::Revoked,
            SitePermissionAuditAction::Transferred => Self::Transferred,
            SitePermissionAuditAction::Consumed => Self::Consumed,
        }
    }
}

fn profile_kind(kind: &ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Standard => "standard",
        ProfileKind::Private => "private",
    }
}

fn tab_state(state: &TabState) -> &'static str {
    match state {
        TabState::Loading => "loading",
        TabState::Ready => "ready",
        TabState::Crashed => "crashed",
        TabState::Discarded => "discarded",
        TabState::Archived => "archived",
    }
}

fn archive_source(source: &ArchiveSource) -> &'static str {
    match source {
        ArchiveSource::ManualClose => "manual_close",
        ArchiveSource::AutoArchive => "auto_archive",
    }
}

fn download_security(security: &DownloadSecurity) -> &'static str {
    match security {
        DownloadSecurity::Standard => "standard",
        DownloadSecurity::DangerousExtension => "dangerous_extension",
    }
}

fn download_state(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::InProgress => "in_progress",
        DownloadState::Paused => "paused",
        DownloadState::Completed => "completed",
        DownloadState::Cancelled => "cancelled",
        DownloadState::Failed => "failed",
    }
}
