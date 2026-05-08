use ely_domain::{SyncObjectKind, SyncObjectPolicy, SyncObjectState, SyncObjectStatus, SyncStatus};

use super::BrowserCore;

#[derive(Clone, Debug)]
pub(super) struct SyncObjectPolicies {
    spaces: SyncObjectPolicy,
    tabs: SyncObjectPolicy,
    bookmarks: SyncObjectPolicy,
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

    #[must_use]
    pub fn sync_object_policy(&self, kind: SyncObjectKind) -> SyncObjectPolicy {
        self.sync_object_policies.get(kind)
    }

    pub(super) fn sync_status(&self) -> SyncStatus {
        SyncStatus::signed_out(vec![
            self.sync_object_status(
                SyncObjectKind::Spaces,
                self.spaces.len(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::Tabs,
                self.sync_enabled_tab_count(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::Bookmarks,
                self.bookmarks.len(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::ReadingList,
                self.reading_list.len(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::Profiles,
                self.profiles.len(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::SitePermissions,
                self.site_permissions.len(),
                SyncObjectState::LocalOnly,
            ),
            self.sync_object_status(
                SyncObjectKind::History,
                self.history_entries.len(),
                SyncObjectState::PrivacyControlled,
            ),
            self.sync_object_status(
                SyncObjectKind::PluginSettings,
                self.installed_plugins.len(),
                SyncObjectState::LocalOnly,
            ),
        ])
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
}
