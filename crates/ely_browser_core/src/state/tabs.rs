use std::time::{Duration, SystemTime};

use ely_domain::{
    ArchivePolicy, ArchiveSource, ArchivedTab, BrowserTab, ProfileId, SpaceId, TabId, UrlText,
};

use crate::{
    CoreError,
    navigation::{tab_matches_query, tab_title},
};

use super::BrowserCore;

impl BrowserCore {
    pub fn open_new_tab(&mut self) -> Result<TabId, CoreError> {
        let url = self.new_tab_url()?;
        Ok(self.open_tab(url))
    }

    pub fn open_tab(&mut self, url: UrlText) -> TabId {
        let tab = self.build_tab(url);
        let tab_id = tab.id().clone();
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);
        self.record_history_entry(&tab);
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

        self.detach_tab_from_split(&tab_id);
        let tab_index = self.active_tab_index()?;
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

    pub fn archive_idle_tabs(&mut self, now: SystemTime) -> Result<usize, CoreError> {
        let ArchivePolicy::IdleDays(idle_days) = self.active_space()?.archive_policy() else {
            return Ok(0);
        };

        let active_space_id = self.active_space_id.clone();
        let idle_after = Duration::from_secs(u64::from(*idle_days) * 86_400);
        let tab_ids = self.idle_archive_candidates(&active_space_id, now, idle_after);
        let archived_count = tab_ids.len();

        for tab_id in tab_ids {
            self.archive_idle_tab(&tab_id)?;
        }

        Ok(archived_count)
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
        self.record_tab_activity(tab_id, SystemTime::now());
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

    fn restore_archived_tab_at_index(&mut self, index: usize) -> Result<TabId, CoreError> {
        let tab_id = self.archived_tabs[index].tab().id().clone();
        if let Some(split_id) = self.archived_split_id_for_tab(&tab_id) {
            return self.restore_archived_split(&split_id, &tab_id);
        }

        let archived_tab = self.archived_tabs.remove(index);
        self.restore_tab(archived_tab.into_tab())
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

    pub(super) fn active_tab_index(&self) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| tab.id() == &self.active_tab_id)
            .ok_or(CoreError::MissingActiveTab)
    }

    fn build_tab(&self, url: UrlText) -> BrowserTab {
        self.build_tab_for(self.active_space_id.clone(), self.active_profile_id.clone(), url)
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

    fn idle_archive_candidates(
        &self,
        space_id: &SpaceId,
        now: SystemTime,
        idle_after: Duration,
    ) -> Vec<TabId> {
        self.tabs
            .iter()
            .filter(|tab| tab.space_id() == space_id)
            .filter(|tab| tab.id() != &self.active_tab_id)
            .filter(|tab| !tab.flags().favorite && !tab.flags().pinned)
            .filter(|tab| tab.split_id().is_none())
            .filter(|tab| tab_is_idle(tab, now, idle_after))
            .map(|tab| tab.id().clone())
            .collect()
    }

    fn archive_idle_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let mut tab = self.tabs.remove(tab_index);
        let space_id = tab.space_id().clone();
        let profile_id = tab.profile_id().clone();

        tab.clear_split_id();
        self.archived_tabs.push(ArchivedTab::new(tab, ArchiveSource::AutoArchive));
        self.refresh_space_profile_active_tab(&space_id, &profile_id);
        Ok(())
    }

    fn refresh_space_profile_active_tab(&mut self, space_id: &SpaceId, profile_id: &ProfileId) {
        let key = (space_id.clone(), profile_id.clone());
        if self.active_tabs_by_space_profile.get(&key).is_some_and(|tab_id| {
            self.tabs.iter().any(|tab| {
                tab.id() == tab_id && tab.space_id() == space_id && tab.profile_id() == profile_id
            })
        }) {
            return;
        }

        self.active_tabs_by_space_profile.remove(&key);
        if let Some(tab_id) = self
            .tabs
            .iter()
            .find(|tab| tab.space_id() == space_id && tab.profile_id() == profile_id)
            .map(|tab| tab.id().clone())
        {
            self.active_tabs_by_space_profile.insert(key, tab_id);
        }
    }
}

fn tab_is_idle(tab: &BrowserTab, now: SystemTime, idle_after: Duration) -> bool {
    now.duration_since(tab.last_active_at()).is_ok_and(|idle_time| idle_time >= idle_after)
}
