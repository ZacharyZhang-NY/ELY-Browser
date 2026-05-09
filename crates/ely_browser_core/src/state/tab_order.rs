use ely_domain::{BrowserTab, SpaceId, TabId};

use super::BrowserCore;

impl BrowserCore {
    pub fn move_active_tab_up(&mut self) -> Result<bool, crate::CoreError> {
        self.move_active_tab_by(TabMoveDirection::Up)
    }

    pub fn move_active_tab_down(&mut self) -> Result<bool, crate::CoreError> {
        self.move_active_tab_by(TabMoveDirection::Down)
    }

    pub(super) fn next_tab_sort_key(&self, space_id: &SpaceId) -> u64 {
        self.tabs
            .iter()
            .filter(|tab| tab.space_id() == space_id)
            .map(BrowserTab::sort_key)
            .max()
            .map_or(0, |sort_key| sort_key.saturating_add(1))
    }

    pub(super) fn normalize_tab_sort_keys(&mut self, space_id: &SpaceId) {
        let mut sort_key = 0;
        for tab in self.tabs.iter_mut().filter(|tab| tab.space_id() == space_id) {
            tab.set_sort_key(sort_key);
            sort_key = sort_key.saturating_add(1);
        }
    }

    pub(super) fn sort_tabs_within_space(&mut self, space_id: &SpaceId) {
        let indices = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| (tab.space_id() == space_id).then_some(index))
            .collect::<Vec<_>>();
        let mut tabs = indices.iter().map(|index| self.tabs[*index].clone()).collect::<Vec<_>>();
        tabs.sort_by(compare_tabs);

        for (index, tab) in indices.into_iter().zip(tabs) {
            self.tabs[index] = tab;
        }
    }

    fn move_active_tab_by(
        &mut self,
        direction: TabMoveDirection,
    ) -> Result<bool, crate::CoreError> {
        let active_tab = self.active_tab()?.clone();
        let space_id = active_tab.space_id().clone();
        let mut tab_ids = sorted_tabs(self.tabs.iter().filter(|tab| tab.space_id() == &space_id))
            .into_iter()
            .map(|tab| tab.id().clone())
            .collect::<Vec<_>>();
        let Some(active_index) = tab_ids.iter().position(|tab_id| tab_id == active_tab.id()) else {
            return Err(crate::CoreError::MissingActiveTab);
        };
        let Some(target_index) = direction.target_index(active_index, tab_ids.len()) else {
            return Ok(false);
        };

        tab_ids.swap(active_index, target_index);
        self.apply_tab_order(&space_id, &tab_ids)?;
        Ok(true)
    }

    fn apply_tab_order(
        &mut self,
        space_id: &SpaceId,
        tab_ids: &[TabId],
    ) -> Result<(), crate::CoreError> {
        for (sort_key, tab_id) in tab_ids.iter().enumerate() {
            let tab = self
                .tabs
                .iter_mut()
                .find(|tab| tab.id() == tab_id && tab.space_id() == space_id)
                .ok_or_else(|| crate::CoreError::TabNotFound { id: tab_id.clone() })?;
            tab.set_sort_key(sort_key as u64);
        }
        self.sort_tabs_within_space(space_id);
        Ok(())
    }
}

pub(super) fn sorted_tabs<'a>(tabs: impl Iterator<Item = &'a BrowserTab>) -> Vec<BrowserTab> {
    let mut tabs = tabs.cloned().collect::<Vec<_>>();
    tabs.sort_by(compare_tabs);
    tabs
}

fn compare_tabs(left: &BrowserTab, right: &BrowserTab) -> std::cmp::Ordering {
    left.sort_key().cmp(&right.sort_key()).then_with(|| left.id().cmp(right.id()))
}

#[derive(Clone, Copy)]
enum TabMoveDirection {
    Up,
    Down,
}

impl TabMoveDirection {
    fn target_index(self, active_index: usize, tab_count: usize) -> Option<usize> {
        match self {
            Self::Up => active_index.checked_sub(1),
            Self::Down => (active_index + 1 < tab_count).then_some(active_index + 1),
        }
    }
}
