use std::time::SystemTime;

use crate::{DomainError, NoteId, ProfileId, SpaceId, TabId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteTarget {
    Url(UrlText),
    Tab(TabId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteEntry {
    id: NoteId,
    profile_id: ProfileId,
    space_id: SpaceId,
    target: NoteTarget,
    title: String,
    source_url: UrlText,
    body: String,
    created_at: SystemTime,
    updated_at: SystemTime,
}

impl NoteEntry {
    pub fn new(
        profile_id: ProfileId,
        space_id: SpaceId,
        target: NoteTarget,
        title: impl Into<String>,
        source_url: UrlText,
        body: impl Into<String>,
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        let title = non_empty_text("note title", title.into())?;
        let body = normalize_body(body.into())?;

        Ok(Self {
            id: NoteId::new(),
            profile_id,
            space_id,
            target,
            title,
            source_url,
            body,
            created_at: now,
            updated_at: now,
        })
    }

    #[must_use]
    pub fn restore(id: NoteId, mut entry: Self, updated_at: SystemTime) -> Self {
        entry.id = id;
        entry.updated_at = updated_at;
        entry
    }

    #[must_use]
    pub fn id(&self) -> &NoteId {
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
    pub fn target(&self) -> &NoteTarget {
        &self.target
    }

    #[must_use]
    pub fn target_label(&self) -> &'static str {
        match &self.target {
            NoteTarget::Url(_) => "URL note",
            NoteTarget::Tab(_) => "Tab note",
        }
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
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    pub fn update(
        &mut self,
        title: impl Into<String>,
        source_url: UrlText,
        body: impl Into<String>,
        updated_at: SystemTime,
    ) -> Result<(), DomainError> {
        self.title = non_empty_text("note title", title.into())?;
        self.source_url = source_url;
        self.body = normalize_body(body.into())?;
        self.updated_at = updated_at;
        Ok(())
    }
}

fn normalize_body(value: String) -> Result<String, DomainError> {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    non_empty_text("note body", normalized)
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
    use std::time::{Duration, SystemTime};

    use super::{NoteEntry, NoteTarget};
    use crate::{DomainError, ProfileId, SpaceId, TabId, UrlText};

    #[test]
    fn creates_url_note_with_markdown_body() -> Result<(), DomainError> {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let source_url = UrlText::parse("https://example.com/article")?;
        let note = NoteEntry::new(
            ProfileId::new(),
            SpaceId::new(),
            NoteTarget::Url(source_url.clone()),
            "Example",
            source_url,
            " # Heading\r\n- item ",
            now,
        )?;

        assert_eq!(note.title(), "Example");
        assert_eq!(note.body(), "# Heading\n- item");
        assert_eq!(note.target_label(), "URL note");
        assert_eq!(note.created_at(), now);
        assert_eq!(note.updated_at(), now);
        Ok(())
    }

    #[test]
    fn creates_tab_note_target() -> Result<(), DomainError> {
        let tab_id = TabId::new();
        let note = NoteEntry::new(
            ProfileId::new(),
            SpaceId::new(),
            NoteTarget::Tab(tab_id.clone()),
            "Example",
            UrlText::parse("https://example.com/tab")?,
            "- detail",
            SystemTime::UNIX_EPOCH,
        )?;

        assert_eq!(note.target(), &NoteTarget::Tab(tab_id));
        assert_eq!(note.target_label(), "Tab note");
        Ok(())
    }

    #[test]
    fn restores_existing_note_identity_and_timestamps() -> Result<(), DomainError> {
        let id = crate::NoteId::new();
        let profile_id = ProfileId::new();
        let space_id = SpaceId::new();
        let created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let updated_at = created_at + Duration::from_secs(20);
        let source_url = UrlText::parse("https://example.com/restored")?;
        let note = NoteEntry::new(
            profile_id.clone(),
            space_id.clone(),
            NoteTarget::Url(source_url.clone()),
            " Restored ",
            source_url,
            " body\r\ncopy ",
            created_at,
        )?;
        let note = NoteEntry::restore(id.clone(), note, updated_at);

        assert_eq!(note.id(), &id);
        assert_eq!(note.profile_id(), &profile_id);
        assert_eq!(note.space_id(), &space_id);
        assert_eq!(note.title(), "Restored");
        assert_eq!(note.body(), "body\ncopy");
        assert_eq!(note.created_at(), created_at);
        assert_eq!(note.updated_at(), updated_at);
        Ok(())
    }

    #[test]
    fn rejects_empty_body() -> Result<(), DomainError> {
        let source_url = UrlText::parse("https://example.com")?;
        let result = NoteEntry::new(
            ProfileId::new(),
            SpaceId::new(),
            NoteTarget::Url(source_url.clone()),
            "Example",
            source_url,
            "  ",
            SystemTime::UNIX_EPOCH,
        );

        assert_eq!(result, Err(DomainError::EmptyField { field: "note body" }));
        Ok(())
    }

    #[test]
    fn updates_body_and_timestamp() -> Result<(), DomainError> {
        let created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let updated_at = created_at + Duration::from_secs(30);
        let mut note = NoteEntry::new(
            ProfileId::new(),
            SpaceId::new(),
            NoteTarget::Url(UrlText::parse("https://example.com/old")?),
            "Old",
            UrlText::parse("https://example.com/old")?,
            "first",
            created_at,
        )?;

        note.update(
            "New",
            UrlText::parse("https://example.com/new")?,
            " second\rline ",
            updated_at,
        )?;

        assert_eq!(note.title(), "New");
        assert_eq!(note.source_url().as_str(), "https://example.com/new");
        assert_eq!(note.body(), "second\nline");
        assert_eq!(note.created_at(), created_at);
        assert_eq!(note.updated_at(), updated_at);
        Ok(())
    }
}
