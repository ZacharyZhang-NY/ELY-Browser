use std::time::SystemTime;

use ely_domain::{ReadingListEntry, ReadingListId, UrlText};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn save_active_tab_to_reading_list(&mut self) -> Result<ReadingListId, CoreError> {
        let active_tab = self.active_tab()?.clone();
        if let Some(entry) = self.reading_list.iter().find(|entry| {
            entry.profile_id() == active_tab.profile_id() && entry.source_url() == active_tab.url()
        }) {
            return Ok(entry.id().clone());
        }

        let entry = ReadingListEntry::new(
            active_tab.profile_id().clone(),
            active_tab.space_id().clone(),
            active_tab.title(),
            active_tab.url().clone(),
            SystemTime::now(),
        )?;
        let entry_id = entry.id().clone();
        self.reading_list.push(entry);
        Ok(entry_id)
    }

    pub(super) fn find_reading_list_match(&self, query: &str) -> Option<UrlText> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return None;
        }

        self.reading_list
            .iter()
            .rev()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .find(|entry| reading_list_entry_matches_query(entry, &normalized_query))
            .map(|entry| entry.source_url().clone())
    }

    pub(super) fn visible_reading_list(&self) -> Vec<ReadingListEntry> {
        self.reading_list
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }
}

fn reading_list_entry_matches_query(entry: &ReadingListEntry, normalized_query: &str) -> bool {
    entry.title().to_lowercase().contains(normalized_query)
        || entry.source_url().as_str().to_lowercase().contains(normalized_query)
        || entry.display_url().to_lowercase().contains(normalized_query)
}
