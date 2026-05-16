use std::time::SystemTime;

use crate::{BookmarkId, DomainError, ProfileId, SpaceId, UrlText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkEntry {
    id: BookmarkId,
    profile_id: ProfileId,
    space_id: SpaceId,
    collection_name: String,
    title: String,
    url: UrlText,
    tags: Vec<String>,
    note: Option<String>,
    thumbnail_key: Option<String>,
    added_at: SystemTime,
}

impl BookmarkEntry {
    pub fn new(
        profile_id: ProfileId,
        space_id: SpaceId,
        collection_name: impl Into<String>,
        title: impl Into<String>,
        url: UrlText,
        added_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let collection_name = non_empty_text("bookmark collection", collection_name.into())?;
        let title = non_empty_text("bookmark title", title.into())?;

        Ok(Self {
            id: BookmarkId::new(),
            profile_id,
            space_id,
            collection_name,
            title,
            url,
            tags: Vec::new(),
            note: None,
            thumbnail_key: None,
            added_at,
        })
    }

    pub fn restore(
        id: BookmarkId,
        profile_id: ProfileId,
        space_id: SpaceId,
        collection_name: impl Into<String>,
        title: impl Into<String>,
        url: UrlText,
        added_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let collection_name = non_empty_text("bookmark collection", collection_name.into())?;
        let title = non_empty_text("bookmark title", title.into())?;

        Ok(Self {
            id,
            profile_id,
            space_id,
            collection_name,
            title,
            url,
            tags: Vec::new(),
            note: None,
            thumbnail_key: None,
            added_at,
        })
    }

    #[must_use]
    pub fn id(&self) -> &BookmarkId {
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
    pub fn collection_name(&self) -> &str {
        &self.collection_name
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
    pub fn display_url(&self) -> String {
        self.url.display_url()
    }

    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    #[must_use]
    pub fn thumbnail_key(&self) -> Option<&str> {
        self.thumbnail_key.as_deref()
    }

    #[must_use]
    pub fn added_at(&self) -> SystemTime {
        self.added_at
    }

    pub fn set_collection_name(
        &mut self,
        collection_name: impl Into<String>,
    ) -> Result<(), DomainError> {
        self.collection_name = non_empty_text("bookmark collection", collection_name.into())?;
        Ok(())
    }

    pub fn set_tags(&mut self, tags: Vec<String>) -> Result<(), DomainError> {
        self.tags = normalize_tags(tags)?;
        Ok(())
    }

    pub fn set_note(&mut self, note: impl Into<String>) -> Result<(), DomainError> {
        self.note = Some(non_empty_text("bookmark note", note.into())?);
        Ok(())
    }

    pub fn clear_note(&mut self) {
        self.note = None;
    }

    pub fn set_thumbnail_key(
        &mut self,
        thumbnail_key: impl Into<String>,
    ) -> Result<(), DomainError> {
        self.thumbnail_key = Some(non_empty_text("bookmark thumbnail key", thumbnail_key.into())?);
        Ok(())
    }

    pub fn clear_thumbnail_key(&mut self) {
        self.thumbnail_key = None;
    }
}

fn non_empty_text(field: &'static str, value: String) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(trimmed.to_string())
}

fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, DomainError> {
    tags.into_iter().map(|tag| non_empty_text("bookmark tag", tag)).collect()
}
