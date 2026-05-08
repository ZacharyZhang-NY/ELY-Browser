use std::time::SystemTime;

use ely_domain::{ReadingListEntry, ReadingListId, ReadingProgress, UrlText};

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

    pub fn set_reading_list_progress(
        &mut self,
        entry_id: &ReadingListId,
        progress: ReadingProgress,
    ) -> Result<(), CoreError> {
        self.reading_list_entry_mut(entry_id)?.set_progress(progress);
        Ok(())
    }

    pub fn remove_reading_list_entry(&mut self, entry_id: &ReadingListId) -> Result<(), CoreError> {
        let index = self.reading_list_entry_index(entry_id)?;
        self.reading_list.remove(index);
        Ok(())
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

    fn reading_list_entry_mut(
        &mut self,
        entry_id: &ReadingListId,
    ) -> Result<&mut ReadingListEntry, CoreError> {
        let index = self.reading_list_entry_index(entry_id)?;
        Ok(&mut self.reading_list[index])
    }

    fn reading_list_entry_index(&self, entry_id: &ReadingListId) -> Result<usize, CoreError> {
        self.reading_list
            .iter()
            .position(|entry| entry.id() == entry_id)
            .ok_or_else(|| CoreError::ReadingListEntryNotFound { id: entry_id.clone() })
    }
}

fn reading_list_entry_matches_query(entry: &ReadingListEntry, normalized_query: &str) -> bool {
    entry.title().to_lowercase().contains(normalized_query)
        || entry.source_url().as_str().to_lowercase().contains(normalized_query)
        || entry.display_url().to_lowercase().contains(normalized_query)
}
