use ely_domain::TabId;

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn visible_content_tab_ids(&self) -> Result<Vec<TabId>, CoreError> {
        let active_tab = self.active_tab()?;
        let Some(split_id) = active_tab.split_id() else {
            return Ok(vec![active_tab.id().clone()]);
        };
        let layout = self
            .split_layouts
            .iter()
            .find(|layout| layout.id() == split_id)
            .ok_or_else(|| CoreError::SplitNotFound { id: split_id.clone() })?;
        if layout.pane_count() < 2 {
            return Ok(vec![active_tab.id().clone()]);
        }
        Ok(layout.panes().iter().map(|pane| pane.tab_id().clone()).collect())
    }
}
