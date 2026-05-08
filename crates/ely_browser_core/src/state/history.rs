use std::time::SystemTime;

use ely_domain::{BrowserTab, HistoryEntry, UrlText};

use crate::navigation::records_history;

use super::BrowserCore;

impl BrowserCore {
    pub(super) fn record_history_entry(&mut self, tab: &BrowserTab) {
        if !records_history(tab.url()) {
            return;
        }

        self.history_entries.push(HistoryEntry::new(
            tab.profile_id().clone(),
            tab.space_id().clone(),
            tab.title(),
            tab.url().clone(),
            SystemTime::now(),
        ));
    }

    pub(super) fn find_history_match(&self, query: &str) -> Option<UrlText> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return None;
        }

        self.history_entries
            .iter()
            .rev()
            .find(|entry| {
                entry.profile_id() == &self.active_profile_id
                    && entry.space_id() == &self.active_space_id
                    && history_entry_matches_query(entry, &normalized_query)
            })
            .map(|entry| entry.url().clone())
    }

    pub(super) fn visible_history(&self) -> Vec<HistoryEntry> {
        self.history_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .filter(|entry| entry.space_id() == &self.active_space_id)
            .cloned()
            .collect()
    }
}

fn history_entry_matches_query(entry: &HistoryEntry, normalized_query: &str) -> bool {
    entry.title().to_lowercase().contains(normalized_query)
        || entry.url().as_str().to_lowercase().contains(normalized_query)
        || entry.url().display_url().to_lowercase().contains(normalized_query)
}
