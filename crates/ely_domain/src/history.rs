use std::{num::NonZeroU32, time::SystemTime};

use crate::{ProfileId, SpaceId, TabId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    profile_id: ProfileId,
    space_id: SpaceId,
    source_tab_id: TabId,
    title: String,
    url: UrlText,
    favicon_key: Option<String>,
    visited_at: SystemTime,
    visit_count: u32,
}

impl HistoryEntry {
    #[must_use]
    pub fn new(
        profile_id: ProfileId,
        space_id: SpaceId,
        source_tab_id: TabId,
        title: impl Into<String>,
        url: UrlText,
        favicon_key: Option<String>,
        visited_at: SystemTime,
    ) -> Self {
        Self {
            profile_id,
            space_id,
            source_tab_id,
            title: title.into(),
            url,
            favicon_key,
            visited_at,
            visit_count: 1,
        }
    }

    pub fn record_visit(
        &mut self,
        source_tab_id: TabId,
        title: impl Into<String>,
        favicon_key: Option<&str>,
        visited_at: SystemTime,
    ) {
        self.source_tab_id = source_tab_id;
        self.title = title.into();
        if let Some(favicon_key) = favicon_key {
            self.favicon_key = Some(favicon_key.to_string());
        }
        self.visited_at = visited_at;
        self.visit_count = self.visit_count.saturating_add(1);
    }

    #[must_use]
    pub fn restore(mut entry: Self, visit_count: NonZeroU32) -> Self {
        entry.visit_count = visit_count.get();
        entry
    }

    #[must_use]
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[must_use]
    pub fn space_id(&self) -> &SpaceId {
        &self.space_id
    }

    #[must_use]
    pub fn source_tab_id(&self) -> &TabId {
        &self.source_tab_id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn url(&self) -> &UrlText {
        &self.url
    }

    #[must_use]
    pub fn favicon_key(&self) -> Option<&str> {
        self.favicon_key.as_deref()
    }

    pub fn set_favicon_key(&mut self, favicon_key: impl Into<String>) {
        self.favicon_key = Some(favicon_key.into());
    }

    pub fn clear_favicon_key(&mut self) {
        self.favicon_key = None;
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    #[must_use]
    pub fn visited_at(&self) -> SystemTime {
        self.visited_at
    }

    #[must_use]
    pub fn visit_count(&self) -> u32 {
        self.visit_count
    }
}
