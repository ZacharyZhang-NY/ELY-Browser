use std::time::SystemTime;

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

    /// Navigate the active tab to `url` in place. Used for clicks on
    /// settings sub-pages, the home anchor, the bottom Settings row —
    /// places where the user expects the current tab to follow the
    /// link instead of accumulating a new tab for every step.
    ///
    /// Records a fresh history entry and resets activity timestamp;
    /// no new tab is created and the active tab id is unchanged.
    pub fn navigate_active_tab(&mut self, url: UrlText) -> Result<(), CoreError> {
        let active_id = self.active_tab_id.clone();
        self.update_tab_url(&active_id, url, TabUrlUpdate::PushHistory)?;
        Ok(())
    }

    pub fn navigate_tab_to_loaded_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
    ) -> Result<bool, CoreError> {
        self.update_tab_url(tab_id, url, TabUrlUpdate::PushHistory)
    }

    pub fn replace_tab_loaded_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
    ) -> Result<bool, CoreError> {
        self.update_tab_url(tab_id, url, TabUrlUpdate::PreserveHistory)
    }

    pub fn navigate_active_tab_back(&mut self) -> Result<bool, CoreError> {
        self.navigate_active_tab_history(TabHistoryDirection::Back)
    }

    pub fn navigate_active_tab_forward(&mut self) -> Result<bool, CoreError> {
        self.navigate_active_tab_history(TabHistoryDirection::Forward)
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

    pub fn toggle_active_tab_favorite(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let favorite_count = self.tabs.iter().filter(|tab| tab.flags().favorite).count();
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_favorite = !active_tab.flags().favorite;
        let favorite_limit = self.favorite_limit.value();

        if next_favorite && favorite_count >= favorite_limit {
            return Err(CoreError::FavoriteLimitReached { limit: favorite_limit });
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

    pub fn set_active_tab_zoom_percent(&mut self, zoom_percent: u16) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.set_zoom_percent(zoom_percent)?;
        Ok(active_tab.zoom_percent())
    }

    pub fn zoom_active_tab_in(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.zoom_in();
        Ok(active_tab.zoom_percent())
    }

    pub fn zoom_active_tab_out(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.zoom_out();
        Ok(active_tab.zoom_percent())
    }

    pub fn reset_active_tab_zoom(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.reset_zoom();
        Ok(active_tab.zoom_percent())
    }

    pub fn set_tab_sync_enabled(
        &mut self,
        tab_id: &TabId,
        sync_enabled: bool,
    ) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        tab.set_sync_enabled(sync_enabled);
        Ok(())
    }

    pub fn set_tab_sort_key(&mut self, tab_id: &TabId, sort_key: u64) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let space_id = tab.space_id().clone();
        tab.set_sort_key(sort_key);
        self.sort_tabs_within_space(&space_id);
        Ok(())
    }

    pub fn set_tab_favicon_key(
        &mut self,
        tab_id: &TabId,
        favicon_key: impl Into<String>,
    ) -> Result<bool, CoreError> {
        let favicon_key = favicon_key.into();
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if self.tabs[tab_index].favicon_key() == Some(favicon_key.as_str()) {
            return Ok(false);
        }
        self.tabs[tab_index].set_favicon_key(favicon_key.clone())?;
        let tab = self.tabs[tab_index].clone();
        self.set_history_favicon_key_for_tab(&tab, favicon_key);
        Ok(true)
    }

    /// Replace `tab_id`'s title with the live page title and mirror
    /// the new title into the history entry that recorded the visit.
    /// Returns `Ok(true)` only when the title actually changed —
    /// callers can use this to suppress redundant re-renders.
    pub fn set_tab_title(
        &mut self,
        tab_id: &TabId,
        title: impl Into<String>,
    ) -> Result<bool, CoreError> {
        let title = title.into();
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if !self.tabs[tab_index].set_title(title.clone()) {
            return Ok(false);
        }
        let tab = self.tabs[tab_index].clone();
        self.set_history_title_for_tab(&tab, tab.title().to_string());
        Ok(true)
    }

    pub fn clear_tab_favicon_key(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        self.tabs[tab_index].clear_favicon_key();
        let tab = self.tabs[tab_index].clone();
        self.clear_history_favicon_key_for_tab(&tab);
        Ok(())
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

    pub(super) fn active_tab_index(&self) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    fn active_tab_mut(&mut self) -> Result<&mut BrowserTab, CoreError> {
        let active_index = self.active_tab_index()?;
        self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)
    }

    fn update_tab_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
        update: TabUrlUpdate,
    ) -> Result<bool, CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if self.tabs[tab_index].url() == &url {
            return Ok(false);
        }

        match update {
            TabUrlUpdate::PushHistory => self.tabs[tab_index].navigate_to(url),
            TabUrlUpdate::PreserveHistory => self.tabs[tab_index].set_url(url),
        }
        self.tabs[tab_index].mark_ready();
        let snapshot_tab = self.tabs[tab_index].clone();
        self.record_history_entry(&snapshot_tab);
        self.record_tab_activity(tab_id, SystemTime::now());
        Ok(true)
    }

    fn navigate_active_tab_history(
        &mut self,
        direction: TabHistoryDirection,
    ) -> Result<bool, CoreError> {
        let active_id = self.active_tab_id.clone();
        let tab_index = self.active_tab_index()?;
        let navigated_url = match direction {
            TabHistoryDirection::Back => self.tabs[tab_index].navigate_back(),
            TabHistoryDirection::Forward => self.tabs[tab_index].navigate_forward(),
        };
        let Some(_url) = navigated_url else {
            return Ok(false);
        };

        self.tabs[tab_index].mark_ready();
        let snapshot_tab = self.tabs[tab_index].clone();
        self.record_history_entry(&snapshot_tab);
        self.record_tab_activity(&active_id, SystemTime::now());
        Ok(true)
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

enum TabHistoryDirection {
    Back,
    Forward,
}

enum TabUrlUpdate {
    PushHistory,
    PreserveHistory,
}
