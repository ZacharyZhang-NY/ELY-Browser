use ely_browser_core::BrowserSnapshot;
use ely_domain::{BookmarkEntry, BrowserTab, HistoryEntry, TabId, UrlText};
use gpui_component::IconName;

pub(crate) const RESULT_LIMIT: usize = 4;

pub(crate) struct CommandActionEntry {
    pub title: &'static str,
    pub hint: &'static str,
    pub icon: IconName,
    pub route: &'static str,
    pub keys: &'static str,
}

pub(crate) const COMMAND_ACTIONS: &[CommandActionEntry] = &[
    CommandActionEntry {
        title: "Switch workspace",
        hint: "Cycle to the next space",
        icon: IconName::LayoutDashboard,
        route: "ely://settings/spaces",
        keys: "⌘⇧W",
    },
    CommandActionEntry {
        title: "Open Settings",
        hint: "Appearance, sync, plugins…",
        icon: IconName::Settings,
        route: "ely://settings",
        keys: "⌘,",
    },
    CommandActionEntry {
        title: "Open Plugins",
        hint: "Marketplace and installed plugins",
        icon: IconName::Asterisk,
        route: "ely://plugins",
        keys: "",
    },
    CommandActionEntry {
        title: "Open Bookmarks",
        hint: "Manage your saved pages",
        icon: IconName::BookOpen,
        route: "ely://bookmarks",
        keys: "",
    },
];

pub(crate) fn matching_actions(needle: &str) -> Vec<&'static CommandActionEntry> {
    COMMAND_ACTIONS
        .iter()
        .filter(|action| {
            needle.is_empty()
                || action.title.to_lowercase().contains(needle)
                || action.hint.to_lowercase().contains(needle)
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) enum CommandSelection {
    SelectTab(TabId),
    OpenUrl(UrlText),
    OpenRoute(&'static str),
}

pub(crate) fn visible_command_rows(
    snapshot: &BrowserSnapshot,
    needle: &str,
) -> Vec<CommandSelection> {
    let mut rows = Vec::new();
    rows.extend(
        matching_tabs(snapshot, needle)
            .iter()
            .map(|tab| CommandSelection::SelectTab(tab.id().clone())),
    );
    rows.extend(
        matching_history(snapshot, needle)
            .iter()
            .map(|entry| CommandSelection::OpenUrl(entry.url().clone())),
    );
    rows.extend(
        matching_bookmarks(snapshot, needle)
            .iter()
            .map(|bookmark| CommandSelection::OpenUrl(bookmark.url().clone())),
    );
    rows.extend(
        matching_actions(needle).iter().map(|action| CommandSelection::OpenRoute(action.route)),
    );
    rows
}

pub(crate) fn matching_tabs<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a BrowserTab> {
    if needle.is_empty() {
        return snapshot.tabs.iter().take(RESULT_LIMIT).collect();
    }

    snapshot.tabs.iter().filter(|tab| matches_tab(tab, needle)).take(RESULT_LIMIT).collect()
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
