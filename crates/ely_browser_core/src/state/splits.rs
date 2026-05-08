use ely_domain::{
    ArchiveSource, ArchivedTab, BrowserTab, MAX_SPLIT_PANES, ProfileId, SpaceId, SplitAxis,
    SplitId, SplitLayout, SplitPane, TabId,
};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn close_active_saved_split_view(&mut self) -> Result<Option<TabId>, CoreError> {
        let Some(split_id) = self.active_tab()?.split_id().cloned() else {
            return Ok(None);
        };
        if !self.saved_split_exists(&split_id) {
            return Ok(None);
        }

        self.close_saved_split_view(&split_id).map(Some)
    }

    pub fn close_saved_split_view(&mut self, split_id: &SplitId) -> Result<TabId, CoreError> {
        let layout = self.saved_split_layout(split_id)?.clone();
        let pane_ids = layout.panes().iter().map(|pane| pane.tab_id().clone()).collect::<Vec<_>>();
        self.validate_split_panes_are_open(&pane_ids)?;

        let first_close_index = self.first_split_tab_index(&pane_ids)?;
        let first_tab = self.tabs.get(first_close_index).ok_or(CoreError::MissingActiveTab)?;
        let closed_space_id = first_tab.space_id().clone();
        let closed_profile_id = first_tab.profile_id().clone();
        let was_active = pane_ids.contains(&self.active_tab_id);
        let was_space_active = self
            .active_tabs_by_space
            .get(&closed_space_id)
            .is_some_and(|tab_id| pane_ids.contains(tab_id));
        let was_profile_active = self
            .active_tabs_by_space_profile
            .get(&(closed_space_id.clone(), closed_profile_id.clone()))
            .is_some_and(|tab_id| pane_ids.contains(tab_id));

        let archived_tabs = self.remove_split_tabs(&pane_ids, true)?;
        self.archived_tabs.extend(archived_tabs);
        self.split_layouts.retain(|layout| layout.id() != split_id);
        self.archived_split_layouts.push(layout);

        self.reselect_after_split_close(
            closed_space_id,
            closed_profile_id,
            first_close_index,
            was_active,
            was_space_active,
            was_profile_active,
        )
    }

    pub fn save_active_split_view(&mut self) -> Result<Option<SplitId>, CoreError> {
        let Some(split_id) = self.active_tab()?.split_id().cloned() else {
            return Ok(None);
        };
        let title = self.active_split_title(&split_id)?;
        let layout = self
            .split_layouts
            .iter_mut()
            .find(|layout| layout.id() == &split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;

        layout.save(title);
        Ok(Some(split_id))
    }

    pub fn set_active_split_axis(&mut self, axis: SplitAxis) -> Result<Option<SplitId>, CoreError> {
        let Some(split_id) = self.active_tab()?.split_id().cloned() else {
            return Ok(None);
        };
        let layout = self
            .split_layouts
            .iter_mut()
            .find(|layout| layout.id() == &split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;

        layout.set_axis(axis);
        Ok(Some(split_id))
    }

    pub fn detach_active_split_pane(&mut self) -> Result<bool, CoreError> {
        let active_tab = self.active_tab()?;
        let active_tab_id = active_tab.id().clone();
        let Some(split_id) = active_tab.split_id().cloned() else {
            return Ok(false);
        };
        if !self.split_layouts.iter().any(|layout| layout.id() == &split_id) {
            return Err(CoreError::SplitNotFound { id: split_id });
        }

        self.detach_tab_from_split(&active_tab_id);
        self.refresh_saved_split_title(&split_id)?;
        Ok(true)
    }

    pub fn split_active_tab_right(&mut self) -> Result<SplitId, CoreError> {
        let active_index = self.active_tab_index()?;
        let active_tab_id = self.tabs[active_index].id().clone();
        let active_space_id = self.tabs[active_index].space_id().clone();
        let active_profile_id = self.tabs[active_index].profile_id().clone();
        let split_id = self.split_id_for_new_pane(&active_tab_id)?;
        let mut new_tab =
            self.build_tab_for(active_space_id, active_profile_id, self.new_tab_url()?);
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

    fn active_split_title(&self, split_id: &SplitId) -> Result<String, CoreError> {
        let layout = self
            .split_layouts
            .iter()
            .find(|layout| layout.id() == split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;
        let pane_titles = layout
            .panes()
            .iter()
            .filter_map(|pane| {
                self.tabs
                    .iter()
                    .find(|tab| tab.id() == pane.tab_id())
                    .map(|tab| tab.title().to_string())
            })
            .collect::<Vec<_>>();

        Ok(match pane_titles.as_slice() {
            [] => "Split View".to_string(),
            [title] => format!("Split View: {title}"),
            [first, second, ..] => format!("Split View: {first} + {second}"),
        })
    }

    fn refresh_saved_split_title(&mut self, split_id: &SplitId) -> Result<(), CoreError> {
        let Some(layout_index) =
            self.split_layouts.iter().position(|layout| layout.id() == split_id)
        else {
            return Ok(());
        };
        if !self.split_layouts[layout_index].saved() {
            return Ok(());
        }

        let title = self.active_split_title(split_id)?;
        self.split_layouts[layout_index].save(title);
        Ok(())
    }

    pub(super) fn saved_split_id_for_tab(&self, tab_id: &TabId) -> Option<SplitId> {
        let split_id =
            self.tabs.iter().find(|tab| tab.id() == tab_id).and_then(|tab| tab.split_id())?;
        self.saved_split_exists(split_id).then(|| split_id.clone())
    }

    fn saved_split_exists(&self, split_id: &SplitId) -> bool {
        self.split_layouts.iter().any(|layout| layout.id() == split_id && layout.saved())
    }

    fn saved_split_layout(&self, split_id: &SplitId) -> Result<&SplitLayout, CoreError> {
        self.split_layouts
            .iter()
            .find(|layout| layout.id() == split_id && layout.saved())
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })
    }

    fn validate_split_panes_are_open(&self, pane_ids: &[TabId]) -> Result<(), CoreError> {
        if let Some(missing_tab_id) =
            pane_ids.iter().find(|tab_id| !self.tabs.iter().any(|tab| tab.id() == *tab_id))
        {
            return Err(CoreError::TabNotFound { id: missing_tab_id.clone() });
        }

        Ok(())
    }

    fn first_split_tab_index(&self, pane_ids: &[TabId]) -> Result<usize, CoreError> {
        self.tabs
            .iter()
            .position(|tab| pane_ids.contains(tab.id()))
            .ok_or(CoreError::MissingActiveTab)
    }

    fn remove_split_tabs(
        &mut self,
        pane_ids: &[TabId],
        preserve_split_id: bool,
    ) -> Result<Vec<ArchivedTab>, CoreError> {
        let mut archived_tabs = Vec::with_capacity(pane_ids.len());
        for pane_id in pane_ids {
            let tab_index = self
                .tabs
                .iter()
                .position(|tab| tab.id() == pane_id)
                .ok_or_else(|| CoreError::TabNotFound { id: pane_id.clone() })?;
            let mut tab = self.tabs.remove(tab_index);
            if !preserve_split_id {
                tab.clear_split_id();
            }
            archived_tabs.push(ArchivedTab::new(tab, ArchiveSource::ManualClose));
        }

        Ok(archived_tabs)
    }

    fn reselect_after_split_close(
        &mut self,
        closed_space_id: SpaceId,
        closed_profile_id: ProfileId,
        start_index: usize,
        was_active: bool,
        was_space_active: bool,
        was_profile_active: bool,
    ) -> Result<TabId, CoreError> {
        if let Some(next_tab_id) = self.nearest_tab_in_space(&closed_space_id, start_index) {
            if was_space_active {
                self.active_tabs_by_space.insert(closed_space_id.clone(), next_tab_id.clone());
            }
            if was_profile_active {
                self.active_tabs_by_space_profile
                    .remove(&(closed_space_id.clone(), closed_profile_id.clone()));
                if let Some(next_profile_tab_id) = self.nearest_tab_in_space_profile(
                    &closed_space_id,
                    &closed_profile_id,
                    start_index,
                ) {
                    self.active_tabs_by_space_profile
                        .insert((closed_space_id, closed_profile_id), next_profile_tab_id);
                }
            }
            if was_active {
                self.select_tab(&next_tab_id)?;
            }
            return Ok(self.active_tab_id.clone());
        }

        let replacement = self.build_tab_for(
            closed_space_id.clone(),
            closed_profile_id.clone(),
            self.new_tab_url()?,
        );
        let replacement_id = replacement.id().clone();
        self.tabs.insert(start_index.min(self.tabs.len()), replacement);
        self.active_tabs_by_space.insert(closed_space_id.clone(), replacement_id.clone());
        self.active_tabs_by_space_profile
            .insert((closed_space_id, closed_profile_id), replacement_id.clone());

        if was_active {
            self.select_tab(&replacement_id)?;
        }

        Ok(self.active_tab_id.clone())
    }

    pub(super) fn archived_split_id_for_tab(&self, tab_id: &TabId) -> Option<SplitId> {
        let split_id = self
            .archived_tabs
            .iter()
            .find(|archived| archived.tab().id() == tab_id)
            .and_then(|archived| archived.tab().split_id())?;
        self.archived_split_exists(split_id).then(|| split_id.clone())
    }

    pub(super) fn restore_archived_split(
        &mut self,
        split_id: &SplitId,
        focus_tab_id: &TabId,
    ) -> Result<TabId, CoreError> {
        let layout = self.archived_split_layout(split_id)?.clone();
        let pane_ids = layout.panes().iter().map(|pane| pane.tab_id().clone()).collect::<Vec<_>>();
        self.validate_archived_split_panes(&pane_ids)?;

        let restored_tabs = self.remove_archived_split_tabs(&pane_ids)?;
        self.archived_split_layouts.retain(|layout| layout.id() != split_id);
        self.insert_restored_split_tabs(restored_tabs);
        self.split_layouts.push(layout);
        self.select_tab(focus_tab_id)?;
        Ok(focus_tab_id.clone())
    }

    fn archived_split_exists(&self, split_id: &SplitId) -> bool {
        self.archived_split_layouts.iter().any(|layout| layout.id() == split_id)
    }

    fn archived_split_layout(&self, split_id: &SplitId) -> Result<&SplitLayout, CoreError> {
        self.archived_split_layouts
            .iter()
            .find(|layout| layout.id() == split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })
    }

    fn validate_archived_split_panes(&self, pane_ids: &[TabId]) -> Result<(), CoreError> {
        if let Some(missing_tab_id) = pane_ids.iter().find(|tab_id| {
            !self.archived_tabs.iter().any(|archived| archived.tab().id() == *tab_id)
        }) {
            return Err(CoreError::TabNotFound { id: missing_tab_id.clone() });
        }

        Ok(())
    }

    fn remove_archived_split_tabs(
        &mut self,
        pane_ids: &[TabId],
    ) -> Result<Vec<BrowserTab>, CoreError> {
        let mut tabs = Vec::with_capacity(pane_ids.len());
        for pane_id in pane_ids {
            let archived_index = self
                .archived_tabs
                .iter()
                .position(|archived| archived.tab().id() == pane_id)
                .ok_or_else(|| CoreError::TabNotFound { id: pane_id.clone() })?;
            tabs.push(self.archived_tabs.remove(archived_index).into_tab());
        }

        Ok(tabs)
    }

    fn insert_restored_split_tabs(&mut self, tabs: Vec<BrowserTab>) {
        let insert_index = self
            .tabs
            .iter()
            .position(|existing| existing.id() == &self.active_tab_id)
            .map_or(self.tabs.len(), |index| index + 1);

        for (offset, tab) in tabs.into_iter().enumerate() {
            self.tabs.insert(insert_index + offset, tab);
        }
    }
}
