use ely_domain::{ArchiveSource, ArchivedTab, BrowserTab, ProfileId, SpaceId, TabId, UrlText};

use super::BrowserCore;
use crate::{
    CoreError,
    navigation::{tab_matches_query, tab_title},
};

impl BrowserCore {
    pub fn open_new_tab(&mut self) -> Result<TabId, CoreError> {
        let url = self.new_tab_url()?;
        Ok(self.open_tab(url))
    }

    pub fn open_tab(&mut self, url: UrlText) -> TabId {
        let tab = self.build_tab(url);
        let tab_id = tab.id().clone();
        let space_id = tab.space_id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);
        self.record_history_entry(&tab);
        self.tabs.insert(insert_index, tab);
        self.normalize_tab_sort_keys(&space_id);
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

        self.detach_tab_from_split(&tab_id);
        let tab_index = self.active_tab_index()?;
        let target_sort_key = self.next_tab_sort_key(space_id);
        self.tabs[tab_index].move_to_space(space_id.clone());
        self.tabs[tab_index].clear_group_id();
        self.tabs[tab_index].set_sort_key(target_sort_key);
        self.sort_tabs_within_space(space_id);
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
                self.new_tab_url()?,
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
        if let Some(active_tab_id) = self.close_active_saved_split_view()? {
            return Ok(active_tab_id);
        }

        let tab_id = self.active_tab_id.clone();
        self.close_tab(&tab_id)
    }

    pub fn close_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        if let Some(split_id) = self.saved_split_id_for_tab(tab_id) {
            return self.close_saved_split_view(&split_id);
        }

        let close_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let was_active = &self.active_tab_id == tab_id;

        let mut closed_tab = self.tabs.remove(close_index);
        let closed_space_id = closed_tab.space_id().clone();
        let closed_profile_id = closed_tab.profile_id().clone();
        let was_space_active_tab = self.active_tabs_by_space.get(&closed_space_id) == Some(tab_id);
        closed_tab.clear_split_id();
        self.detach_tab_from_split(tab_id);
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
            self.new_tab_url()?,
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
        let sort_key = self.next_tab_sort_key(&space_id);
        BrowserTab::new(TabId::new(), space_id, profile_id, title, url).with_sort_key(sort_key)
    }

    pub(super) fn tab_belongs_to_space(&self, tab_id: &TabId, space_id: &SpaceId) -> bool {
        self.tabs.iter().any(|tab| tab.id() == tab_id && tab.space_id() == space_id)
    }

    pub(super) fn active_tab_index(&self) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    pub(super) fn active_tab_mut(&mut self) -> Result<&mut BrowserTab, CoreError> {
        let active_index = self.active_tab_index()?;
        self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)
    }

    pub(super) fn build_tab(&self, url: UrlText) -> BrowserTab {
        self.build_tab_for(self.active_space_id.clone(), self.active_profile_id.clone(), url)
            .with_parent_tab_id(self.active_tab_id.clone())
    }

    pub(super) fn nearest_tab_in_space(
        &self,
        space_id: &SpaceId,
        start_index: usize,
    ) -> Option<TabId> {
        self.tabs
            .iter()
            .skip(start_index)
            .chain(self.tabs.iter().take(start_index))
            .find(|tab| tab.space_id() == space_id)
            .map(|tab| tab.id().clone())
    }

    pub(super) fn nearest_tab_in_space_profile(
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
