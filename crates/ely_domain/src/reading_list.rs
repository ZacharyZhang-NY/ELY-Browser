use std::time::SystemTime;

use crate::{DomainError, ProfileId, ReadingListId, SpaceId, UrlText};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadingProgressPercent(u8);

impl ReadingProgressPercent {
    pub fn new(value: u8) -> Result<Self, DomainError> {
        if !(1..=99).contains(&value) {
            return Err(DomainError::InvalidReadingProgressPercent { value: value.to_string() });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingProgress {
    Unread,
    InProgress(ReadingProgressPercent),
    Finished,
}

impl ReadingProgress {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Unread => "Unread".to_string(),
            Self::InProgress(percent) => format!("{}% read", percent.value()),
            Self::Finished => "Read".to_string(),
        }
    }

    #[must_use]
    pub fn action_label(self) -> &'static str {
        match self {
            Self::Unread | Self::InProgress(_) => "Mark Read",
            Self::Finished => "Mark Unread",
        }
    }

    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Unread | Self::InProgress(_) => Self::Finished,
            Self::Finished => Self::Unread,
        }
    }
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
    pub fn restore(id: ReadingListId, mut entry: Self) -> Self {
        entry.id = id;
        entry
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

    pub fn set_progress(&mut self, progress: ReadingProgress) {
        self.progress = progress;
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

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::{ReadingListEntry, ReadingProgress, ReadingProgressPercent};
    use crate::{DomainError, ProfileId, ReadingListId, SpaceId, UrlText};

    #[test]
    fn reading_progress_percent_accepts_partial_progress() -> Result<(), DomainError> {
        let percent = ReadingProgressPercent::new(42)?;

        assert_eq!(percent.value(), 42);
        assert_eq!(ReadingProgress::InProgress(percent).label(), "42% read");
        Ok(())
    }

    #[test]
    fn reading_progress_percent_rejects_terminal_values() {
        assert_eq!(
            ReadingProgressPercent::new(0),
            Err(DomainError::InvalidReadingProgressPercent { value: "0".to_string() })
        );
        assert_eq!(
            ReadingProgressPercent::new(100),
            Err(DomainError::InvalidReadingProgressPercent { value: "100".to_string() })
        );
    }

    #[test]
    fn restores_existing_reading_list_identity() -> Result<(), DomainError> {
        let id = ReadingListId::new();
        let profile_id = ProfileId::new();
        let space_id = SpaceId::new();
        let added_at = SystemTime::UNIX_EPOCH;
        let entry = ReadingListEntry::new(
            profile_id.clone(),
            space_id.clone(),
            " Restored ",
            UrlText::parse("https://example.com/read")?,
            added_at,
        )?;
        let entry = ReadingListEntry::restore(id.clone(), entry);

        assert_eq!(entry.id(), &id);
        assert_eq!(entry.profile_id(), &profile_id);
        assert_eq!(entry.space_id(), &space_id);
        assert_eq!(entry.title(), "Restored");
        assert_eq!(entry.added_at(), added_at);
        Ok(())
    }
}
