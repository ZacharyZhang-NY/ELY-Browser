use ely_domain::TabId;

use super::BrowserCore;
use crate::CoreError;

impl BrowserCore {
    pub fn toggle_active_tab_favorite(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let favorite_count = self.tabs.iter().filter(|tab| tab.flags().favorite).count();
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_favorite = !active_tab.flags().favorite;
        let favorite_limit = self.favorite_limit.value();

        if next_favorite && favorite_count >= favorite_limit {
            return Err(CoreError::FavoriteLimitReached { limit: favorite_limit });
        }

        active_tab.set_favorite(next_favorite);
        Ok(next_favorite)
    }

    pub fn toggle_active_tab_pinned(&mut self) -> Result<bool, CoreError> {
        let active_index = self.active_tab_index()?;
        let active_tab = self.tabs.get_mut(active_index).ok_or(CoreError::MissingActiveTab)?;
        let next_pinned = !active_tab.flags().pinned;
        active_tab.set_pinned(next_pinned);
        Ok(next_pinned)
    }

    pub fn set_active_tab_zoom_percent(&mut self, zoom_percent: u16) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.set_zoom_percent(zoom_percent)?;
        Ok(active_tab.zoom_percent())
    }

    pub fn zoom_active_tab_in(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.zoom_in();
        Ok(active_tab.zoom_percent())
    }

    pub fn zoom_active_tab_out(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.zoom_out();
        Ok(active_tab.zoom_percent())
    }

    pub fn reset_active_tab_zoom(&mut self) -> Result<u16, CoreError> {
        let active_tab = self.active_tab_mut()?;
        active_tab.reset_zoom();
        Ok(active_tab.zoom_percent())
    }

    pub fn set_tab_sync_enabled(
        &mut self,
        tab_id: &TabId,
        sync_enabled: bool,
    ) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        tab.set_sync_enabled(sync_enabled);
        Ok(())
    }

    pub fn set_tab_sort_key(&mut self, tab_id: &TabId, sort_key: u64) -> Result<(), CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let space_id = tab.space_id().clone();
        tab.set_sort_key(sort_key);
        self.sort_tabs_within_space(&space_id);
        Ok(())
    }

    pub fn set_tab_favicon_key(
        &mut self,
        tab_id: &TabId,
        favicon_key: impl Into<String>,
    ) -> Result<bool, CoreError> {
        let favicon_key = favicon_key.into();
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if self.tabs[tab_index].favicon_key() == Some(favicon_key.as_str()) {
            return Ok(false);
        }
        self.tabs[tab_index].set_favicon_key(favicon_key.clone())?;
        let tab = self.tabs[tab_index].clone();
        self.set_history_favicon_key_for_tab(&tab, favicon_key);
        Ok(true)
    }

    /// Replace `tab_id`'s title with the live page title and mirror
    /// the new title into the history entry that recorded the visit.
    /// Returns `Ok(true)` only when the title actually changed —
    /// callers can use this to suppress redundant re-renders.
    pub fn set_tab_title(
        &mut self,
        tab_id: &TabId,
        title: impl Into<String>,
    ) -> Result<bool, CoreError> {
        let title = title.into();
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        if !self.tabs[tab_index].set_title(title.clone()) {
            return Ok(false);
        }
        let tab = self.tabs[tab_index].clone();
        self.set_history_title_for_tab(&tab, tab.title().to_string());
        Ok(true)
    }

    pub fn clear_tab_favicon_key(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        self.tabs[tab_index].clear_favicon_key();
        let tab = self.tabs[tab_index].clone();
        self.clear_history_favicon_key_for_tab(&tab);
        Ok(())
    }
}
