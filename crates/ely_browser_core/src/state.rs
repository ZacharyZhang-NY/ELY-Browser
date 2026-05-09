use std::{collections::BTreeMap, time::SystemTime};

use ely_domain::{
    ArchivePolicy, ArchivedTab, BookmarkEntry, BrowserTab, DEFAULT_SIDEBAR_WIDTH_PX,
    DiagnosticEvent, DiagnosticsReportingPolicy, DomainError, DownloadEntry, DownloadPolicy,
    FavoriteLimit, HistoryEntry, HistoryRecordingPolicy, NewTabDestination, NoteEntry, Profile,
    ProfileId, ProfileKind, ReadingListEntry, SearchEngine, SitePermissionAuditEvent,
    SitePermissionEntry, Space, SpaceId, SplitLayout, SyncStatus, TabGroup, TabId, UpdatePolicy,
    UrlText,
};

use crate::{CoreError, navigation::tab_title};
use sync::SyncObjectPolicies;

mod bookmarks;
mod commands;
mod diagnostics;
mod downloads;
mod history;
mod local_data_export_records;
mod local_data_exports;
mod notes;
mod plugins;
mod privacy;
mod profiles;
mod reading_list;
mod site_data;
mod site_permissions;
mod space_exports;
mod spaces;
mod splits;
mod sync;
mod tab_group_order;
mod tab_groups;
mod tab_lifecycle;
mod tab_order;
mod tab_selection;
mod tabs;

pub use bookmarks::{
    BookmarkImportSummary, ELYBOOKMARKS_FILE_EXTENSION, ELYBOOKMARKS_SCHEMA_VERSION,
    ElyBookmarksPackage,
};
pub use local_data_exports::{ELYDATA_FILE_EXTENSION, ELYDATA_SCHEMA_VERSION, ElyLocalDataPackage};
pub use plugins::{InstalledPlugin, PluginAuditAction, PluginAuditEvent};
pub use privacy::LocalDataInventory;
pub use site_data::SiteDataClearance;
pub use space_exports::{
    ELYSPACE_FILE_EXTENSION, ELYSPACE_SCHEMA_VERSION, ElySpacePackage, SpaceImportProfileMapping,
};
pub use spaces::TrashedSpace;

#[derive(Clone, Debug)]
pub struct InitialBrowserConfig {
    pub space_name: String,
    pub space_icon: String,
    pub space_color_hex: u32,
    pub profile_name: String,
    pub profile_color_hex: u32,
    pub profile_kind: ProfileKind,
    pub new_tab_destination: NewTabDestination,
}

impl InitialBrowserConfig {
    pub fn ely_defaults() -> Result<Self, DomainError> {
        Ok(Self {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            space_color_hex: 0xf54e00,
            profile_name: "Default".to_string(),
            profile_color_hex: 0x26251e,
            profile_kind: ProfileKind::Standard,
            new_tab_destination: NewTabDestination::default(),
        })
    }

    pub fn private_window() -> Result<Self, DomainError> {
        Ok(Self {
            space_name: "Private".to_string(),
            space_icon: "P".to_string(),
            space_color_hex: 0x807d72,
            profile_name: "Private".to_string(),
            profile_color_hex: 0x807d72,
            profile_kind: ProfileKind::Private,
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
    pub local_data_inventory: LocalDataInventory,
    pub tab_groups: Vec<TabGroup>,
    pub split_layouts: Vec<SplitLayout>,
    pub installed_plugins: Vec<InstalledPlugin>,
    pub plugin_audit_events: Vec<PluginAuditEvent>,
    pub diagnostic_events: Vec<DiagnosticEvent>,
    pub spaces: Vec<Space>,
    pub trashed_spaces: Vec<TrashedSpace>,
    pub profiles: Vec<Profile>,
    pub sync_status: SyncStatus,
    pub active_tab_id: TabId,
    pub active_space_id: SpaceId,
    pub active_profile_id: ProfileId,
    pub active_space_name: String,
    pub active_profile_name: String,
    pub active_profile_kind: ProfileKind,
    pub active_download_policy: DownloadPolicy,
    pub search_engine: SearchEngine,
    pub new_tab_destination: NewTabDestination,
    pub history_recording_policy: HistoryRecordingPolicy,
    pub diagnostics_reporting_policy: DiagnosticsReportingPolicy,
    pub favorite_limit: FavoriteLimit,
    pub update_policy: UpdatePolicy,
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
    diagnostic_events: Vec<DiagnosticEvent>,
    active_space_id: SpaceId,
    active_profile_id: ProfileId,
    active_tab_id: TabId,
    active_tabs_by_space: BTreeMap<SpaceId, TabId>,
    active_tabs_by_space_profile: BTreeMap<(SpaceId, ProfileId), TabId>,
    search_engine: SearchEngine,
    new_tab_destination: NewTabDestination,
    history_recording_policy: HistoryRecordingPolicy,
    diagnostics_reporting_policy: DiagnosticsReportingPolicy,
    favorite_limit: FavoriteLimit,
    update_policy: UpdatePolicy,
    sync_object_policies: SyncObjectPolicies,
    command_query: String,
}

impl BrowserCore {
    pub fn new(config: InitialBrowserConfig) -> Result<Self, CoreError> {
        let profile =
            Profile::new(config.profile_name, config.profile_color_hex, config.profile_kind);
        let active_profile_id = profile.id().clone();
        let space = Space::new(
            config.space_name,
            config.space_icon,
            config.space_color_hex,
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
            diagnostics_reporting_policy: DiagnosticsReportingPolicy::default(),
            favorite_limit: FavoriteLimit::default(),
            update_policy: UpdatePolicy::default(),
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
            diagnostic_events: vec![DiagnosticEvent::startup_success(SystemTime::now())],
            command_query: String::new(),
        })
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

    pub fn reset_search_settings(&mut self) {
        self.set_search_engine(SearchEngine::default());
    }

    #[must_use]
    pub fn search_engine(&self) -> SearchEngine {
        self.search_engine
    }

    pub fn set_new_tab_destination(&mut self, destination: NewTabDestination) {
        self.new_tab_destination = destination;
    }

    pub fn reset_general_settings(&mut self) {
        self.set_new_tab_destination(NewTabDestination::default());
    }

    #[must_use]
    pub fn new_tab_destination(&self) -> NewTabDestination {
        self.new_tab_destination
    }

    pub fn set_favorite_limit(&mut self, favorite_limit: FavoriteLimit) {
        self.favorite_limit = favorite_limit;
    }

    pub fn reset_sidebar_tabs_settings(&mut self) -> Result<(), CoreError> {
        let active_space_id = self.active_space_id.clone();
        self.set_space_archive_policy(&active_space_id, ArchivePolicy::Manual)?;
        self.set_space_sidebar_width(&active_space_id, DEFAULT_SIDEBAR_WIDTH_PX)?;
        self.set_favorite_limit(FavoriteLimit::default());
        Ok(())
    }

    #[must_use]
    pub fn favorite_limit(&self) -> FavoriteLimit {
        self.favorite_limit
    }

    pub fn set_update_policy(&mut self, update_policy: UpdatePolicy) {
        self.update_policy = update_policy;
    }

    pub fn reset_update_settings(&mut self) {
        self.set_update_policy(UpdatePolicy::default());
    }

    #[must_use]
    pub fn update_policy(&self) -> UpdatePolicy {
        self.update_policy
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
            local_data_inventory: self.active_profile_local_data_inventory(),
            tab_groups: self.visible_tab_groups(),
            split_layouts: self.visible_split_layouts(),
            installed_plugins: self.installed_plugins.clone(),
            plugin_audit_events: self.plugin_audit_events.clone(),
            diagnostic_events: self.diagnostic_events.clone(),
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
            active_profile_kind: active_profile.kind().clone(),
            active_download_policy: active_profile.download_policy().clone(),
            search_engine: self.search_engine,
            new_tab_destination: self.new_tab_destination,
            history_recording_policy: self.history_recording_policy,
            diagnostics_reporting_policy: self.diagnostics_reporting_policy,
            favorite_limit: self.favorite_limit,
            update_policy: self.update_policy,
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
