use std::time::{Duration, SystemTime};

use ely_domain::{
    ArchivePolicy, ArchiveSource, ArchivedTab, BrowserTab, DiagnosticEventKind, ProfileId, SpaceId,
    TabId, WebViewCrashKind,
};

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

    pub fn archive_idle_tabs(&mut self, now: SystemTime) -> Result<usize, CoreError> {
        let ArchivePolicy::IdleDays(idle_days) = self.active_space()?.archive_policy() else {
            return Ok(0);
        };

        let active_space_id = self.active_space_id.clone();
        let idle_after = Duration::from_secs(u64::from(*idle_days) * 86_400);
        let tab_ids = self.idle_archive_candidates(&active_space_id, now, idle_after);
        let archived_count = tab_ids.len();

        for tab_id in tab_ids {
            self.archive_idle_tab(&tab_id)?;
        }

        Ok(archived_count)
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

    fn idle_archive_candidates(
        &self,
        space_id: &SpaceId,
        now: SystemTime,
        idle_after: Duration,
    ) -> Vec<TabId> {
        self.tabs
            .iter()
            .filter(|tab| tab.space_id() == space_id)
            .filter(|tab| tab.id() != &self.active_tab_id)
            .filter(|tab| !tab.flags().favorite && !tab.flags().pinned)
            .filter(|tab| tab.split_id().is_none())
            .filter(|tab| tab_is_idle(tab, now, idle_after))
            .map(|tab| tab.id().clone())
            .collect()
    }

    fn archive_idle_tab(&mut self, tab_id: &TabId) -> Result<(), CoreError> {
        let tab_index = self
            .tabs
            .iter()
            .position(|tab| tab.id() == tab_id)
            .ok_or_else(|| CoreError::TabNotFound { id: tab_id.clone() })?;
        let mut tab = self.tabs.remove(tab_index);
        let space_id = tab.space_id().clone();
        let profile_id = tab.profile_id().clone();

        tab.clear_split_id();
        self.archived_tabs.push(ArchivedTab::new(tab, ArchiveSource::AutoArchive));
        self.refresh_space_profile_active_tab(&space_id, &profile_id);
        Ok(())
    }

    fn refresh_space_profile_active_tab(&mut self, space_id: &SpaceId, profile_id: &ProfileId) {
        let key = (space_id.clone(), profile_id.clone());
        if self.active_tabs_by_space_profile.get(&key).is_some_and(|tab_id| {
            self.tabs.iter().any(|tab| {
                tab.id() == tab_id && tab.space_id() == space_id && tab.profile_id() == profile_id
            })
        }) {
            return;
        }

        self.active_tabs_by_space_profile.remove(&key);
        if let Some(tab_id) = self
            .tabs
            .iter()
            .find(|tab| tab.space_id() == space_id && tab.profile_id() == profile_id)
            .map(|tab| tab.id().clone())
        {
            self.active_tabs_by_space_profile.insert(key, tab_id);
        }
    }
}

fn tab_is_idle(tab: &BrowserTab, now: SystemTime, idle_after: Duration) -> bool {
    now.duration_since(tab.last_active_at()).is_ok_and(|idle_time| idle_time >= idle_after)
}
