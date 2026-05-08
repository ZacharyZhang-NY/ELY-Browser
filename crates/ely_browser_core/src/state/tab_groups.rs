use std::collections::BTreeMap;

use ely_domain::{
    BrowserTab, MAX_SPLIT_PANES, SpaceId, SplitAxis, SplitId, SplitLayout, SplitPane, TabGroup,
    TabGroupId, TabId,
};

use crate::CoreError;

use super::{BrowserCore, tab_order};

impl BrowserCore {
    pub fn group_active_tab(&mut self, name: impl Into<String>) -> Result<TabGroupId, CoreError> {
        let group_id = self.find_or_create_active_space_tab_group(name)?;
        let active_tab_id = self.active_tab_id.clone();
        self.assign_tab_to_group(&active_tab_id, &group_id)?;
        Ok(group_id)
    }

    pub fn auto_group_active_space_tabs_by_domain(&mut self) -> Result<usize, CoreError> {
        let domain_tab_ids = self.active_space_domain_tab_ids();
        let mut grouped_count = 0;

        for (domain, tab_ids) in domain_tab_ids {
            if tab_ids.len() < 2 {
                continue;
            }

            let group_id = self.find_or_create_active_space_tab_group(domain)?;
            for tab_id in tab_ids {
                self.assign_tab_to_group(&tab_id, &group_id)?;
                grouped_count += 1;
            }
        }

        Ok(grouped_count)
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

    pub fn ungroup_active_tab(&mut self) -> Result<bool, CoreError> {
        let active_tab_id = self.active_tab_id.clone();
        let Some(_) = self.active_tab_group_id()? else {
            return Ok(false);
        };
        self.clear_tab_group(&active_tab_id)?;
        Ok(true)
    }

    pub fn rename_active_tab_group(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Option<TabGroupId>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        self.rename_tab_group(&group_id, name)?;
        Ok(Some(group_id))
    }

    pub fn rename_tab_group(
        &mut self,
        group_id: &TabGroupId,
        name: impl Into<String>,
    ) -> Result<(), CoreError> {
        let group = self.tab_group_mut(group_id)?;
        group.rename(name).map_err(CoreError::from)
    }

    pub fn set_active_tab_group_color(&mut self, color_hex: u32) -> Result<Option<()>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        self.set_tab_group_color(&group_id, color_hex)?;
        Ok(Some(()))
    }

    pub fn set_tab_group_color(
        &mut self,
        group_id: &TabGroupId,
        color_hex: u32,
    ) -> Result<(), CoreError> {
        let group = self.tab_group_mut(group_id)?;
        group.set_color_hex(color_hex);
        Ok(())
    }

    pub fn discard_active_tab_group(&mut self) -> Result<Option<usize>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        let tab_ids = self.active_space_group_tab_ids(&group_id);

        for tab_id in &tab_ids {
            self.discard_tab(tab_id)?;
        }

        Ok(Some(tab_ids.len()))
    }

    pub fn refresh_active_tab_group(&mut self) -> Result<Option<usize>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        let tab_ids = self.active_space_group_tab_ids(&group_id);

        for tab_id in &tab_ids {
            self.refresh_tab(tab_id)?;
        }

        Ok(Some(tab_ids.len()))
    }

    pub fn close_active_tab_group(&mut self) -> Result<Option<usize>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        let tab_ids = self.active_space_group_tab_ids(&group_id);

        for tab_id in &tab_ids {
            self.clear_tab_group(tab_id)?;
        }

        self.tab_groups.retain(|group| group.id() != &group_id);

        for tab_id in &tab_ids {
            self.close_tab(tab_id)?;
        }

        Ok(Some(tab_ids.len()))
    }

    pub fn split_active_tab_group(&mut self) -> Result<Option<SplitId>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        let group_tabs = self.active_space_group_tabs(&group_id);
        if group_tabs.len() < 2 {
            return Ok(None);
        }
        if group_tabs.len() > MAX_SPLIT_PANES {
            return Err(CoreError::SplitPaneLimitReached { limit: MAX_SPLIT_PANES });
        }

        let pane_ids = group_tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();
        for pane_id in &pane_ids {
            self.detach_tab_from_split(pane_id);
        }

        let panes = pane_ids.iter().cloned().map(|tab_id| SplitPane::new(tab_id, 1)).collect();
        let layout = SplitLayout::new(SplitAxis::Horizontal, panes);
        let split_id = layout.id().clone();

        for tab in self.tabs.iter_mut().filter(|tab| pane_ids.contains(tab.id())) {
            tab.clear_group_id();
            tab.set_split_id(split_id.clone());
        }

        self.tab_groups.retain(|group| group.id() != &group_id);
        self.split_layouts.push(layout);
        Ok(Some(split_id))
    }

    pub fn group_active_split_view(
        &mut self,
        name: Option<&str>,
    ) -> Result<Option<TabGroupId>, CoreError> {
        let Some(split_id) = self.active_tab()?.split_id().cloned() else {
            return Ok(None);
        };
        let layout_index = self
            .split_layouts
            .iter()
            .position(|layout| layout.id() == &split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;
        let layout = self.split_layouts[layout_index].clone();
        if layout.pane_count() < 2 {
            return Ok(None);
        }

        let pane_ids = layout.panes().iter().map(|pane| pane.tab_id().clone()).collect::<Vec<_>>();
        let space_id = self.split_pane_space_id(&split_id, &pane_ids)?;
        let group_name = name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(|| layout.title().to_string(), ToString::to_string);
        let color_hex = self
            .spaces
            .iter()
            .find(|space| space.id() == &space_id)
            .map(|space| space.accent_hex())
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })?;
        let sort_key = self.next_tab_group_sort_key_for_space(&space_id);
        let group = TabGroup::new(space_id, group_name, color_hex, sort_key)?;
        let group_id = group.id().clone();

        for tab in self.tabs.iter_mut().filter(|tab| pane_ids.contains(tab.id())) {
            tab.clear_split_id();
            tab.set_group_id(group_id.clone());
        }

        self.split_layouts.remove(layout_index);
        self.tab_groups.push(group);
        Ok(Some(group_id))
    }

    pub fn toggle_active_tab_group_collapsed(&mut self) -> Result<Option<bool>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        self.toggle_tab_group_collapsed(&group_id).map(Some)
    }

    pub fn set_active_tab_group_collapsed(
        &mut self,
        collapsed: bool,
    ) -> Result<Option<()>, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(None);
        };
        self.set_tab_group_collapsed(&group_id, collapsed)?;
        Ok(Some(()))
    }

    pub fn toggle_tab_group_collapsed(&mut self, group_id: &TabGroupId) -> Result<bool, CoreError> {
        let group = self.tab_group_mut(group_id)?;
        let collapsed = !group.collapsed();
        group.set_collapsed(collapsed);
        Ok(collapsed)
    }

    pub fn set_tab_group_collapsed(
        &mut self,
        group_id: &TabGroupId,
        collapsed: bool,
    ) -> Result<(), CoreError> {
        let group = self.tab_group_mut(group_id)?;
        group.set_collapsed(collapsed);
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
        let sort_key = self.next_tab_group_sort_key_for_space(&active_space_id);
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

    fn active_space_domain_tab_ids(&self) -> BTreeMap<String, Vec<TabId>> {
        let mut domain_tab_ids = BTreeMap::new();
        for tab in self.tabs.iter().filter(|tab| self.tab_is_domain_group_candidate(tab)) {
            let Some(domain) = domain_group_name(tab) else {
                continue;
            };
            domain_tab_ids.entry(domain).or_insert_with(Vec::new).push(tab.id().clone());
        }

        domain_tab_ids
    }

    fn tab_is_domain_group_candidate(&self, tab: &BrowserTab) -> bool {
        tab.space_id() == &self.active_space_id
            && tab.group_id().is_none()
            && tab.split_id().is_none()
            && !tab.flags().favorite
            && !tab.flags().pinned
    }

    fn next_tab_group_sort_key_for_space(&self, space_id: &SpaceId) -> u64 {
        self.tab_groups
            .iter()
            .filter(|group| group.space_id() == space_id)
            .map(TabGroup::sort_key)
            .max()
            .map_or(0, |sort_key| sort_key.saturating_add(1))
    }

    fn active_space_group_tabs(&self, group_id: &TabGroupId) -> Vec<BrowserTab> {
        tab_order::sorted_tabs(
            self.tabs
                .iter()
                .filter(|tab| tab.space_id() == &self.active_space_id)
                .filter(|tab| tab.group_id() == Some(group_id)),
        )
    }

    fn active_space_group_tab_ids(&self, group_id: &TabGroupId) -> Vec<TabId> {
        self.active_space_group_tabs(group_id).iter().map(|tab| tab.id().clone()).collect()
    }

    fn split_pane_space_id(
        &self,
        split_id: &SplitId,
        pane_ids: &[TabId],
    ) -> Result<SpaceId, CoreError> {
        let first_pane_id =
            pane_ids.first().ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;
        let first_tab = self
            .tabs
            .iter()
            .find(|tab| tab.id() == first_pane_id)
            .ok_or_else(|| CoreError::TabNotFound { id: first_pane_id.clone() })?;
        let space_id = first_tab.space_id().clone();

        for pane_id in pane_ids {
            let tab = self
                .tabs
                .iter()
                .find(|tab| tab.id() == pane_id)
                .ok_or_else(|| CoreError::TabNotFound { id: pane_id.clone() })?;
            if tab.space_id() != &space_id {
                return Err(CoreError::SplitPaneSpaceMismatch { id: split_id.clone() });
            }
        }

        Ok(space_id)
    }

    fn active_tab_group_id(&self) -> Result<Option<TabGroupId>, CoreError> {
        let tab = self.active_tab()?;
        let Some(group_id) = tab.group_id().cloned() else {
            return Ok(None);
        };
        if self.tab_groups.iter().any(|group| group.id() == &group_id) {
            return Ok(Some(group_id));
        }

        Err(CoreError::TabGroupNotFound { id: group_id })
    }

    fn tab_group_mut(&mut self, group_id: &TabGroupId) -> Result<&mut TabGroup, CoreError> {
        self.tab_groups
            .iter_mut()
            .find(|group| group.id() == group_id)
            .ok_or_else(|| CoreError::TabGroupNotFound { id: group_id.clone() })
    }
}

fn domain_group_name(tab: &BrowserTab) -> Option<String> {
    if tab.url().as_str().starts_with("ely://") {
        return None;
    }

    tab.url().host()
}
