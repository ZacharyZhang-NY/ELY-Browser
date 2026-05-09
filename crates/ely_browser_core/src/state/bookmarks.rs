use std::{collections::BTreeSet, time::SystemTime};

use ely_domain::{BookmarkEntry, BookmarkId, SpaceId, UrlText};
use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::BrowserCore;

pub const ELYBOOKMARKS_SCHEMA_VERSION: u16 = 1;
pub const ELYBOOKMARKS_FILE_EXTENSION: &str = "elybookmarks";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElyBookmarksPackage {
    version: u16,
    bookmarks: Vec<ElyBookmarkRecord>,
}

impl ElyBookmarksPackage {
    #[must_use]
    pub fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn bookmark_count(&self) -> usize {
        self.bookmarks.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ElyBookmarkRecord {
    title: String,
    url: String,
    collection_name: String,
    space_name: String,
    tags: Vec<String>,
    note: Option<String>,
    thumbnail_key: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BookmarkImportSummary {
    imported: usize,
    skipped: usize,
}

impl BookmarkImportSummary {
    #[must_use]
    pub fn imported(self) -> usize {
        self.imported
    }

    #[must_use]
    pub fn skipped(self) -> usize {
        self.skipped
    }

    #[must_use]
    pub fn label(self) -> String {
        format!("Imported {} bookmarks, skipped {}", self.imported, self.skipped)
    }
}

impl BrowserCore {
    pub fn bookmark_active_tab(&mut self) -> Result<BookmarkId, CoreError> {
        let active_tab = self.active_tab()?.clone();
        if let Some(bookmark) = self.bookmarks.iter().find(|bookmark| {
            bookmark.profile_id() == active_tab.profile_id()
                && bookmark.space_id() == active_tab.space_id()
                && bookmark.url() == active_tab.url()
        }) {
            return Ok(bookmark.id().clone());
        }

        let collection_name = self.active_space()?.name().to_string();
        let bookmark = BookmarkEntry::new(
            active_tab.profile_id().clone(),
            active_tab.space_id().clone(),
            collection_name,
            active_tab.title(),
            active_tab.url().clone(),
            SystemTime::now(),
        )?;
        let bookmark_id = bookmark.id().clone();
        self.bookmarks.push(bookmark);
        Ok(bookmark_id)
    }

    pub fn set_bookmark_collection_name(
        &mut self,
        bookmark_id: &BookmarkId,
        collection_name: impl Into<String>,
    ) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.set_collection_name(collection_name)?;
        Ok(())
    }

    pub fn set_bookmark_tags(
        &mut self,
        bookmark_id: &BookmarkId,
        tags: Vec<String>,
    ) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.set_tags(tags)?;
        Ok(())
    }

    pub fn set_bookmark_note(
        &mut self,
        bookmark_id: &BookmarkId,
        note: impl Into<String>,
    ) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.set_note(note)?;
        Ok(())
    }

    pub fn clear_bookmark_note(&mut self, bookmark_id: &BookmarkId) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.clear_note();
        Ok(())
    }

    pub fn set_bookmark_thumbnail_key(
        &mut self,
        bookmark_id: &BookmarkId,
        thumbnail_key: impl Into<String>,
    ) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.set_thumbnail_key(thumbnail_key)?;
        Ok(())
    }

    pub fn clear_bookmark_thumbnail_key(
        &mut self,
        bookmark_id: &BookmarkId,
    ) -> Result<(), CoreError> {
        self.bookmark_mut(bookmark_id)?.clear_thumbnail_key();
        Ok(())
    }

    pub fn update_bookmark_metadata(
        &mut self,
        bookmark_id: &BookmarkId,
        collection_name: impl Into<String>,
        tags: Vec<String>,
        note: Option<String>,
    ) -> Result<(), CoreError> {
        let bookmark = self.bookmark_mut(bookmark_id)?;
        let mut updated_bookmark = bookmark.clone();

        updated_bookmark.set_collection_name(collection_name)?;
        updated_bookmark.set_tags(tags)?;
        if let Some(note) = note {
            updated_bookmark.set_note(note)?;
        } else {
            updated_bookmark.clear_note();
        }

        *bookmark = updated_bookmark;
        Ok(())
    }

    pub fn export_bookmarks_package_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&self.export_bookmarks_package()?)
            .map_err(invalid_bookmark_package)
    }

    pub fn export_bookmarks_package(&self) -> Result<ElyBookmarksPackage, CoreError> {
        let bookmarks = self
            .bookmarks
            .iter()
            .filter(|bookmark| bookmark.profile_id() == &self.active_profile_id)
            .map(|bookmark| {
                self.bookmark_space_name(bookmark.space_id())
                    .map(|space_name| ElyBookmarkRecord::from_bookmark(bookmark, space_name))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ElyBookmarksPackage { version: ELYBOOKMARKS_SCHEMA_VERSION, bookmarks })
    }

    pub fn import_bookmarks_package_json(
        &mut self,
        package_json: &str,
    ) -> Result<BookmarkImportSummary, CoreError> {
        let package = serde_json::from_str(package_json).map_err(invalid_bookmark_package)?;
        self.import_bookmarks_package(package)
    }

    pub fn import_bookmarks_package(
        &mut self,
        package: ElyBookmarksPackage,
    ) -> Result<BookmarkImportSummary, CoreError> {
        validate_bookmark_package_version(package.version)?;

        let active_profile_id = self.active_profile_id.clone();
        let mut seen = self
            .bookmarks
            .iter()
            .filter(|bookmark| bookmark.profile_id() == &active_profile_id)
            .map(|bookmark| bookmark_identity(bookmark.space_id(), bookmark.url()))
            .collect::<BTreeSet<_>>();
        let mut imported_bookmarks = Vec::new();
        let mut skipped = 0;

        for record in package.bookmarks {
            let space_id = self.import_bookmark_space_id(&record.space_name);
            let url = UrlText::parse(&record.url)?;
            let identity = bookmark_identity(&space_id, &url);
            if !seen.insert(identity) {
                skipped += 1;
                continue;
            }

            imported_bookmarks.push(record.into_bookmark(active_profile_id.clone(), space_id)?);
        }

        let imported = imported_bookmarks.len();
        self.bookmarks.extend(imported_bookmarks);
        Ok(BookmarkImportSummary { imported, skipped })
    }

    pub(super) fn find_bookmark_match(&self, query: &str) -> Option<UrlText> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return None;
        }

        self.bookmarks
            .iter()
            .rev()
            .filter(|bookmark| bookmark.profile_id() == &self.active_profile_id)
            .find(|bookmark| bookmark_matches_query(bookmark, &normalized_query))
            .map(|bookmark| bookmark.url().clone())
    }

    pub(super) fn visible_bookmarks(&self) -> Vec<BookmarkEntry> {
        self.bookmarks
            .iter()
            .filter(|bookmark| bookmark.profile_id() == &self.active_profile_id)
            .cloned()
            .collect()
    }

    fn bookmark_mut(&mut self, bookmark_id: &BookmarkId) -> Result<&mut BookmarkEntry, CoreError> {
        self.bookmarks
            .iter_mut()
            .find(|bookmark| bookmark.id() == bookmark_id)
            .ok_or_else(|| CoreError::BookmarkNotFound { id: bookmark_id.clone() })
    }

    fn bookmark_space_name(&self, space_id: &SpaceId) -> Result<String, CoreError> {
        self.spaces
            .iter()
            .find(|space| space.id() == space_id)
            .map(|space| space.name().to_string())
            .ok_or_else(|| CoreError::SpaceNotFound { id: space_id.clone() })
    }

    fn import_bookmark_space_id(&self, space_name: &str) -> SpaceId {
        self.spaces
            .iter()
            .find(|space| space.name().eq_ignore_ascii_case(space_name.trim()))
            .map_or_else(|| self.active_space_id.clone(), |space| space.id().clone())
    }
}

fn bookmark_matches_query(bookmark: &BookmarkEntry, normalized_query: &str) -> bool {
    bookmark.title().to_lowercase().contains(normalized_query)
        || bookmark.url().as_str().to_lowercase().contains(normalized_query)
        || bookmark.display_url().to_lowercase().contains(normalized_query)
        || bookmark.collection_name().to_lowercase().contains(normalized_query)
        || bookmark.tags().iter().any(|tag| tag.to_lowercase().contains(normalized_query))
        || bookmark.note().is_some_and(|note| note.to_lowercase().contains(normalized_query))
}

impl ElyBookmarkRecord {
    fn from_bookmark(bookmark: &BookmarkEntry, space_name: String) -> Self {
        Self {
            title: bookmark.title().to_string(),
            url: bookmark.url().as_str().to_string(),
            collection_name: bookmark.collection_name().to_string(),
            space_name,
            tags: bookmark.tags().to_vec(),
            note: bookmark.note().map(str::to_string),
            thumbnail_key: bookmark.thumbnail_key().map(str::to_string),
        }
    }

    fn into_bookmark(
        self,
        profile_id: ely_domain::ProfileId,
        space_id: SpaceId,
    ) -> Result<BookmarkEntry, CoreError> {
        let mut bookmark = BookmarkEntry::new(
            profile_id,
            space_id,
            self.collection_name,
            self.title,
            UrlText::parse(self.url)?,
            SystemTime::now(),
        )?;
        bookmark.set_tags(self.tags)?;
        if let Some(note) = self.note {
            bookmark.set_note(note)?;
        }
        if let Some(thumbnail_key) = self.thumbnail_key {
            bookmark.set_thumbnail_key(thumbnail_key)?;
        }
        Ok(bookmark)
    }
}

fn bookmark_identity(space_id: &SpaceId, url: &UrlText) -> (SpaceId, String) {
    (space_id.clone(), url.as_str().to_string())
}

fn validate_bookmark_package_version(version: u16) -> Result<(), CoreError> {
    if version == ELYBOOKMARKS_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(CoreError::InvalidBookmarkPackage { reason: format!("unsupported version {version}") })
    }
}

fn invalid_bookmark_package(error: impl ToString) -> CoreError {
    CoreError::InvalidBookmarkPackage { reason: error.to_string() }
}
