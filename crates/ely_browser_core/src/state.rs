use std::{collections::BTreeMap, time::SystemTime};

use ely_domain::{
    ArchivePolicy, ArchivedTab, BookmarkEntry, BrowserTab, DomainError, DownloadEntry,
    DownloadPolicy, HistoryEntry, Profile, ProfileId, ProfileKind, ReadingListEntry,
    SitePermissionAuditEvent, SitePermissionEntry, Space, SpaceId, SplitLayout, SyncStatus, TabId,
    UrlText,
};

use crate::CoreError;

mod bookmarks;
mod commands;
mod downloads;
mod history;
mod plugins;
mod profiles;
mod reading_list;
mod site_permissions;
mod splits;
mod sync;
mod tabs;

pub use plugins::{InstalledPlugin, PluginAuditAction, PluginAuditEvent};

#[derive(Clone, Debug)]
pub struct InitialBrowserConfig {
    pub space_name: String,
    pub space_icon: String,
    pub profile_name: String,
    pub initial_url: UrlText,
}

impl InitialBrowserConfig {
    pub fn ely_defaults() -> Result<Self, DomainError> {
        Ok(Self {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            profile_name: "Default".to_string(),
            initial_url: UrlText::parse("ely://new-tab")?,
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
    pub reading_list: Vec<ReadingListEntry>,
    pub site_permissions: Vec<SitePermissionEntry>,
    pub site_permission_audit_events: Vec<SitePermissionAuditEvent>,
    pub download_entries: Vec<DownloadEntry>,
    pub history_entries: Vec<HistoryEntry>,
    pub split_layouts: Vec<SplitLayout>,
    pub installed_plugins: Vec<InstalledPlugin>,
    pub plugin_audit_events: Vec<PluginAuditEvent>,
    pub spaces: Vec<Space>,
    pub profiles: Vec<Profile>,
    pub sync_status: SyncStatus,
    pub active_tab_id: TabId,
    pub active_space_id: SpaceId,
    pub active_profile_id: ProfileId,
    pub active_space_name: String,
    pub active_profile_name: String,
    pub active_download_policy: DownloadPolicy,
    pub command_query: String,
}

#[derive(Debug)]
pub struct BrowserCore {
    spaces: Vec<Space>,
    profiles: Vec<Profile>,
    tabs: Vec<BrowserTab>,
    archived_tabs: Vec<ArchivedTab>,
    bookmarks: Vec<BookmarkEntry>,
    reading_list: Vec<ReadingListEntry>,
    site_permissions: Vec<SitePermissionEntry>,
    site_permission_audit_events: Vec<SitePermissionAuditEvent>,
    download_entries: Vec<DownloadEntry>,
    history_entries: Vec<HistoryEntry>,
    split_layouts: Vec<SplitLayout>,
    archived_split_layouts: Vec<SplitLayout>,
    installed_plugins: Vec<InstalledPlugin>,
    plugin_audit_events: Vec<PluginAuditEvent>,
    active_space_id: SpaceId,
    active_profile_id: ProfileId,
    active_tab_id: TabId,
    active_tabs_by_space: BTreeMap<SpaceId, TabId>,
    active_tabs_by_space_profile: BTreeMap<(SpaceId, ProfileId), TabId>,
    command_query: String,
    new_tab_url: UrlText,
}

impl BrowserCore {
    pub fn new(config: InitialBrowserConfig) -> Result<Self, CoreError> {
        let space = Space::new(config.space_name, config.space_icon, 0xf54e00);
        let profile = Profile::new(config.profile_name, 0x26251e, ProfileKind::Standard);
        let new_tab_url = config.initial_url;
        let active_space_id = space.id().clone();
        let active_profile_id = profile.id().clone();
        let tab = BrowserTab::new(
            TabId::new(),
            active_space_id.clone(),
            active_profile_id.clone(),
            "New Tab",
            new_tab_url.clone(),
        );
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
            spaces: vec![space],
            profiles: vec![profile],
            tabs: vec![tab],
            archived_tabs: Vec::new(),
            bookmarks: Vec::new(),
            reading_list: Vec::new(),
            site_permissions: Vec::new(),
            site_permission_audit_events: Vec::new(),
            download_entries: Vec::new(),
            history_entries: Vec::new(),
            split_layouts: Vec::new(),
            archived_split_layouts: Vec::new(),
            installed_plugins: Vec::new(),
            plugin_audit_events: Vec::new(),
            command_query: String::new(),
            new_tab_url,
        })
    }

    pub fn create_space(
        &mut self,
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
    ) -> Result<SpaceId, CoreError> {
        let space = Space::new(name, icon, accent_hex);
        let space_id = space.id().clone();
        let tab = self.build_tab_for(
            space_id.clone(),
            self.active_profile_id.clone(),
            self.new_tab_url.clone(),
        );
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

        let tab = self.build_tab_for(
            space_id.clone(),
            self.active_profile_id.clone(),
            self.new_tab_url.clone(),
        );
        let tab_id = tab.id().clone();
        self.tabs.push(tab);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
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
            reading_list: self.visible_reading_list(),
            site_permissions: self.visible_site_permissions(),
            site_permission_audit_events: self.visible_site_permission_audit_events(),
            download_entries: self.visible_downloads(),
            history_entries: self.visible_history(),
            split_layouts: self.visible_split_layouts(),
            installed_plugins: self.installed_plugins.clone(),
            plugin_audit_events: self.plugin_audit_events.clone(),
            spaces: self.spaces.clone(),
            profiles: self.profiles.clone(),
            sync_status: self.sync_status(),
            tabs: self.visible_tabs(),
            active_tab_id: self.active_tab_id.clone(),
            active_space_id: self.active_space_id.clone(),
            active_profile_id: self.active_profile_id.clone(),
            active_space_name: active_space.name().to_string(),
            active_profile_name: active_profile.name().to_string(),
            active_download_policy: active_profile.download_policy().clone(),
            command_query: self.command_query.clone(),
        })
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

    fn favorites(&self) -> Vec<BrowserTab> {
        self.tabs.iter().filter(|tab| tab.flags().favorite).cloned().collect()
    }

    fn pinned_tabs(&self) -> Vec<BrowserTab> {
        self.tabs
            .iter()
            .filter(|tab| tab.space_id() == &self.active_space_id)
            .filter(|tab| tab.flags().pinned && !tab.flags().favorite)
            .cloned()
            .collect()
    }

    fn visible_tabs(&self) -> Vec<BrowserTab> {
        self.tabs.iter().filter(|tab| tab.space_id() == &self.active_space_id).cloned().collect()
    }

    fn record_tab_activity(&mut self, tab_id: &TabId, active_at: SystemTime) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id() == tab_id) {
            tab.record_activity(active_at);
        }
    }
}
