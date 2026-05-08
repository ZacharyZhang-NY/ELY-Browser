use std::time::SystemTime;

use ely_domain::{NoteEntry, NoteId, NoteTarget, UrlText};

use crate::CoreError;

use super::BrowserCore;

impl BrowserCore {
    pub fn save_active_url_note(&mut self, body: impl Into<String>) -> Result<NoteId, CoreError> {
        let active_tab = self.active_tab()?.clone();
        let now = SystemTime::now();
        let target = NoteTarget::Url(active_tab.url().clone());

        if let Some(index) = self.note_index_for_target(active_tab.profile_id(), &target) {
            self.notes[index].update(
                active_tab.title(),
                active_tab.url().clone(),
                body.into(),
                now,
            )?;
            return Ok(self.notes[index].id().clone());
        }

        let entry = NoteEntry::new(
            active_tab.profile_id().clone(),
            active_tab.space_id().clone(),
            target,
            active_tab.title(),
            active_tab.url().clone(),
            body,
            now,
        )?;
        let entry_id = entry.id().clone();
        self.notes.push(entry);
        Ok(entry_id)
    }

    pub fn save_active_tab_note(&mut self, body: impl Into<String>) -> Result<NoteId, CoreError> {
        let active_tab = self.active_tab()?.clone();
        let now = SystemTime::now();
        let target = NoteTarget::Tab(active_tab.id().clone());

        if let Some(index) = self.note_index_for_target(active_tab.profile_id(), &target) {
            self.notes[index].update(
                active_tab.title(),
                active_tab.url().clone(),
                body.into(),
                now,
            )?;
            return Ok(self.notes[index].id().clone());
        }

        let entry = NoteEntry::new(
            active_tab.profile_id().clone(),
            active_tab.space_id().clone(),
            target,
            active_tab.title(),
            active_tab.url().clone(),
            body,
            now,
        )?;
        let entry_id = entry.id().clone();
        self.notes.push(entry);
        Ok(entry_id)
    }

    pub(super) fn find_note_match(&self, query: &str) -> Option<UrlText> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return None;
        }

        self.notes
            .iter()
            .rev()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .find(|entry| note_entry_matches_query(entry, &normalized_query))
            .map(|entry| entry.source_url().clone())
    }

    pub(super) fn visible_notes(&self) -> Vec<NoteEntry> {
        self.notes
            .iter()
            .filter(|entry| entry.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }

    fn note_index_for_target(
        &self,
        profile_id: &ely_domain::ProfileId,
        target: &NoteTarget,
    ) -> Option<usize> {
        self.notes.iter().position(|entry| {
            entry.profile_id() == profile_id && note_targets_match(entry.target(), target)
        })
    }
}

fn note_targets_match(left: &NoteTarget, right: &NoteTarget) -> bool {
    match (left, right) {
        (NoteTarget::Url(left_url), NoteTarget::Url(right_url)) => left_url == right_url,
        (NoteTarget::Tab(left_tab), NoteTarget::Tab(right_tab)) => left_tab == right_tab,
        _ => false,
    }
}

fn note_entry_matches_query(entry: &NoteEntry, normalized_query: &str) -> bool {
    entry.title().to_lowercase().contains(normalized_query)
        || entry.source_url().as_str().to_lowercase().contains(normalized_query)
        || entry.display_url().to_lowercase().contains(normalized_query)
        || entry.body().to_lowercase().contains(normalized_query)
}
