use ely_domain::{ArchivedTab, BrowserTab, TabId};

use super::BrowserCore;
use crate::{CoreError, navigation::tab_matches_query};

impl BrowserCore {
    pub fn restore_last_archived_tab(&mut self) -> Result<TabId, CoreError> {
        let index = self.archived_tabs.len().checked_sub(1).ok_or(CoreError::NoArchivedTabs)?;
        self.restore_archived_tab_at_index(index)
    }

    pub fn restore_archived_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let index = self
            .archived_tabs
            .iter()
            .position(|archived| archived.tab().id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        self.restore_archived_tab_at_index(index)
    }

    pub fn restore_archived_tab_match(&mut self, query: &str) -> Result<Option<TabId>, CoreError> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return Ok(None);
        }

        let Some(index) = self
            .archived_tabs
            .iter()
            .rposition(|archived| self.archived_tab_matches_query(archived, &normalized_query))
        else {
            return Ok(None);
        };

        self.restore_archived_tab_at_index(index).map(Some)
    }

    fn restore_tab(&mut self, tab: BrowserTab) -> Result<TabId, CoreError> {
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);

        let space_id = tab.space_id().clone();
        self.tabs.insert(insert_index, tab);
        self.normalize_tab_sort_keys(&space_id);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    fn restore_archived_tab_at_index(&mut self, index: usize) -> Result<TabId, CoreError> {
        let tab_id = self.archived_tabs[index].tab().id().clone();
        if let Some(split_id) = self.archived_split_id_for_tab(&tab_id) {
            return self.restore_archived_split(&split_id, &tab_id);
        }

        let archived_tab = self.archived_tabs.remove(index);
        self.restore_tab(archived_tab.into_tab())
    }

    fn archived_tab_matches_query(&self, archived: &ArchivedTab, normalized_query: &str) -> bool {
        let tab = archived.tab();
        tab_matches_query(tab, normalized_query)
            || self.archived_tab_space_matches_query(tab, normalized_query)
            || self.archived_tab_profile_matches_query(tab, normalized_query)
    }

    fn archived_tab_space_matches_query(&self, tab: &BrowserTab, normalized_query: &str) -> bool {
        self.spaces.iter().find(|space| space.id() == tab.space_id()).is_some_and(|space| {
            space.name().to_lowercase().contains(normalized_query)
                || space.icon().to_lowercase().contains(normalized_query)
                || space.id().as_str().to_lowercase().contains(normalized_query)
        })
    }

    fn archived_tab_profile_matches_query(&self, tab: &BrowserTab, normalized_query: &str) -> bool {
        self.profiles.iter().find(|profile| profile.id() == tab.profile_id()).is_some_and(
            |profile| {
                profile.name().to_lowercase().contains(normalized_query)
                    || profile.id().as_str().to_lowercase().contains(normalized_query)
            },
        )
    }
}
