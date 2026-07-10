//! Local persistence sees more than cloud sync: pausing cloud sync or
//! disabling a tab's sync must never reduce what survives a restart.
//! Only Private-profile data stays out of the on-disk state.

use ely_domain::{
    BookmarkEntry, BrowserTab, HistoryEntry, NoteEntry, Profile, ProfileId, ProfileKind,
    ReadingListEntry, SitePermissionEntry, Space,
};

use super::BrowserCore;
use crate::state::InstalledPlugin;

impl BrowserCore {
    fn profile_persists_locally(&self, profile_id: &ProfileId) -> bool {
        self.profiles
            .iter()
            .any(|profile| profile.id() == profile_id && profile.kind() != &ProfileKind::Private)
    }

    pub(crate) fn visible_profiles_for_local(&self) -> Vec<&Profile> {
        self.profiles.iter().filter(|profile| profile.kind() != &ProfileKind::Private).collect()
    }

    pub(crate) fn visible_spaces_for_local(&self) -> Vec<&Space> {
        self.spaces.iter().collect()
    }

    pub(crate) fn visible_tabs_for_local(&self) -> Vec<&BrowserTab> {
        self.tabs.iter().filter(|tab| self.profile_persists_locally(tab.profile_id())).collect()
    }

    pub(crate) fn visible_bookmarks_for_local(&self) -> Vec<&BookmarkEntry> {
        self.bookmarks
            .iter()
            .filter(|entry| self.profile_persists_locally(entry.profile_id()))
            .collect()
    }

    pub(crate) fn visible_notes_for_local(&self) -> Vec<&NoteEntry> {
        self.notes
            .iter()
            .filter(|entry| self.profile_persists_locally(entry.profile_id()))
            .collect()
    }

    pub(crate) fn visible_reading_list_for_local(&self) -> Vec<&ReadingListEntry> {
        self.reading_list
            .iter()
            .filter(|entry| self.profile_persists_locally(entry.profile_id()))
            .collect()
    }

    pub(crate) fn visible_site_permissions_for_local(&self) -> Vec<&SitePermissionEntry> {
        self.site_permissions
            .iter()
            .filter(|entry| self.profile_persists_locally(entry.profile_id()))
            .collect()
    }

    pub(crate) fn visible_history_for_local(&self) -> Vec<&HistoryEntry> {
        self.history_entries
            .iter()
            .filter(|entry| self.profile_persists_locally(entry.profile_id()))
            .collect()
    }

    pub(crate) fn visible_plugin_settings_for_local(&self) -> Vec<&InstalledPlugin> {
        self.installed_plugins.iter().collect()
    }
}
