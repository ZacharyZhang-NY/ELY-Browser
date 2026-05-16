use std::time::{Duration, UNIX_EPOCH};

use ely_domain::{
    ArchivePolicy, BookmarkEntry, BookmarkId, BrowserTab, ProfileId, Space, SpaceId,
    SyncConnectionState, SyncObjectKind, SyncObjectPolicy, SyncObjectState, SyncObjectStatus,
    SyncStatus, TabId, UrlText,
};
use ely_sync_client::SyncClientError;

use super::{BrowserCore, sync_context::SyncSnapshotApplyContext};
use crate::sync_engine::SyncSnapshotApplySummary;
use crate::sync_records::{BookmarkSyncRecord, SpaceSyncRecord, TabSyncRecord};

#[derive(Clone, Debug)]
pub(super) struct SyncObjectPolicies {
    spaces: SyncObjectPolicy,
    tabs: SyncObjectPolicy,
    bookmarks: SyncObjectPolicy,
    notes: SyncObjectPolicy,
    reading_list: SyncObjectPolicy,
    profiles: SyncObjectPolicy,
    site_permissions: SyncObjectPolicy,
    history: SyncObjectPolicy,
    plugin_settings: SyncObjectPolicy,
}

impl Default for SyncObjectPolicies {
    fn default() -> Self {
        Self {
            spaces: SyncObjectPolicy::Enabled,
            tabs: SyncObjectPolicy::Enabled,
            bookmarks: SyncObjectPolicy::Enabled,
            notes: SyncObjectPolicy::Enabled,
            reading_list: SyncObjectPolicy::Enabled,
            profiles: SyncObjectPolicy::Enabled,
            site_permissions: SyncObjectPolicy::Enabled,
            history: SyncObjectPolicy::Enabled,
            plugin_settings: SyncObjectPolicy::Enabled,
        }
    }
}

impl SyncObjectPolicies {
    fn get(&self, kind: SyncObjectKind) -> SyncObjectPolicy {
        match kind {
            SyncObjectKind::Spaces => self.spaces,
            SyncObjectKind::Tabs => self.tabs,
            SyncObjectKind::Bookmarks => self.bookmarks,
            SyncObjectKind::Notes => self.notes,
            SyncObjectKind::ReadingList => self.reading_list,
            SyncObjectKind::Profiles => self.profiles,
            SyncObjectKind::SitePermissions => self.site_permissions,
            SyncObjectKind::History => self.history,
            SyncObjectKind::PluginSettings => self.plugin_settings,
        }
    }

    fn set(&mut self, kind: SyncObjectKind, policy: SyncObjectPolicy) {
        match kind {
            SyncObjectKind::Spaces => self.spaces = policy,
            SyncObjectKind::Tabs => self.tabs = policy,
            SyncObjectKind::Bookmarks => self.bookmarks = policy,
            SyncObjectKind::Notes => self.notes = policy,
            SyncObjectKind::ReadingList => self.reading_list = policy,
            SyncObjectKind::Profiles => self.profiles = policy,
            SyncObjectKind::SitePermissions => self.site_permissions = policy,
            SyncObjectKind::History => self.history = policy,
            SyncObjectKind::PluginSettings => self.plugin_settings = policy,
        }
    }
}

impl BrowserCore {
    pub fn set_sync_object_policy(&mut self, kind: SyncObjectKind, policy: SyncObjectPolicy) {
        self.sync_object_policies.set(kind, policy);
    }

    pub fn reset_sync_settings(&mut self) {
        self.sync_object_policies = SyncObjectPolicies::default();
    }

    #[must_use]
    pub fn sync_object_policy(&self, kind: SyncObjectKind) -> SyncObjectPolicy {
        self.sync_object_policies.get(kind)
    }

    pub fn set_sync_connection_state(&mut self, state: SyncConnectionState) {
        self.sync_connection_state = state;
    }

    #[must_use]
    pub fn cloud_sync_upload_enabled(&self) -> bool {
        matches!(
            self.sync_connection_state,
            SyncConnectionState::SignedIn
                | SyncConnectionState::AwaitingDeviceApproval
                | SyncConnectionState::SyncReady { .. }
                | SyncConnectionState::SyncError { .. }
        )
    }

    pub(crate) fn sync_space_name_for(&self, space_id: &SpaceId) -> Option<String> {
        self.spaces
            .iter()
            .find(|space| space.id() == space_id)
            .map(|space| space.name().to_string())
    }

    pub(super) fn sync_status(&self) -> SyncStatus {
        let enabled_state = match &self.sync_connection_state {
            SyncConnectionState::SyncReady { .. } => SyncObjectState::Synced,
            _ => SyncObjectState::LocalOnly,
        };
        SyncStatus::new(
            self.sync_connection_state.clone(),
            vec![
                self.sync_object_status(SyncObjectKind::Spaces, self.spaces.len(), enabled_state),
                self.sync_object_status(
                    SyncObjectKind::Tabs,
                    self.sync_enabled_tab_count(),
                    enabled_state,
                ),
                self.sync_object_status(
                    SyncObjectKind::Bookmarks,
                    self.bookmarks.len(),
                    enabled_state,
                ),
                self.sync_object_status(SyncObjectKind::Notes, self.notes.len(), enabled_state),
                self.sync_object_status(
                    SyncObjectKind::ReadingList,
                    self.reading_list.len(),
                    enabled_state,
                ),
                self.sync_object_status(
                    SyncObjectKind::Profiles,
                    self.profiles.len(),
                    enabled_state,
                ),
                self.sync_object_status(
                    SyncObjectKind::SitePermissions,
                    self.site_permissions.len(),
                    enabled_state,
                ),
                self.sync_object_status(
                    SyncObjectKind::History,
                    self.history_entries.len(),
                    SyncObjectState::PrivacyControlled,
                ),
                self.sync_object_status(
                    SyncObjectKind::PluginSettings,
                    self.installed_plugins.len(),
                    enabled_state,
                ),
            ],
        )
    }

    fn sync_object_status(
        &self,
        kind: SyncObjectKind,
        local_count: usize,
        enabled_state: SyncObjectState,
    ) -> SyncObjectStatus {
        let policy = self.sync_object_policies.get(kind);
        let state = match policy {
            SyncObjectPolicy::Enabled => enabled_state,
            SyncObjectPolicy::Paused => SyncObjectState::Paused,
        };

        SyncObjectStatus::with_policy(kind, local_count, state, policy)
    }

    fn sync_enabled_tab_count(&self) -> usize {
        self.tabs.iter().filter(|tab| tab.sync_enabled()).count()
    }

    pub(crate) fn visible_tabs_for_sync(&self) -> Vec<&BrowserTab> {
        if self.sync_object_policy(SyncObjectKind::Tabs) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.tabs
            .iter()
            .filter(|tab| tab.sync_enabled() && self.profile_allows_cloud_sync(tab.profile_id()))
            .collect()
    }

    pub(crate) fn visible_spaces_for_sync(&self) -> Vec<&Space> {
        if self.sync_object_policy(SyncObjectKind::Spaces) == SyncObjectPolicy::Paused {
            return Vec::new();
        }
        self.spaces.iter().collect()
    }

    pub(super) fn apply_space_sync_record(
        &mut self,
        record: SpaceSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let space_id = parse_space_id(&record.id)?;
        let default_profile_id = self.sync_profile_id(&record.default_profile_id, context)?;
        let archive_policy = ArchivePolicy::from(record.archive_policy.clone());
        let existing_index =
            self.spaces.iter().position(|space| space.id() == &space_id).or_else(|| {
                self.spaces
                    .iter()
                    .position(|space| space.name().eq_ignore_ascii_case(record.name.trim()))
            });

        match existing_index {
            Some(index) => {
                if self.update_space_from_sync_record(
                    index,
                    record,
                    default_profile_id,
                    archive_policy,
                ) {
                    summary.record_updated();
                }
            }
            None => {
                let mut space = Space::new(
                    record.name,
                    record.icon,
                    record.accent_hex,
                    default_profile_id,
                    record.sort_key,
                );
                space.set_archive_policy(archive_policy);
                space.set_sidebar_width_px(record.sidebar_width_px);
                self.spaces.push(space);
                summary.record_imported();
            }
        }
        Ok(())
    }

    fn update_space_from_sync_record(
        &mut self,
        index: usize,
        record: SpaceSyncRecord,
        default_profile_id: ProfileId,
        archive_policy: ArchivePolicy,
    ) -> bool {
        let space = &mut self.spaces[index];
        let mut changed = false;
        if space.name() != record.name
            || space.icon() != record.icon
            || space.accent_hex() != record.accent_hex
        {
            space.set_presentation(record.name, record.icon, record.accent_hex);
            changed = true;
        }
        if space.default_profile_id() != &default_profile_id {
            space.set_default_profile_id(default_profile_id);
            changed = true;
        }
        if space.archive_policy() != &archive_policy {
            space.set_archive_policy(archive_policy);
            changed = true;
        }
        if space.sidebar_width_px() != record.sidebar_width_px {
            space.set_sidebar_width_px(record.sidebar_width_px);
            changed = true;
        }
        if space.sort_key() != record.sort_key {
            space.set_sort_key(record.sort_key);
            changed = true;
        }
        changed
    }

    pub(super) fn apply_tab_sync_record(
        &mut self,
        record: TabSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let tab_id = parse_tab_id(&record.id)?;
        let profile_id = self.sync_profile_id(&record.profile_id, context)?;
        let space_id = self.sync_space_id(&record.space_id, record.space_name.as_deref())?;
        let url = UrlText::parse(&record.url).map_err(snapshot_schema_error)?;
        let created_at = UNIX_EPOCH + Duration::from_secs(record.created_at_secs);
        let last_active_at = UNIX_EPOCH + Duration::from_secs(record.last_active_at_secs);
        let existing_index = self.tabs.iter().position(|tab| tab.id() == &tab_id).or_else(|| {
            self.tabs.iter().position(|tab| {
                tab.profile_id() == &profile_id && tab.space_id() == &space_id && tab.url() == &url
            })
        });
        let id = existing_index
            .and_then(|index| self.tabs.get(index).map(|tab| tab.id().clone()))
            .unwrap_or(tab_id);
        let mut tab =
            BrowserTab::new(id.clone(), space_id.clone(), profile_id.clone(), record.title, url)
                .with_sort_key(record.sort_key);
        tab.restore_activity_timestamps(created_at, last_active_at);
        tab.set_flags(record.flags);
        tab.set_sync_enabled(record.sync_enabled);
        tab.set_zoom_percent(record.zoom_percent).map_err(snapshot_schema_error)?;
        if let Some(favicon_key) = record.favicon_key {
            tab.set_favicon_key(favicon_key).map_err(snapshot_schema_error)?;
        }

        match existing_index {
            Some(index) if self.tabs[index] == tab => summary.record_skipped(),
            Some(index) => {
                self.tabs[index] = tab;
                self.sort_tabs_within_space(&space_id);
                self.ensure_synced_tab_indexes(&id, &space_id, &profile_id);
                summary.record_updated();
            }
            None => {
                self.tabs.push(tab);
                self.sort_tabs_within_space(&space_id);
                self.ensure_synced_tab_indexes(&id, &space_id, &profile_id);
                summary.record_imported();
            }
        }
        Ok(())
    }

    pub(super) fn apply_bookmark_sync_record(
        &mut self,
        record: BookmarkSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
        context: &SyncSnapshotApplyContext,
    ) -> Result<(), SyncClientError> {
        let bookmark_id = parse_bookmark_id(&record.id)?;
        let profile_id = self.sync_profile_id(&record.profile_id, context)?;
        let space_id = self.sync_space_id(&record.space_id, record.space_name.as_deref())?;
        let url = UrlText::parse(&record.url).map_err(snapshot_schema_error)?;
        let added_at = UNIX_EPOCH + Duration::from_secs(record.added_at_secs);
        let existing_index =
            self.bookmarks.iter().position(|bookmark| bookmark.id() == &bookmark_id).or_else(
                || {
                    self.bookmarks.iter().position(|bookmark| {
                        bookmark.profile_id() == &profile_id
                            && bookmark.space_id() == &space_id
                            && bookmark.url() == &url
                    })
                },
            );

        let id = existing_index
            .and_then(|index| self.bookmarks.get(index).map(|bookmark| bookmark.id().clone()))
            .unwrap_or(bookmark_id);
        let mut bookmark = BookmarkEntry::restore(
            id,
            profile_id,
            space_id,
            record.collection_name,
            record.title,
            url,
            added_at,
        )
        .map_err(snapshot_schema_error)?;
        bookmark.set_tags(record.tags).map_err(snapshot_schema_error)?;
        if let Some(note) = record.note {
            bookmark.set_note(note).map_err(snapshot_schema_error)?;
        }
        if let Some(thumbnail_key) = record.thumbnail_key {
            bookmark.set_thumbnail_key(thumbnail_key).map_err(snapshot_schema_error)?;
        }

        match existing_index {
            Some(index) if self.bookmarks[index] == bookmark => summary.record_skipped(),
            Some(index) => {
                self.bookmarks[index] = bookmark;
                summary.record_updated();
            }
            None => {
                self.bookmarks.push(bookmark);
                summary.record_imported();
            }
        }
        Ok(())
    }

    pub(super) fn sync_profile_id(
        &self,
        raw: &str,
        context: &SyncSnapshotApplyContext,
    ) -> Result<ProfileId, SyncClientError> {
        let profile_id = ProfileId::parse(raw).map_err(snapshot_schema_error)?;
        if let Some(local_profile_id) = context.profile_alias(&profile_id) {
            return Ok(local_profile_id);
        }
        if self.profiles.iter().any(|profile| profile.id() == &profile_id) {
            return Ok(profile_id);
        }
        Ok(self.active_profile_id.clone())
    }

    pub(super) fn profile_allows_cloud_sync(&self, profile_id: &ProfileId) -> bool {
        self.profiles.iter().any(|profile| {
            profile.id() == profile_id
                && profile.allows_sync()
                && profile.sync_policy() == ely_domain::ProfileSyncPolicy::Enabled
        })
    }

    pub(super) fn sync_space_id(
        &self,
        raw: &str,
        space_name: Option<&str>,
    ) -> Result<SpaceId, SyncClientError> {
        let space_id = SpaceId::parse(raw).map_err(snapshot_schema_error)?;
        if self.spaces.iter().any(|space| space.id() == &space_id) {
            return Ok(space_id);
        }
        if let Some(space_name) = space_name
            && let Some(space) =
                self.spaces.iter().find(|space| space.name().eq_ignore_ascii_case(space_name))
        {
            return Ok(space.id().clone());
        }
        Ok(self.active_space_id.clone())
    }

    fn ensure_synced_tab_indexes(
        &mut self,
        tab_id: &TabId,
        space_id: &SpaceId,
        profile_id: &ProfileId,
    ) {
        if self
            .active_tabs_by_space
            .get(space_id)
            .is_none_or(|mapped_id| !self.tab_belongs_to_space(mapped_id, space_id))
        {
            self.active_tabs_by_space.insert(space_id.clone(), tab_id.clone());
        }
        let key = (space_id.clone(), profile_id.clone());
        if self.active_tabs_by_space_profile.get(&key).is_none_or(|mapped_id| {
            !self.tabs.iter().any(|tab| {
                tab.id() == mapped_id
                    && tab.space_id() == space_id
                    && tab.profile_id() == profile_id
            })
        }) {
            self.active_tabs_by_space_profile.insert(key, tab_id.clone());
        }
    }
}

fn parse_bookmark_id(raw: &str) -> Result<BookmarkId, SyncClientError> {
    BookmarkId::parse(raw).map_err(snapshot_schema_error)
}

fn parse_tab_id(raw: &str) -> Result<TabId, SyncClientError> {
    TabId::parse(raw).map_err(snapshot_schema_error)
}

fn parse_space_id(raw: &str) -> Result<SpaceId, SyncClientError> {
    SpaceId::parse(raw).map_err(snapshot_schema_error)
}

pub(super) fn snapshot_schema_error(error: impl ToString) -> SyncClientError {
    SyncClientError::SnapshotSchema(error.to_string())
}
