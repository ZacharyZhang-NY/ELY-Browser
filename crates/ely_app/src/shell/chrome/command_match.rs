use ely_browser_core::BrowserSnapshot;
use ely_domain::{BookmarkEntry, BrowserTab, HistoryEntry};

pub(crate) const RESULT_LIMIT: usize = 4;

pub(crate) fn matching_tabs<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a BrowserTab> {
    if needle.is_empty() {
        return snapshot.tabs.iter().take(RESULT_LIMIT).collect();
    }

    snapshot
        .tabs
        .iter()
        .filter(|tab| matches_tab(tab, needle))
        .take(RESULT_LIMIT)
        .collect()
}

fn matches_tab(tab: &BrowserTab, needle: &str) -> bool {
    tab.title().to_lowercase().contains(needle)
        || tab.url().as_str().to_lowercase().contains(needle)
}

pub(crate) fn matching_history<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a HistoryEntry> {
    if needle.is_empty() {
        return snapshot.history_entries.iter().rev().take(RESULT_LIMIT).collect();
    }

    snapshot
        .history_entries
        .iter()
        .rev()
        .filter(|entry| matches_history(entry, needle))
        .take(RESULT_LIMIT)
        .collect()
}

fn matches_history(entry: &HistoryEntry, needle: &str) -> bool {
    entry.title().to_lowercase().contains(needle)
        || entry.url().as_str().to_lowercase().contains(needle)
}

pub(crate) fn matching_bookmarks<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a BookmarkEntry> {
    if needle.is_empty() {
        return snapshot.bookmarks.iter().take(RESULT_LIMIT).collect();
    }

    snapshot
        .bookmarks
        .iter()
        .filter(|bookmark| matches_bookmark(bookmark, needle))
        .take(RESULT_LIMIT)
        .collect()
}

fn matches_bookmark(bookmark: &BookmarkEntry, needle: &str) -> bool {
    bookmark.title().to_lowercase().contains(needle)
        || bookmark.url().as_str().to_lowercase().contains(needle)
}
