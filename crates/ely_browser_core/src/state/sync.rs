use std::time::{Duration, UNIX_EPOCH};

use ely_domain::{
    BookmarkEntry, BookmarkId, ProfileId, SpaceId, SyncConnectionState, SyncObjectKind,
    SyncObjectPolicy, SyncObjectState, SyncObjectStatus, SyncStatus, UrlText,
};
use ely_sync_client::SyncClientError;

use super::BrowserCore;
use crate::sync_engine::{BookmarkSyncRecord, SyncSnapshotApplySummary, SyncSnapshotBody};

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

    pub(crate) fn sync_space_name_for(&self, space_id: &SpaceId) -> Option<String> {
        self.spaces
            .iter()
            .find(|space| space.id() == space_id)
            .map(|space| space.name().to_string())
    }

    pub(crate) fn apply_sync_snapshot_body(
        &mut self,
        body: SyncSnapshotBody,
    ) -> Result<SyncSnapshotApplySummary, SyncClientError> {
        let mut summary = SyncSnapshotApplySummary::default();
        for record in body.bookmarks {
            self.apply_bookmark_sync_record(record, &mut summary)?;
        }
        Ok(summary)
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

    fn apply_bookmark_sync_record(
        &mut self,
        record: BookmarkSyncRecord,
        summary: &mut SyncSnapshotApplySummary,
    ) -> Result<(), SyncClientError> {
        let bookmark_id = parse_bookmark_id(&record.id)?;
        let profile_id = self.sync_profile_id(&record.profile_id)?;
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

    fn sync_profile_id(&self, raw: &str) -> Result<ProfileId, SyncClientError> {
        let profile_id = ProfileId::parse(raw).map_err(snapshot_schema_error)?;
        if self.profiles.iter().any(|profile| profile.id() == &profile_id) {
            return Ok(profile_id);
        }
        Ok(self.active_profile_id.clone())
    }

    fn sync_space_id(
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
}

fn parse_bookmark_id(raw: &str) -> Result<BookmarkId, SyncClientError> {
    BookmarkId::parse(raw).map_err(snapshot_schema_error)
}

fn snapshot_schema_error(error: impl ToString) -> SyncClientError {
    SyncClientError::SnapshotSchema(error.to_string())
}
