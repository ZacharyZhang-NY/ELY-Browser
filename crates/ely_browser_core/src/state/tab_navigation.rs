use std::time::SystemTime;

use ely_domain::{TabId, UrlText};

use super::BrowserCore;
use crate::{CoreError, navigation::tab_title};

impl BrowserCore {
    /// Navigate the active tab to `url` in place. Used for clicks on
    /// settings sub-pages, the home anchor, the bottom Settings row —
    /// places where the user expects the current tab to follow the
    /// link instead of accumulating a new tab for every step.
    ///
    /// Records a fresh history entry and resets activity timestamp;
    /// no new tab is created and the active tab id is unchanged.
    pub fn navigate_active_tab(&mut self, url: UrlText) -> Result<(), CoreError> {
        let active_id = self.active_tab_id.clone();
        self.update_tab_url(&active_id, url, TabUrlUpdate::PushHistory)?;
        Ok(())
    }

    pub fn navigate_tab_to_loaded_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
    ) -> Result<bool, CoreError> {
        self.update_tab_url(tab_id, url, TabUrlUpdate::PushHistory)
    }

    pub fn replace_tab_loaded_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
    ) -> Result<bool, CoreError> {
        self.update_tab_url(tab_id, url, TabUrlUpdate::PreserveHistory)
    }

    pub fn navigate_active_tab_back(&mut self) -> Result<bool, CoreError> {
        self.navigate_active_tab_history(TabHistoryDirection::Back)
    }

    pub fn navigate_active_tab_forward(&mut self) -> Result<bool, CoreError> {
        self.navigate_active_tab_history(TabHistoryDirection::Forward)
    }

    pub(super) fn update_tab_url(
        &mut self,
        tab_id: &TabId,
        url: UrlText,
        update: TabUrlUpdate,
    ) -> Result<bool, CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if self.tabs[tab_index].url() == &url {
            return Ok(false);
        }

        match update {
            TabUrlUpdate::PushHistory => self.tabs[tab_index].navigate_to(url),
            TabUrlUpdate::PreserveHistory => self.tabs[tab_index].set_url(url),
        }
        self.refresh_tab_url_metadata(tab_index)?;
        self.tabs[tab_index].mark_ready();
        let snapshot_tab = self.tabs[tab_index].clone();
        self.record_history_entry(&snapshot_tab);
        self.record_tab_activity(tab_id, SystemTime::now());
        Ok(true)
    }

    fn navigate_active_tab_history(
        &mut self,
        direction: TabHistoryDirection,
    ) -> Result<bool, CoreError> {
        let active_id = self.active_tab_id.clone();
        let tab_index = self.active_tab_index()?;
        let navigated_url = match direction {
            TabHistoryDirection::Back => self.tabs[tab_index].navigate_back(),
            TabHistoryDirection::Forward => self.tabs[tab_index].navigate_forward(),
        };
        let Some(_url) = navigated_url else {
            return Ok(false);
        };

        self.refresh_tab_url_metadata(tab_index)?;
        self.tabs[tab_index].mark_ready();
        let snapshot_tab = self.tabs[tab_index].clone();
        self.record_history_entry(&snapshot_tab);
        self.record_tab_activity(&active_id, SystemTime::now());
        Ok(true)
    }

    fn refresh_tab_url_metadata(&mut self, tab_index: usize) -> Result<(), CoreError> {
        let title = tab_title(self.tabs[tab_index].url());
        self.tabs[tab_index].set_title(title);
        if let Some(favicon_key) = self.tabs[tab_index].url().favicon_key() {
            self.tabs[tab_index].set_favicon_key(favicon_key)?;
        } else {
            self.tabs[tab_index].clear_favicon_key();
        }
        Ok(())
    }
}

enum TabHistoryDirection {
    Back,
    Forward,
}

pub(super) enum TabUrlUpdate {
    PushHistory,
    PreserveHistory,
}
