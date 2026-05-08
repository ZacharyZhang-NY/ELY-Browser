use std::time::SystemTime;

use crate::{DomainError, ProfileId, ReadingListId, SpaceId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadingProgress {
    Unread,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingListEntry {
    id: ReadingListId,
    profile_id: ProfileId,
    space_id: SpaceId,
    title: String,
    source_url: UrlText,
    progress: ReadingProgress,
    added_at: SystemTime,
}

impl ReadingListEntry {
    pub fn new(
        profile_id: ProfileId,
        space_id: SpaceId,
        title: impl Into<String>,
        source_url: UrlText,
        added_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let title = non_empty_text("reading list title", title.into())?;

        Ok(Self {
            id: ReadingListId::new(),
            profile_id,
            space_id,
            title,
            source_url,
            progress: ReadingProgress::Unread,
            added_at,
        })
    }

    #[must_use]
    pub fn id(&self) -> &ReadingListId {
        &self.id
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
    pub fn source_url(&self) -> &UrlText {
        &self.source_url
    }

    #[must_use]
    pub fn display_url(&self) -> String {
        self.source_url.display_url()
    }

    #[must_use]
    pub fn progress(&self) -> &ReadingProgress {
        &self.progress
    }

    #[must_use]
    pub fn added_at(&self) -> SystemTime {
        self.added_at
    }
}

fn non_empty_text(field: &'static str, value: String) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(trimmed.to_string())
}
