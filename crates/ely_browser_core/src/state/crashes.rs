use ely_domain::TabId;

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn crash_active_tab(&mut self) -> Result<TabId, CoreError> {
        let tab_id = self.active_tab_id.clone();
        self.crash_tab(&tab_id)
    }

    pub fn crash_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        tab.mark_crashed();
        Ok(tab_id.clone())
    }

    pub fn recover_crashed_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        {
            let tab = self
                .tabs
                .iter_mut()
                .find(|tab| tab.id() == tab_id)
                .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
            tab.mark_ready();
        }

        self.select_tab(tab_id)?;
        Ok(tab_id.clone())
    }
}
