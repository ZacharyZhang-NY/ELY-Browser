use std::time::SystemTime;

use ely_domain::TabId;

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn select_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let previous_profile_id = self.active_profile_id.clone();
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
            .insert((active_space_id.clone(), active_profile_id.clone()), active_tab_id.clone());
        self.active_tabs_by_space.insert(active_space_id, active_tab_id);
        self.record_tab_activity(tab_id, SystemTime::now());

        if previous_profile_id != active_profile_id {
            self.cleanup_private_profile_session_data(&previous_profile_id);
        }
        Ok(())
    }

    pub fn select_next_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(1)
    }

    pub fn select_previous_tab(&mut self) -> Result<TabId, CoreError> {
        self.select_tab_by_offset(-1)
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
}
