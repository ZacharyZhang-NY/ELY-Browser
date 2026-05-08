use std::time::SystemTime;

use crate::{ProfileId, SpaceId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    profile_id: ProfileId,
    space_id: SpaceId,
    title: String,
    url: UrlText,
    visited_at: SystemTime,
    visit_count: u32,
}

impl HistoryEntry {
    #[must_use]
    pub fn new(
        profile_id: ProfileId,
        space_id: SpaceId,
        title: impl Into<String>,
        url: UrlText,
        visited_at: SystemTime,
    ) -> Self {
        Self { profile_id, space_id, title: title.into(), url, visited_at, visit_count: 1 }
    }

    pub fn record_visit(&mut self, title: impl Into<String>, visited_at: SystemTime) {
        self.title = title.into();
        self.visited_at = visited_at;
        self.visit_count = self.visit_count.saturating_add(1);
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
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn url(&self) -> &UrlText {
        &self.url
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
