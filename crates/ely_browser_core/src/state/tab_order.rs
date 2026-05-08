use ely_domain::{BrowserTab, SpaceId};

use super::BrowserCore;

impl BrowserCore {
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
}

pub(super) fn sorted_tabs<'a>(tabs: impl Iterator<Item = &'a BrowserTab>) -> Vec<BrowserTab> {
    let mut tabs = tabs.cloned().collect::<Vec<_>>();
    tabs.sort_by(compare_tabs);
    tabs
}

fn compare_tabs(left: &BrowserTab, right: &BrowserTab) -> std::cmp::Ordering {
    left.sort_key().cmp(&right.sort_key()).then_with(|| left.id().cmp(right.id()))
}
