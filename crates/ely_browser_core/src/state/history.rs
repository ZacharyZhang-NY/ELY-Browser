use std::time::SystemTime;

use ely_domain::{BrowserTab, HistoryEntry, ProfileId, ProfileKind, SpaceId, UrlText};

use crate::navigation::records_history;

use super::BrowserCore;

impl BrowserCore {
    pub fn clear_active_profile_history(&mut self) -> Result<usize, crate::CoreError> {
        let profile_id = self.active_profile_id.clone();
        self.clear_profile_history(&profile_id)
    }

    pub fn clear_active_space_history_for_host(
        &mut self,
        host: &str,
    ) -> Result<usize, crate::CoreError> {
        let profile_id = self.active_profile_id.clone();
        let space_id = self.active_space_id.clone();
        self.clear_space_profile_history_for_host(&profile_id, &space_id, host)
    }

    pub fn clear_active_space_history_since(
        &mut self,
        cutoff: SystemTime,
    ) -> Result<usize, crate::CoreError> {
        let profile_id = self.active_profile_id.clone();
        let space_id = self.active_space_id.clone();
        self.clear_space_profile_history_since(&profile_id, &space_id, cutoff)
    }

    pub(super) fn record_history_entry(&mut self, tab: &BrowserTab) {
        if !self.history_recording_policy.records_history()
            || !records_history(tab.url())
            || !self.profile_records_history(tab.profile_id())
        {
            return;
        }

        let visited_at = SystemTime::now();
        if let Some(entry) = self.history_entries.iter_mut().find(|entry| {
            entry.profile_id() == tab.profile_id()
                && entry.space_id() == tab.space_id()
                && entry.url() == tab.url()
        }) {
            entry.record_visit(tab.id().clone(), tab.title(), tab.favicon_key(), visited_at);
            return;
        }

        self.history_entries.push(HistoryEntry::new(
            tab.profile_id().clone(),
            tab.space_id().clone(),
            tab.id().clone(),
            tab.title(),
            tab.url().clone(),
            tab.favicon_key().map(ToOwned::to_owned),
            visited_at,
        ));
    }

    pub(super) fn set_history_favicon_key_for_tab(
        &mut self,
        tab: &BrowserTab,
        favicon_key: impl Into<String>,
    ) {
        let favicon_key = favicon_key.into();
        if let Some(entry) = self.history_entries.iter_mut().find(|entry| {
            entry.profile_id() == tab.profile_id()
                && entry.space_id() == tab.space_id()
                && entry.url() == tab.url()
        }) {
            entry.set_favicon_key(favicon_key);
        }
    }

    pub(super) fn clear_history_favicon_key_for_tab(&mut self, tab: &BrowserTab) {
        if let Some(entry) = self.history_entries.iter_mut().find(|entry| {
            entry.profile_id() == tab.profile_id()
                && entry.space_id() == tab.space_id()
                && entry.url() == tab.url()
        }) {
            entry.clear_favicon_key();
        }
    }

    pub(super) fn find_history_match(&self, query: &str) -> Option<UrlText> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return None;
        }

        self.history_entries
            .iter()
            .rev()
            .find(|entry| {
                entry.profile_id() == &self.active_profile_id
                    && entry.space_id() == &self.active_space_id
                    && history_entry_matches_query(entry, &normalized_query)
            })
            .map(|entry| entry.url().clone())
    }

    pub(super) fn visible_history(&self) -> Vec<HistoryEntry> {
        self.history_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .filter(|entry| entry.space_id() == &self.active_space_id)
            .cloned()
            .collect()
    }

    pub(super) fn active_profile_history_count(&self) -> usize {
        self.history_entries
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .count()
    }

    fn clear_profile_history(&mut self, profile_id: &ProfileId) -> Result<usize, crate::CoreError> {
        if !self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Err(crate::CoreError::ProfileNotFound { id: profile_id.clone() });
        }

        let original_count = self.history_entries.len();
        self.history_entries.retain(|entry| entry.profile_id() != profile_id);
        Ok(original_count - self.history_entries.len())
    }

    fn clear_space_profile_history_for_host(
        &mut self,
        profile_id: &ProfileId,
        space_id: &SpaceId,
        host: &str,
    ) -> Result<usize, crate::CoreError> {
        if !self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Err(crate::CoreError::ProfileNotFound { id: profile_id.clone() });
        }
        if !self.spaces.iter().any(|space| space.id() == space_id) {
            return Err(crate::CoreError::SpaceNotFound { id: space_id.clone() });
        }

        let normalized_host = host.trim().to_ascii_lowercase();
        if normalized_host.is_empty() {
            return Ok(0);
        }

        let original_count = self.history_entries.len();
        self.history_entries.retain(|entry| {
            entry.profile_id() != profile_id
                || entry.space_id() != space_id
                || entry.url().host().as_deref() != Some(normalized_host.as_str())
        });
        Ok(original_count - self.history_entries.len())
    }

    fn clear_space_profile_history_since(
        &mut self,
        profile_id: &ProfileId,
        space_id: &SpaceId,
        cutoff: SystemTime,
    ) -> Result<usize, crate::CoreError> {
        if !self.profiles.iter().any(|profile| profile.id() == profile_id) {
            return Err(crate::CoreError::ProfileNotFound { id: profile_id.clone() });
        }
        if !self.spaces.iter().any(|space| space.id() == space_id) {
            return Err(crate::CoreError::SpaceNotFound { id: space_id.clone() });
        }

        let original_count = self.history_entries.len();
        self.history_entries.retain(|entry| {
            entry.profile_id() != profile_id
                || entry.space_id() != space_id
                || entry.visited_at() < cutoff
        });
        Ok(original_count - self.history_entries.len())
    }

    fn profile_records_history(&self, profile_id: &ProfileId) -> bool {
        match self.profiles.iter().find(|profile| profile.id() == profile_id) {
            Some(profile) => profile.kind() == &ProfileKind::Standard,
            None => false,
        }
    }
}

fn history_entry_matches_query(entry: &HistoryEntry, normalized_query: &str) -> bool {
    entry.title().to_lowercase().contains(normalized_query)
        || entry.url().as_str().to_lowercase().contains(normalized_query)
        || entry.url().display_url().to_lowercase().contains(normalized_query)
}
