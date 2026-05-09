use std::time::SystemTime;

use ely_domain::{DiagnosticEventKind, TabId, WebViewCrashKind};

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
        self.record_diagnostic_event(DiagnosticEventKind::WebViewCrash {
            crash_kind: WebViewCrashKind::TabCrashed,
        });
        Ok(tab_id.clone())
    }

    pub fn recover_crashed_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        self.mark_tab_ready(tab_id)
    }

    pub fn discard_active_tab(&mut self) -> Result<TabId, CoreError> {
        let tab_id = self.active_tab_id.clone();
        self.discard_tab(&tab_id)
    }

    pub fn discard_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        tab.mark_discarded();
        Ok(tab_id.clone())
    }

    pub fn wake_discarded_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        self.mark_tab_ready(tab_id)
    }

    pub(super) fn refresh_tab(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
        {
            let tab = self
                .tabs
                .iter_mut()
                .find(|tab| tab.id() == tab_id)
                .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
            tab.mark_ready();
        }

        self.record_tab_activity(tab_id, SystemTime::now());
        Ok(tab_id.clone())
    }

    fn mark_tab_ready(&mut self, tab_id: &TabId) -> Result<TabId, CoreError> {
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
