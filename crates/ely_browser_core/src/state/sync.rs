use ely_domain::{SyncObjectKind, SyncObjectState, SyncObjectStatus, SyncStatus};

use super::BrowserCore;

impl BrowserCore {
    pub(super) fn sync_status(&self) -> SyncStatus {
        SyncStatus::signed_out(vec![
            SyncObjectStatus::new(
                SyncObjectKind::Spaces,
                self.spaces.len(),
                SyncObjectState::LocalOnly,
            ),
            SyncObjectStatus::new(
                SyncObjectKind::Tabs,
                self.tabs.len(),
                SyncObjectState::LocalOnly,
            ),
            SyncObjectStatus::new(
                SyncObjectKind::Profiles,
                self.profiles.len(),
                SyncObjectState::LocalOnly,
            ),
            SyncObjectStatus::new(
                SyncObjectKind::History,
                self.history_entries.len(),
                SyncObjectState::PrivacyControlled,
            ),
            SyncObjectStatus::new(
                SyncObjectKind::PluginSettings,
                self.installed_plugins.len(),
                SyncObjectState::LocalOnly,
            ),
        ])
    }
}
