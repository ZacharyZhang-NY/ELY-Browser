use std::time::{Duration, SystemTime};

use ely_domain::{ArchivedTab, BrowserTab, ProfileKind, Space, SpaceId, TabId};

use super::BrowserCore;
use crate::CoreError;

pub const SPACE_TRASH_RETENTION_DAYS: u64 = 30;

#[derive(Clone, Debug)]
pub struct TrashedSpace {
    space: Space,
    tabs: Vec<BrowserTab>,
    archived_tabs: Vec<ArchivedTab>,
    trashed_at: SystemTime,
    purge_at: SystemTime,
}

impl TrashedSpace {
    fn new(
        space: Space,
        tabs: Vec<BrowserTab>,
        archived_tabs: Vec<ArchivedTab>,
        trashed_at: SystemTime,
    ) -> Self {
        let purge_at = trashed_at + Duration::from_secs(SPACE_TRASH_RETENTION_DAYS * 86_400);
        Self { space, tabs, archived_tabs, trashed_at, purge_at }
    }

    #[must_use]
    pub fn space(&self) -> &Space {
        &self.space
    }

    #[must_use]
    pub fn tabs(&self) -> &[BrowserTab] {
        &self.tabs
    }

    #[must_use]
    pub fn archived_tabs(&self) -> &[ArchivedTab] {
        &self.archived_tabs
    }

    #[must_use]
    pub fn trashed_at(&self) -> SystemTime {
        self.trashed_at
    }

    #[must_use]
    pub fn purge_at(&self) -> SystemTime {
        self.purge_at
    }
}

impl BrowserCore {
    pub fn create_space(
        &mut self,
        name: impl Into<String>,
        icon: impl Into<String>,
        accent_hex: u32,
    ) -> Result<SpaceId, CoreError> {
        let sort_key = self.next_space_sort_key();
        let default_profile_id = if self.active_profile()?.kind() == &ProfileKind::Private {
            self.active_space()?.default_profile_id().clone()
        } else {
            self.active_profile_id.clone()
        };
        let space = Space::new(name, icon, accent_hex, default_profile_id, sort_key);
        let space_id = space.id().clone();
        let default_profile_id = space.default_profile_id().clone();
        let tab = self.build_tab_for(space_id.clone(), default_profile_id, self.new_tab_url()?);
        let tab_id = tab.id().clone();

        self.spaces.push(space);
        self.tabs.push(tab);
        self.active_tabs_by_space.insert(space_id.clone(), tab_id.clone());
        self.select_tab(&tab_id)?;
        Ok(space_id)
    }

    pub fn select_space(&mut self, space_id: &SpaceId) -> Result<TabId, CoreError> {
        if !self.spaces.iter().any(|space| space.id() == space_id) {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        }

        if let Some(tab_id) = self
            .active_tabs_by_space
            .get(space_id)
            .filter(|tab_id| self.tab_belongs_to_space(tab_id, space_id))
            .cloned()
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        if let Some(tab_id) =
            self.tabs.iter().find(|tab| tab.space_id() == space_id).map(|tab| tab.id().clone())
        {
            self.select_tab(&tab_id)?;
            return Ok(tab_id);
        }

        let default_profile_id = self
            .spaces
            .iter()
            .find(|space| space.id() == space_id)
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?
            .default_profile_id()
            .clone();
        let tab = self.build_tab_for(space_id.clone(), default_profile_id, self.new_tab_url()?);
        let tab_id = tab.id().clone();
        self.tabs.push(tab);
        self.select_tab(&tab_id)?;
        Ok(tab_id)
    }

    pub fn select_next_space(&mut self) -> Result<TabId, CoreError> {
        let spaces = self.sorted_spaces();
        let Some(active_index) =
            spaces.iter().position(|space| space.id() == &self.active_space_id)
        else {
            return Err(CoreError::SpaceNotFound { id: self.active_space_id.clone() });
        };

        let next_index = (active_index + 1) % spaces.len();
        let space_id = spaces[next_index].id().clone();
        self.select_space(&space_id)
    }

    pub fn select_previous_space(&mut self) -> Result<TabId, CoreError> {
        let spaces = self.sorted_spaces();
        let Some(active_index) =
            spaces.iter().position(|space| space.id() == &self.active_space_id)
        else {
            return Err(CoreError::SpaceNotFound { id: self.active_space_id.clone() });
        };

        let previous_index = if active_index == 0 { spaces.len() - 1 } else { active_index - 1 };
        let space_id = spaces[previous_index].id().clone();
        self.select_space(&space_id)
    }

    pub fn trash_space(
        &mut self,
        space_id: &SpaceId,
        trashed_at: SystemTime,
    ) -> Result<bool, CoreError> {
        if self.spaces.len() <= 1 {
            return Err(CoreError::LastSpaceCannotBeTrashed);
        }

        let Some(space_index) = self.spaces.iter().position(|space| space.id() == space_id) else {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        };

        let trashed_space = self.spaces.remove(space_index);
        let tabs = self.remove_space_tabs(space_id);
        let archived_tabs = self.remove_space_archived_tabs(space_id);
        self.active_tabs_by_space.remove(space_id);
        self.active_tabs_by_space_profile
            .retain(|(mapped_space_id, _), _| mapped_space_id != space_id);
        self.trashed_spaces.push(TrashedSpace::new(trashed_space, tabs, archived_tabs, trashed_at));

        if &self.active_space_id == space_id {
            let next_space_id = self
                .sorted_spaces()
                .first()
                .map(|space| space.id().clone())
                .ok_or(CoreError::MissingActiveTab)?;
            self.select_space(&next_space_id)?;
        }

        Ok(true)
    }

    pub fn restore_trashed_space(&mut self, space_id: &SpaceId) -> Result<bool, CoreError> {
        let Some(trash_index) =
            self.trashed_spaces.iter().position(|entry| entry.space().id() == space_id)
        else {
            return Err(CoreError::TrashedSpaceNotFound { id: space_id.clone() });
        };

        let trashed_space = self.trashed_spaces.remove(trash_index);
        let restored_space_id = trashed_space.space().id().clone();
        self.spaces.push(trashed_space.space);
        self.tabs.extend(trashed_space.tabs);
        self.archived_tabs.extend(trashed_space.archived_tabs);
        self.select_space(&restored_space_id)?;
        Ok(true)
    }

    pub fn move_space_up(&mut self, space_id: &SpaceId) -> Result<bool, CoreError> {
        let mut ordered_ids = self.sorted_space_ids();
        let Some(index) = ordered_ids.iter().position(|id| id == space_id) else {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        };

        if index == 0 {
            return Ok(false);
        }

        ordered_ids.swap(index, index - 1);
        self.apply_space_order(&ordered_ids)?;
        Ok(true)
    }

    pub fn move_space_down(&mut self, space_id: &SpaceId) -> Result<bool, CoreError> {
        let mut ordered_ids = self.sorted_space_ids();
        let Some(index) = ordered_ids.iter().position(|id| id == space_id) else {
            return Err(CoreError::SpaceNotFound { id: space_id.clone() });
        };

        if index + 1 == ordered_ids.len() {
            return Ok(false);
        }

        ordered_ids.swap(index, index + 1);
        self.apply_space_order(&ordered_ids)?;
        Ok(true)
    }

    fn sorted_space_ids(&self) -> Vec<SpaceId> {
        self.sorted_spaces().iter().map(|space| space.id().clone()).collect()
    }

    pub(super) fn next_space_sort_key(&self) -> u64 {
        self.spaces
            .iter()
            .map(Space::sort_key)
            .max()
            .map_or(0, |sort_key| sort_key.saturating_add(1))
    }

    pub(super) fn sorted_spaces(&self) -> Vec<Space> {
        let mut spaces = self.spaces.clone();
        spaces.sort_by(|left, right| {
            left.sort_key().cmp(&right.sort_key()).then_with(|| left.id().cmp(right.id()))
        });
        spaces
    }

    fn apply_space_order(&mut self, ordered_ids: &[SpaceId]) -> Result<(), CoreError> {
        for (sort_key, space_id) in ordered_ids.iter().enumerate() {
            let Some(space) = self.spaces.iter_mut().find(|space| space.id() == space_id) else {
                return Err(CoreError::SpaceNotFound { id: space_id.clone() });
            };

            let sort_key = sort_key as u64;
            if space.sort_key() != sort_key {
                space.set_sort_key(sort_key);
            }
        }

        Ok(())
    }

    fn remove_space_tabs(&mut self, space_id: &SpaceId) -> Vec<BrowserTab> {
        let mut removed_tabs = Vec::new();
        self.tabs.retain(|tab| {
            if tab.space_id() == space_id {
                removed_tabs.push(tab.clone());
                false
            } else {
                true
            }
        });
        removed_tabs
    }

    fn remove_space_archived_tabs(&mut self, space_id: &SpaceId) -> Vec<ArchivedTab> {
        let mut removed_tabs = Vec::new();
        self.archived_tabs.retain(|archived| {
            if archived.tab().space_id() == space_id {
                removed_tabs.push(archived.clone());
                false
            } else {
                true
            }
        });
        removed_tabs
    }
}
