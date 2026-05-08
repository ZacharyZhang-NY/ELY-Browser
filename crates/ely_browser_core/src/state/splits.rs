use ely_domain::{MAX_SPLIT_PANES, SplitAxis, SplitId, SplitLayout, SplitPane, TabId};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn split_active_tab_right(&mut self) -> Result<SplitId, CoreError> {
        let active_index = self.active_tab_index()?;
        let active_tab_id = self.tabs[active_index].id().clone();
        let active_space_id = self.tabs[active_index].space_id().clone();
        let active_profile_id = self.tabs[active_index].profile_id().clone();
        let split_id = self.split_id_for_new_pane(&active_tab_id)?;
        let mut new_tab =
            self.build_tab_for(active_space_id, active_profile_id, self.new_tab_url.clone());
        let new_tab_id = new_tab.id().clone();
        new_tab.set_split_id(split_id.clone());

        let layout = self
            .split_layouts
            .iter_mut()
            .find(|layout| layout.id() == &split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;
        if !layout.add_pane(SplitPane::new(new_tab_id.clone(), 1)) {
            return Err(CoreError::SplitPaneLimitReached { limit: MAX_SPLIT_PANES });
        }

        let insert_index = self.active_tab_index()? + 1;
        self.tabs.insert(insert_index, new_tab);
        self.select_tab(&new_tab_id)?;
        Ok(split_id)
    }

    pub(super) fn detach_tab_from_split(&mut self, tab_id: &TabId) {
        for tab in self.tabs.iter_mut().filter(|tab| tab.id() == tab_id) {
            tab.clear_split_id();
        }

        for layout in &mut self.split_layouts {
            layout.remove_tab(tab_id);
        }

        let dissolved_split_ids = self
            .split_layouts
            .iter()
            .filter(|layout| layout.pane_count() <= 1)
            .map(|layout| layout.id().clone())
            .collect::<Vec<_>>();

        for split_id in &dissolved_split_ids {
            for tab in self.tabs.iter_mut().filter(|tab| tab.split_id() == Some(split_id)) {
                tab.clear_split_id();
            }
        }

        self.split_layouts.retain(|layout| layout.pane_count() > 1);
    }

    pub(super) fn visible_split_layouts(&self) -> Vec<SplitLayout> {
        self.split_layouts
            .iter()
            .filter(|layout| {
                layout.panes().iter().all(|pane| {
                    self.tabs.iter().any(|tab| {
                        tab.id() == pane.tab_id() && tab.space_id() == &self.active_space_id
                    })
                })
            })
            .cloned()
            .collect()
    }

    fn split_id_for_new_pane(&mut self, active_tab_id: &TabId) -> Result<SplitId, CoreError> {
        if let Some(split_id) = self.active_split_id(active_tab_id) {
            if self
                .split_layouts
                .iter()
                .any(|layout| layout.id() == &split_id && layout.pane_count() < MAX_SPLIT_PANES)
            {
                return Ok(split_id);
            }

            self.detach_tab_from_split(active_tab_id);
        }

        let active_index = self.active_tab_index()?;
        let layout =
            SplitLayout::new(SplitAxis::Horizontal, vec![SplitPane::new(active_tab_id.clone(), 1)]);
        let split_id = layout.id().clone();
        self.tabs[active_index].set_split_id(split_id.clone());
        self.split_layouts.push(layout);
        Ok(split_id)
    }

    fn active_split_id(&self, active_tab_id: &TabId) -> Option<SplitId> {
        self.tabs
            .iter()
            .find(|tab| tab.id() == active_tab_id)
            .and_then(|tab| tab.split_id().cloned())
    }
}
