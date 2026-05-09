use ely_domain::{DiagnosticsReportingPolicy, HistoryRecordingPolicy};

use super::BrowserCore;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalDataInventory {
    open_tabs: usize,
    archived_tabs: usize,
    bookmarks: usize,
    notes: usize,
    reading_list: usize,
    history_entries: usize,
    site_permissions: usize,
    site_permission_audit_events: usize,
    downloads: usize,
}

impl LocalDataInventory {
    #[must_use]
    pub fn open_tabs(self) -> usize {
        self.open_tabs
    }

    #[must_use]
    pub fn archived_tabs(self) -> usize {
        self.archived_tabs
    }

    #[must_use]
    pub fn bookmarks(self) -> usize {
        self.bookmarks
    }

    #[must_use]
    pub fn notes(self) -> usize {
        self.notes
    }

    #[must_use]
    pub fn reading_list(self) -> usize {
        self.reading_list
    }

    #[must_use]
    pub fn history_entries(self) -> usize {
        self.history_entries
    }

    #[must_use]
    pub fn site_permissions(self) -> usize {
        self.site_permissions
    }

    #[must_use]
    pub fn site_permission_audit_events(self) -> usize {
        self.site_permission_audit_events
    }

    #[must_use]
    pub fn downloads(self) -> usize {
        self.downloads
    }

    #[must_use]
    pub fn total_items(self) -> usize {
        self.open_tabs
            + self.archived_tabs
            + self.bookmarks
            + self.notes
            + self.reading_list
            + self.history_entries
            + self.site_permissions
            + self.site_permission_audit_events
            + self.downloads
    }
}

impl BrowserCore {
    pub fn set_history_recording_policy(&mut self, policy: HistoryRecordingPolicy) {
        self.history_recording_policy = policy;
    }

    pub fn set_diagnostics_reporting_policy(&mut self, policy: DiagnosticsReportingPolicy) {
        self.diagnostics_reporting_policy = policy;
    }

    pub fn reset_privacy_settings(&mut self) {
        self.set_history_recording_policy(HistoryRecordingPolicy::default());
        self.set_diagnostics_reporting_policy(DiagnosticsReportingPolicy::default());
    }

    #[must_use]
    pub fn history_recording_policy(&self) -> HistoryRecordingPolicy {
        self.history_recording_policy
    }

    #[must_use]
    pub fn diagnostics_reporting_policy(&self) -> DiagnosticsReportingPolicy {
        self.diagnostics_reporting_policy
    }

    #[must_use]
    pub fn active_profile_local_data_inventory(&self) -> LocalDataInventory {
        let profile_id = &self.active_profile_id;

        LocalDataInventory {
            open_tabs: self.tabs.iter().filter(|tab| tab.profile_id() == profile_id).count(),
            archived_tabs: self
                .archived_tabs
                .iter()
                .filter(|archived| archived.tab().profile_id() == profile_id)
                .count(),
            bookmarks: self
                .bookmarks
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .count(),
            notes: self.notes.iter().filter(|entry| entry.profile_id() == profile_id).count(),
            reading_list: self
                .reading_list
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .count(),
            history_entries: self
                .history_entries
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .count(),
            site_permissions: self
                .site_permissions
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .count(),
            site_permission_audit_events: self
                .site_permission_audit_events
                .iter()
                .filter(|event| event.profile_id() == profile_id)
                .count(),
            downloads: self
                .download_entries
                .iter()
                .filter(|entry| entry.profile_id() == profile_id)
                .count(),
        }
    }
}
