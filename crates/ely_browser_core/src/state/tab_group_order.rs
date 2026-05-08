use std::collections::BTreeSet;

use ely_domain::{SpaceId, TabGroupId, TabId};

use crate::CoreError;

use super::{BrowserCore, tab_order};

#[derive(Clone, Debug, Eq, PartialEq)]
enum TabGroupRowSegment {
    Group { group_id: TabGroupId, tab_ids: Vec<TabId> },
    Tab(TabId),
}

impl BrowserCore {
    pub fn move_active_tab_group_up(&mut self) -> Result<bool, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(false);
        };
        self.move_tab_group_up(&group_id)
    }

    pub fn move_active_tab_group_down(&mut self) -> Result<bool, CoreError> {
        let Some(group_id) = self.active_tab_group_id()? else {
            return Ok(false);
        };
        self.move_tab_group_down(&group_id)
    }

    pub fn move_tab_group_up(&mut self, group_id: &TabGroupId) -> Result<bool, CoreError> {
        let space_id = self.tab_group_space_id(group_id)?;
        let mut segments = self.tab_group_row_segments(&space_id);
        let Some(index) = tab_group_segment_index(&segments, group_id) else {
            return Ok(false);
        };
        let Some(previous_index) = previous_tab_group_segment_index(&segments, index) else {
            return Ok(false);
        };

        segments.swap(index, previous_index);
        self.apply_tab_group_segments(&space_id, &segments)?;
        Ok(true)
    }

    pub fn move_tab_group_down(&mut self, group_id: &TabGroupId) -> Result<bool, CoreError> {
        let space_id = self.tab_group_space_id(group_id)?;
        let mut segments = self.tab_group_row_segments(&space_id);
        let Some(index) = tab_group_segment_index(&segments, group_id) else {
            return Ok(false);
        };
        let Some(next_index) = next_tab_group_segment_index(&segments, index) else {
            return Ok(false);
        };

        segments.swap(index, next_index);
        self.apply_tab_group_segments(&space_id, &segments)?;
        Ok(true)
    }

    fn tab_group_space_id(&self, group_id: &TabGroupId) -> Result<SpaceId, CoreError> {
        self.tab_groups
            .iter()
            .find(|group| group.id() == group_id)
            .map(|group| group.space_id().clone())
            .ok_or_else(|| CoreError::TabGroupNotFound { id: group_id.clone() })
    }

    fn tab_group_row_segments(&self, space_id: &SpaceId) -> Vec<TabGroupRowSegment> {
        let ordered_tabs =
            tab_order::sorted_tabs(self.tabs.iter().filter(|tab| tab.space_id() == space_id));
        let valid_group_ids = self
            .tab_groups
            .iter()
            .filter(|group| group.space_id() == space_id)
            .map(|group| group.id().clone())
            .collect::<BTreeSet<_>>();
        let mut rendered_group_ids = BTreeSet::new();
        let mut segments = Vec::new();

        for tab in &ordered_tabs {
            let Some(group_id) = tab.group_id() else {
                segments.push(TabGroupRowSegment::Tab(tab.id().clone()));
                continue;
            };

            if !valid_group_ids.contains(group_id) {
                segments.push(TabGroupRowSegment::Tab(tab.id().clone()));
                continue;
            }

            if rendered_group_ids.insert(group_id.clone()) {
                let tab_ids = ordered_tabs
                    .iter()
                    .filter(|candidate| candidate.group_id() == Some(group_id))
                    .map(|candidate| candidate.id().clone())
                    .collect();
                segments.push(TabGroupRowSegment::Group { group_id: group_id.clone(), tab_ids });
            }
        }

        segments
    }

    fn apply_tab_group_segments(
        &mut self,
        space_id: &SpaceId,
        segments: &[TabGroupRowSegment],
    ) -> Result<(), CoreError> {
        let mut ordered_tab_ids = Vec::new();
        let mut ordered_group_ids = Vec::new();

        for segment in segments {
            match segment {
                TabGroupRowSegment::Group { group_id, tab_ids } => {
                    ordered_group_ids.push(group_id.clone());
                    ordered_tab_ids.extend(tab_ids.iter().cloned());
                }
                TabGroupRowSegment::Tab(tab_id) => ordered_tab_ids.push(tab_id.clone()),
            }
        }

        self.apply_space_tab_order(space_id, &ordered_tab_ids)?;
        self.apply_tab_group_order(&ordered_group_ids)?;
        Ok(())
    }

    fn apply_space_tab_order(
        &mut self,
        space_id: &SpaceId,
        ordered_tab_ids: &[TabId],
    ) -> Result<(), CoreError> {
        for (sort_key, tab_id) in ordered_tab_ids.iter().enumerate() {
            let Some(tab) =
                self.tabs.iter_mut().find(|tab| tab.space_id() == space_id && tab.id() == tab_id)
            else {
                return Err(CoreError::TabNotFound { id: tab_id.clone() });
            };
            tab.set_sort_key(sort_key as u64);
        }

        self.sort_tabs_within_space(space_id);
        Ok(())
    }

    fn apply_tab_group_order(&mut self, ordered_group_ids: &[TabGroupId]) -> Result<(), CoreError> {
        for (sort_key, group_id) in ordered_group_ids.iter().enumerate() {
            let Some(group) = self.tab_groups.iter_mut().find(|group| group.id() == group_id)
            else {
                return Err(CoreError::TabGroupNotFound { id: group_id.clone() });
            };
            group.set_sort_key(sort_key as u64);
        }

        Ok(())
    }
}

fn tab_group_segment_index(
    segments: &[TabGroupRowSegment],
    group_id: &TabGroupId,
) -> Option<usize> {
    segments.iter().position(|segment| match segment {
        TabGroupRowSegment::Group { group_id: candidate, .. } => candidate == group_id,
        TabGroupRowSegment::Tab(_) => false,
    })
}

fn previous_tab_group_segment_index(
    segments: &[TabGroupRowSegment],
    index: usize,
) -> Option<usize> {
    segments[..index]
        .iter()
        .rposition(|segment| matches!(segment, TabGroupRowSegment::Group { .. }))
}

fn next_tab_group_segment_index(segments: &[TabGroupRowSegment], index: usize) -> Option<usize> {
    segments[index.saturating_add(1)..]
        .iter()
        .position(|segment| matches!(segment, TabGroupRowSegment::Group { .. }))
        .map(|offset| index + 1 + offset)
}
