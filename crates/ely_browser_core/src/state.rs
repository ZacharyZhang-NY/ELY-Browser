use std::{collections::BTreeMap, time::SystemTime};

use ely_domain::{
    ArchivePolicy, ArchivedTab, BookmarkEntry, BrowserTab, DomainError, DownloadEntry,
    DownloadPolicy, FavoriteLimit, HistoryEntry, HistoryRecordingPolicy, NewTabDestination,
    NoteEntry, Profile, ProfileId, ProfileKind, ReadingListEntry, SearchEngine,
    SitePermissionAuditEvent, SitePermissionEntry, Space, SpaceId, SplitLayout, SyncStatus,
    TabGroup, TabId, UrlText,
};

use crate::{CoreError, navigation::tab_title};
use sync::SyncObjectPolicies;

mod bookmarks;
mod commands;
mod downloads;
mod history;
mod notes;
mod plugins;
mod profiles;
mod reading_list;
mod site_permissions;
mod spaces;
mod splits;
mod sync;
mod tab_group_order;
mod tab_groups;
mod tab_lifecycle;
mod tab_order;
mod tabs;

pub use plugins::{InstalledPlugin, PluginAuditAction, PluginAuditEvent};
pub use spaces::TrashedSpace;

#[derive(Clone, Debug)]
pub struct InitialBrowserConfig {
    pub space_name: String,
    pub space_icon: String,
    pub profile_name: String,
    pub new_tab_destination: NewTabDestination,
}

impl InitialBrowserConfig {
    pub fn ely_defaults() -> Result<Self, DomainError> {
        Ok(Self {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            profile_name: "Default".to_string(),
            new_tab_destination: NewTabDestination::default(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct BrowserSnapshot {
    pub tabs: Vec<BrowserTab>,
    pub favorites: Vec<BrowserTab>,
    pub pinned_tabs: Vec<BrowserTab>,
    pub archived_tabs: Vec<ArchivedTab>,
    pub bookmarks: Vec<BookmarkEntry>,
    pub notes: Vec<NoteEntry>,
    pub reading_list: Vec<ReadingListEntry>,
    pub site_permissions: Vec<SitePermissionEntry>,
    pub site_permission_audit_events: Vec<SitePermissionAuditEvent>,
    pub download_entries: Vec<DownloadEntry>,
    pub history_entries: Vec<HistoryEntry>,
    pub active_profile_history_entry_count: usize,
    pub tab_groups: Vec<TabGroup>,
    pub split_layouts: Vec<SplitLayout>,
    pub installed_plugins: Vec<InstalledPlugin>,
    pub plugin_audit_events: Vec<PluginAuditEvent>,
    pub spaces: Vec<Space>,
    pub trashed_spaces: Vec<TrashedSpace>,
    pub profiles: Vec<Profile>,
    pub sync_status: SyncStatus,
    pub active_tab_id: TabId,
    pub active_space_id: SpaceId,
    pub active_profile_id: ProfileId,
    pub active_space_name: String,
    pub active_profile_name: String,
    pub active_download_policy: DownloadPolicy,
    pub search_engine: SearchEngine,
    pub new_tab_destination: NewTabDestination,
    pub history_recording_policy: HistoryRecordingPolicy,
    pub favorite_limit: FavoriteLimit,
    pub command_query: String,
}

#[derive(Debug)]
pub struct BrowserCore {
    spaces: Vec<Space>,
    profiles: Vec<Profile>,
    tabs: Vec<BrowserTab>,
    archived_tabs: Vec<ArchivedTab>,
    bookmarks: Vec<BookmarkEntry>,
    notes: Vec<NoteEntry>,
    reading_list: Vec<ReadingListEntry>,
    site_permissions: Vec<SitePermissionEntry>,
    site_permission_audit_events: Vec<SitePermissionAuditEvent>,
    download_entries: Vec<DownloadEntry>,
    history_entries: Vec<HistoryEntry>,
    tab_groups: Vec<TabGroup>,
    split_layouts: Vec<SplitLayout>,
    archived_split_layouts: Vec<SplitLayout>,
    trashed_spaces: Vec<TrashedSpace>,
    installed_plugins: Vec<InstalledPlugin>,
    plugin_audit_events: Vec<PluginAuditEvent>,
    active_space_id: SpaceId,
    active_profile_id: ProfileId,
    active_tab_id: TabId,
    active_tabs_by_space: BTreeMap<SpaceId, TabId>,
    active_tabs_by_space_profile: BTreeMap<(SpaceId, ProfileId), TabId>,
    search_engine: SearchEngine,
    new_tab_destination: NewTabDestination,
    history_recording_policy: HistoryRecordingPolicy,
    favorite_limit: FavoriteLimit,
    sync_object_policies: SyncObjectPolicies,
    command_query: String,
}

impl BrowserCore {
    pub fn new(config: InitialBrowserConfig) -> Result<Self, CoreError> {
        let profile = Profile::new(config.profile_name, 0x26251e, ProfileKind::Standard);
        let active_profile_id = profile.id().clone();
        let space = Space::new(
            config.space_name,
            config.space_icon,
            0xf54e00,
            active_profile_id.clone(),
            0,
        );
        let new_tab_destination = config.new_tab_destination;
        let new_tab_url = new_tab_destination.url()?;
        let new_tab_title = tab_title(&new_tab_url);
        let active_space_id = space.id().clone();
        let tab = BrowserTab::new(
            TabId::new(),
            active_space_id.clone(),
            active_profile_id.clone(),
            new_tab_title,
            new_tab_url.clone(),
        )
        .with_sort_key(0);
        let active_tab_id = tab.id().clone();
        let mut active_tabs_by_space = BTreeMap::new();
        let mut active_tabs_by_space_profile = BTreeMap::new();
        active_tabs_by_space.insert(active_space_id.clone(), active_tab_id.clone());
        active_tabs_by_space_profile
            .insert((active_space_id.clone(), active_profile_id.clone()), active_tab_id.clone());

        Ok(Self {
            active_space_id,
            active_profile_id,
            active_tab_id,
            active_tabs_by_space,
            active_tabs_by_space_profile,
            search_engine: SearchEngine::default(),
            new_tab_destination,
            history_recording_policy: HistoryRecordingPolicy::default(),
            favorite_limit: FavoriteLimit::default(),
            sync_object_policies: SyncObjectPolicies::default(),
            spaces: vec![space],
            profiles: vec![profile],
            tabs: vec![tab],
            archived_tabs: Vec::new(),
            bookmarks: Vec::new(),
            notes: Vec::new(),
            reading_list: Vec::new(),
            site_permissions: Vec::new(),
            site_permission_audit_events: Vec::new(),
            download_entries: Vec::new(),
            history_entries: Vec::new(),
            tab_groups: Vec::new(),
            split_layouts: Vec::new(),
            archived_split_layouts: Vec::new(),
            trashed_spaces: Vec::new(),
            installed_plugins: Vec::new(),
            plugin_audit_events: Vec::new(),
            command_query: String::new(),
        })
    }

    pub fn create_space(
        &mut self,
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
    ) -> Result<SpaceId, CoreError> {
        let sort_key = self.next_space_sort_key();
        let space = Space::new(name, icon, accent_hex, self.active_profile_id.clone(), sort_key);
        let space_id = space.id().clone();
        let default_profile_id = space.default_profile_id().clone();
        let tab = self.build_tab_for(space_id.clone(), default_profile_id, self.new_tab_url()?);
        let tab_id = tab.id().clone();

        self.spaces.push(space);
        self.tabs.push(tab);
        self.active_tabs_by_space.insert(space_id.clone(), tab_id.clone());
        self.select_tab(&tab_id)?;
        Ok(space_id)
    }

    pub fn select_space(&mut self, space_id: &SpaceId) -> Result<TabId, CoreError> {
        if !self.spaces.iter().any(|space| space.id() == space_id) {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        }

        if let Some(tab_id) = self
            .active_tabs_by_space
            .get(space_id)
            .filter(|tab_id| self.tab_belongs_to_space(tab_id, space_id))
            .cloned()
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        if let Some(tab_id) =
            self.tabs.iter().find(|tab| tab.space_id() == space_id).map(|tab| tab.id().clone())
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        let default_profile_id = self
            .spaces
            .iter()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?
            .default_profile_id()
            .clone();
        let tab = self.build_tab_for(space_id.clone(), default_profile_id, self.new_tab_url()?);
        let tab_id = tab.id().clone();
        self.tabs.push(tab);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    pub fn select_next_space(&mut self) -> Result<TabId, CoreError> {
        let spaces = self.sorted_spaces();
        let Some(active_index) =
            spaces.iter().position(|space| space.id() == &self.active_space_id)
        else {
            return Err(CoreError::SpaceNotFound { id: self.active_space_id.clone() });
        };

        let next_index = (active_index + 1) % spaces.len();
        let space_id = spaces[next_index].id().clone();
        self.select_space(&space_id)
    }

    pub fn select_previous_space(&mut self) -> Result<TabId, CoreError> {
        let spaces = self.sorted_spaces();
        let Some(active_index) =
            spaces.iter().position(|space| space.id() == &self.active_space_id)
        else {
            return Err(CoreError::SpaceNotFound { id: self.active_space_id.clone() });
        };

        let previous_index = if active_index == 0 { spaces.len() - 1 } else { active_index - 1 };
        let space_id = spaces[previous_index].id().clone();
        self.select_space(&space_id)
    }

    pub fn set_active_space_archive_policy(
        &mut self,
        archive_policy: ArchivePolicy,
    ) -> Result<(), CoreError> {
        let active_space_id = self.active_space_id.clone();
        self.set_space_archive_policy(&active_space_id, archive_policy)
    }

    pub fn set_space_archive_policy(
        &mut self,
        space_id: &SpaceId,
        archive_policy: ArchivePolicy,
    ) -> Result<(), CoreError> {
        let space = self
            .spaces
            .iter_mut()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        space.set_archive_policy(archive_policy);
        Ok(())
    }

    pub fn set_space_default_profile(
        &mut self,
        space_id: &SpaceId,
        profile_id: &ProfileId,
    ) -> Result<(), CoreError> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id() == profile_id)
            .ok_or_else(|| CoreError::ProfileNotFound { id: profile_id.clone() })?;
        if profile.kind() == &ProfileKind::Private {
            return Err(CoreError::PrivateProfileDefaultLocked { id: profile_id.clone() });
        }

        let space = self
            .spaces
            .iter_mut()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        space.set_default_profile_id(profile_id.clone());
        Ok(())
    }

    pub fn set_active_space_default_profile(
        &mut self,
        profile_id: &ProfileId,
    ) -> Result<(), CoreError> {
        let active_space_id = self.active_space_id.clone();
        self.set_space_default_profile(&active_space_id, profile_id)
    }

    pub fn set_space_sidebar_width(
        &mut self,
        space_id: &SpaceId,
        sidebar_width_px: u16,
    ) -> Result<(), CoreError> {
        let space = self
            .spaces
            .iter_mut()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        space.set_sidebar_width_px(sidebar_width_px);
        Ok(())
    }

    pub fn set_space_sort_key(
        &mut self,
        space_id: &SpaceId,
        sort_key: u64,
    ) -> Result<(), CoreError> {
        let space = self
            .spaces
            .iter_mut()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        space.set_sort_key(sort_key);
        Ok(())
    }

    pub fn set_search_engine(&mut self, search_engine: SearchEngine) {
        self.search_engine = search_engine;
    }

    #[must_use]
    pub fn search_engine(&self) -> SearchEngine {
        self.search_engine
    }

    pub fn set_new_tab_destination(&mut self, destination: NewTabDestination) {
        self.new_tab_destination = destination;
    }

    #[must_use]
    pub fn new_tab_destination(&self) -> NewTabDestination {
        self.new_tab_destination
    }

    pub fn set_history_recording_policy(&mut self, policy: HistoryRecordingPolicy) {
        self.history_recording_policy = policy;
    }

    #[must_use]
    pub fn history_recording_policy(&self) -> HistoryRecordingPolicy {
        self.history_recording_policy
    }

    pub fn set_favorite_limit(&mut self, favorite_limit: FavoriteLimit) {
        self.favorite_limit = favorite_limit;
    }

    #[must_use]
    pub fn favorite_limit(&self) -> FavoriteLimit {
        self.favorite_limit
    }

    pub fn set_command_query(&mut self, query: impl Into<String>) {
        self.command_query = query.into();
    }

    #[must_use]
    pub fn command_query(&self) -> &str {
        &self.command_query
    }

    pub fn snapshot(&self) -> Result<BrowserSnapshot, CoreError> {
        let active_space = self.active_space()?;
        let active_profile = self.active_profile()?;

        Ok(BrowserSnapshot {
            favorites: self.favorites(),
            pinned_tabs: self.pinned_tabs(),
            archived_tabs: self.archived_tabs.clone(),
            bookmarks: self.visible_bookmarks(),
            notes: self.visible_notes(),
            reading_list: self.visible_reading_list(),
            site_permissions: self.visible_site_permissions(),
            site_permission_audit_events: self.visible_site_permission_audit_events(),
            download_entries: self.visible_downloads(),
            history_entries: self.visible_history(),
            active_profile_history_entry_count: self.active_profile_history_count(),
            tab_groups: self.visible_tab_groups(),
            split_layouts: self.visible_split_layouts(),
            installed_plugins: self.installed_plugins.clone(),
            plugin_audit_events: self.plugin_audit_events.clone(),
            spaces: self.sorted_spaces(),
            trashed_spaces: self.trashed_spaces.clone(),
            profiles: self.profiles.clone(),
            sync_status: self.sync_status(),
            tabs: self.visible_tabs(),
            active_tab_id: self.active_tab_id.clone(),
            active_space_id: self.active_space_id.clone(),
            active_profile_id: self.active_profile_id.clone(),
            active_space_name: active_space.name().to_string(),
            active_profile_name: active_profile.name().to_string(),
            active_download_policy: active_profile.download_policy().clone(),
            search_engine: self.search_engine,
            new_tab_destination: self.new_tab_destination,
            history_recording_policy: self.history_recording_policy,
            favorite_limit: self.favorite_limit,
            command_query: self.command_query.clone(),
        })
    }

    pub(super) fn new_tab_url(&self) -> Result<UrlText, CoreError> {
        self.new_tab_destination.url().map_err(CoreError::from)
    }

    fn active_profile(&self) -> Result<&Profile, CoreError> {
        self.profiles
            .iter()
            .find(|profile| profile.id() == &self.active_profile_id)
            .ok_or_else(|| CoreError::ProfileNotFound { id: self.active_profile_id.clone() })
    }

    fn active_space(&self) -> Result<&Space, CoreError> {
        self.spaces
            .iter()
            .find(|space| space.id() == &self.active_space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: self.active_space_id.clone() })
    }

    fn next_space_sort_key(&self) -> u64 {
        self.spaces
            .iter()
            .map(Space::sort_key)
            .max()
            .map_or(0, |sort_key| sort_key.saturating_add(1))
    }

    fn sorted_spaces(&self) -> Vec<Space> {
        let mut spaces = self.spaces.clone();
        spaces.sort_by(|left, right| {
            left.sort_key().cmp(&right.sort_key()).then_with(|| left.id().cmp(right.id()))
        });
        spaces
    }

    fn favorites(&self) -> Vec<BrowserTab> {
        self.tabs.iter().filter(|tab| tab.flags().favorite).cloned().collect()
    }

    fn pinned_tabs(&self) -> Vec<BrowserTab> {
        tab_order::sorted_tabs(
            self.tabs
                .iter()
                .filter(|tab| tab.space_id() == &self.active_space_id)
                .filter(|tab| tab.flags().pinned && !tab.flags().favorite),
        )
    }

    fn visible_tabs(&self) -> Vec<BrowserTab> {
        tab_order::sorted_tabs(
            self.tabs.iter().filter(|tab| tab.space_id() == &self.active_space_id),
        )
    }

    fn record_tab_activity(&mut self, tab_id: &TabId, active_at: SystemTime) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id() == tab_id) {
            tab.record_activity(active_at);
        }
    }
}
