use ely_domain::{TabGroup, TabGroupId, TabId};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn group_active_tab(&mut self, name: impl Into<String>) -> Result<TabGroupId, CoreError> {
        let group_id = self.find_or_create_active_space_tab_group(name)?;
        let active_tab_id = self.active_tab_id.clone();
        self.assign_tab_to_group(&active_tab_id, &group_id)?;
        Ok(group_id)
    }

    pub fn assign_tab_to_group(
        &mut self,
        tab_id: &TabId,
        group_id: &TabGroupId,
    ) -> Result<(), CoreError> {
        let group_space_id = self
            .tab_groups
            .iter()
            .find(|group| group.id() == group_id)
            .map(|group| group.space_id().clone())
            .ok_or_else(|| CoreError::TabGroupNotFound { id: group_id.clone() })?;
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;

        if tab.space_id() != &group_space_id {
            return Err(CoreError::TabGroupNotFound { id: group_id.clone() });
        }

        tab.set_group_id(group_id.clone());
        Ok(())
    }

    pub fn clear_tab_group(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        tab.clear_group_id();
        Ok(())
    }

    pub(super) fn visible_tab_groups(&self) -> Vec<TabGroup> {
        let mut groups = self
            .tab_groups
            .iter()
            .filter(|group| group.space_id() == &self.active_space_id)
            .cloned()
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| {
            left.sort_key().cmp(&right.sort_key()).then_with(|| left.id().cmp(right.id()))
        });
        groups
    }

    fn find_or_create_active_space_tab_group(
        &mut self,
        name: impl Into<String>,
    ) -> Result<TabGroupId, CoreError> {
        let name = name.into();
        if let Some(group_id) = self.find_active_space_group_id(&name) {
            return Ok(group_id);
        }

        let active_space_id = self.active_space_id.clone();
        let color_hex = self.active_space()?.accent_hex();
        let sort_key = self.next_tab_group_sort_key();
        let group = TabGroup::new(active_space_id, name, color_hex, sort_key)?;
        let group_id = group.id().clone();
        self.tab_groups.push(group);
        Ok(group_id)
    }

    fn find_active_space_group_id(&self, name: &str) -> Option<TabGroupId> {
        let normalized_name = name.trim().to_lowercase();
        self.tab_groups
            .iter()
            .find(|group| {
                group.space_id() == &self.active_space_id
                    && group.name().to_lowercase() == normalized_name
            })
            .map(|group| group.id().clone())
    }

    fn next_tab_group_sort_key(&self) -> u64 {
        self.tab_groups
            .iter()
            .filter(|group| group.space_id() == &self.active_space_id)
            .map(TabGroup::sort_key)
            .max()
            .map_or(0, |sort_key| sort_key.saturating_add(1))
    }
}
