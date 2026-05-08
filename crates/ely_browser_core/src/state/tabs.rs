use ely_domain::{ArchiveSource, ArchivedTab, BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use crate::{
    CoreError,
    navigation::{tab_matches_query, tab_title},
};

use super::BrowserCore;

const DEFAULT_FAVORITE_LIMIT: usize = 12;

impl BrowserCore {
    pub fn open_tab(&mut self, url: UrlText) -> TabId {
        let tab = self.build_tab(url);
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);
        self.tabs.insert(insert_index, tab);
        self.active_tab_id = tab_id.clone();
        self.active_tabs_by_space.insert(self.active_space_id.clone(), tab_id.clone());
        self.active_tabs_by_space_profile
            .insert((self.active_space_id.clone(), self.active_profile_id.clone()), tab_id.clone());
        tab_id
    }

    pub fn move_active_tab_to_space(&mut self, space_id: &SpaceId) -> Result<TabId, CoreError> {
        if !self.spaces.iter().any(|space| space.id() == space_id) {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        }

        let tab_index = self.active_tab_index()?;
        let tab_id = self.active_tab_id.clone();
        let source_space_id = self.tabs[tab_index].space_id().clone();
        let profile_id = self.tabs[tab_index].profile_id().clone();
        if &source_space_id == space_id {
            return Ok(tab_id);
        }

        self.tabs[tab_index].move_to_space(space_id.clone());
        self.active_tabs_by_space.insert(space_id.clone(), tab_id.clone());
        self.active_tabs_by_space_profile.remove(&(source_space_id.clone(), profile_id.clone()));

        if let Some(next_tab_id) = self.nearest_tab_in_space(&source_space_id, tab_index) {
            self.active_tabs_by_space.insert(source_space_id.clone(), next_tab_id);
            if let Some(next_profile_tab_id) =
                self.nearest_tab_in_space_profile(&source_space_id, &profile_id, tab_index)
            {
                self.active_tabs_by_space_profile
                    .insert((source_space_id, profile_id), next_profile_tab_id);
            }
        } else {
            let tab = self.build_tab_for(
                source_space_id.clone(),
                self.active_profile_id.clone(),
                self.new_tab_url.clone(),
            );
            let replacement_id = tab.id().clone();
            self.tabs.insert(tab_index, tab);
            self.active_tabs_by_space_profile
                .insert((source_space_id.clone(), profile_id), replacement_id.clone());
            self.active_tabs_by_space.insert(source_space_id, replacement_id);
        }

        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    pub fn close_active_tab(&mut self) -> Result<TabId, CoreError> {
        let tab_id = self.active_tab_id.clone();
        self.close_tab(&tab_id)
    }

    pub fn close_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let close_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let was_active = &self.active_tab_id == tab_id;

        let closed_tab = self.tabs.remove(close_index);
        let closed_space_id = closed_tab.space_id().clone();
        let closed_profile_id = closed_tab.profile_id().clone();
        let was_space_active_tab = self.active_tabs_by_space.get(&closed_space_id) == Some(tab_id);
        self.active_tabs_by_space_profile
            .remove(&(closed_space_id.clone(), closed_profile_id.clone()));
        self.archived_tabs.push(ArchivedTab::new(closed_tab, ArchiveSource::ManualClose));

        if let Some(next_tab_id) = self.nearest_tab_in_space(&closed_space_id, close_index) {
            if was_space_active_tab {
                self.active_tabs_by_space.insert(closed_space_id.clone(), next_tab_id.clone());
            }
            if let Some(next_profile_tab_id) =
                self.nearest_tab_in_space_profile(&closed_space_id, &closed_profile_id, close_index)
            {
                self.active_tabs_by_space_profile
                    .insert((closed_space_id, closed_profile_id), next_profile_tab_id);
            }
            if was_active {
                self.select_tab(&next_tab_id)?;
            }
            return Ok(self.active_tab_id.clone());
        }

        let tab = self.build_tab_for(
            closed_space_id.clone(),
            closed_profile_id.clone(),
            self.new_tab_url.clone(),
        );
        let replacement_id = tab.id().clone();
        self.tabs.insert(close_index.min(self.tabs.len()), tab);
        self.active_tabs_by_space_profile
            .insert((closed_space_id.clone(), closed_profile_id), replacement_id.clone());
        self.active_tabs_by_space.insert(closed_space_id, replacement_id.clone());

        if was_active {
            self.select_tab(&replacement_id)?;
            return Ok(replacement_id);
        }

        Ok(self.active_tab_id.clone())
    }

    pub fn restore_last_archived_tab(&mut self) -> Result<TabId, CoreError> {
        let archived_tab = self.archived_tabs.pop().ok_or(CoreError::NoArchivedTabs)?;
        let tab = archived_tab.into_tab();
        self.restore_tab(tab)
    }

    pub fn restore_archived_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let index = self
            .archived_tabs
            .iter()
            .position(|archived| archived.tab().id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let archived_tab = self.archived_tabs.remove(index);
        let tab = archived_tab.into_tab();
        self.restore_tab(tab)
    }

    pub fn restore_archived_tab_match(&mut self, query: &str) -> Result<Option<TabId>, CoreError> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return Ok(None);
        }

        let Some(index) = self
            .archived_tabs
            .iter()
            .rposition(|archived| tab_matches_query(archived.tab(), &normalized_query))
        else {
            return Ok(None);
        };

        let archived_tab = self.archived_tabs.remove(index);
        let tab = archived_tab.into_tab();
        self.restore_tab(tab).map(Some)
    }

    pub fn select_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let active_space_id = tab.space_id().clone();
        let active_profile_id = tab.profile_id().clone();
        let active_tab_id = tab.id().clone();

        self.active_tab_id = active_tab_id.clone();
        self.active_space_id = active_space_id.clone();
        self.active_profile_id = active_profile_id.clone();
        self.active_tabs_by_space_profile
            .insert((active_space_id.clone(), active_profile_id), active_tab_id.clone());
        self.active_tabs_by_space.insert(active_space_id, active_tab_id);
        Ok(())
    }

    pub fn select_next_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(1)
    }

    pub fn select_previous_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(-1)
    }

    pub fn toggle_active_tab_favorite(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let favorite_count = self.tabs.iter().filter(|tab| tab.flags().favorite).count();
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_favorite = !active_tab.flags().favorite;

        if next_favorite && favorite_count >= DEFAULT_FAVORITE_LIMIT {
            return Err(CoreError::FavoriteLimitReached { limit: DEFAULT_FAVORITE_LIMIT });
        }

        active_tab.set_favorite(next_favorite);
        Ok(next_favorite)
    }

    pub fn toggle_active_tab_pinned(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_pinned = !active_tab.flags().pinned;
        active_tab.set_pinned(next_pinned);
        Ok(next_pinned)
    }

    pub fn active_tab(&self) -> Result<&BrowserTab, CoreError> {
        self.tabs
            .iter()
            .find(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    pub(super) fn find_tab_match(&self, query: &str) -> Option<TabId> {
        let normalized_query = query.trim().to_lowercase();
        self.tabs
            .iter()
            .find(|tab| tab_matches_query(tab, &normalized_query))
            .map(|tab| tab.id().clone())
    }

    pub(super) fn build_tab_for(
        &self,
        space_id: SpaceId,
        profile_id: ProfileId,
        url: UrlText,
    ) -> BrowserTab {
        let title = tab_title(&url);
        BrowserTab::new(TabId::new(), space_id, profile_id, title, url)
    }

    pub(super) fn tab_belongs_to_space(&self, tab_id: &TabId, space_id: &SpaceId) -> bool {
        self.tabs.iter().any(|tab| tab.id() == tab_id && tab.space_id() == space_id)
    }

    fn restore_tab(&mut self, tab: BrowserTab) -> Result<TabId, CoreError> {
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);

        self.tabs.insert(insert_index, tab);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    fn select_tab_by_offset(&mut self, offset: isize) -> Result<TabId, CoreError> {
        let visible_tab_ids = self
            .tabs
            .iter()
            .filter(|tab| tab.space_id() == &self.active_space_id)
            .map(|tab| tab.id().clone())
            .collect::<Vec<_>>();
        let active_index = visible_tab_ids
            .iter()
            .position(|tab_id| tab_id == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)?;
        let tab_count = visible_tab_ids.len() as isize;
        let next_index = (active_index as isize + offset).rem_euclid(tab_count) as usize;
        let next_tab_id = visible_tab_ids[next_index].clone();
        self.select_tab(&next_tab_id)?;
        Ok(next_tab_id)
    }

    fn active_tab_index(&self) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    fn build_tab(&self, url: UrlText) -> BrowserTab {
        self.build_tab_for(self.active_space_id.clone(), self.active_profile_id.clone(), url)
    }

    fn nearest_tab_in_space(&self, space_id: &SpaceId, start_index: usize) -> Option<TabId> {
        self.tabs
            .iter()
            .skip(start_index)
            .chain(self.tabs.iter().take(start_index))
            .find(|tab| tab.space_id() == space_id)
            .map(|tab| tab.id().clone())
    }

    fn nearest_tab_in_space_profile(
        &self,
        space_id: &SpaceId,
        profile_id: &ProfileId,
        start_index: usize,
    ) -> Option<TabId> {
        self.tabs
            .iter()
            .skip(start_index)
            .chain(self.tabs.iter().take(start_index))
            .find(|tab| tab.space_id() == space_id && tab.profile_id() == profile_id)
            .map(|tab| tab.id().clone())
    }
}
