use std::time::SystemTime;

use ely_domain::{BookmarkEntry, BookmarkId, UrlText};

use crate::CoreError;

use super::BrowserCore;

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
}

fn bookmark_matches_query(bookmark: &BookmarkEntry, normalized_query: &str) -> bool {
    bookmark.title().to_lowercase().contains(normalized_query)
        || bookmark.url().as_str().to_lowercase().contains(normalized_query)
        || bookmark.display_url().to_lowercase().contains(normalized_query)
        || bookmark.collection_name().to_lowercase().contains(normalized_query)
        || bookmark.tags().iter().any(|tag| tag.to_lowercase().contains(normalized_query))
        || bookmark.note().is_some_and(|note| note.to_lowercase().contains(normalized_query))
}
