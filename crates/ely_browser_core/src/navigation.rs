use ely_domain::{
    BrowserTab, DomainError, PluginId, ReadingProgressPercent, SearchEngine, SiteOrigin, UrlText,
};
use url::Url;

use crate::CoreError;

pub(crate) fn tab_title(url: &UrlText) -> String {
    if let Some(title) = internal_page_title(url.as_str()) {
        return title.to_string();
    }

    url.display_host()
}

fn internal_page_title(url: &str) -> Option<&'static str> {
    match url {
        "ely://new-tab" => Some("New Tab"),
        "ely://bookmarks" => Some("Bookmarks"),
        "ely://notes" => Some("Notes"),
        "ely://reading-list" => Some("Reading List"),
        "ely://downloads" => Some("Downloads"),
        "ely://history" => Some("History"),
        "ely://archive" => Some("Archived Tabs"),
        "ely://task-manager" => Some("Task Manager"),
        "ely://plugins" => Some("Plugin Marketplace"),
        url if plugin_detail_route_id(url).is_some() => Some("Plugin Details"),
        url if SiteOrigin::from_site_route(url).ok().flatten().is_some() => Some("Site Settings"),
        "ely://about" => Some("About ELY Browser"),
        "ely://settings" => Some("Settings"),
        "ely://settings/general" => Some("General Settings"),
        "ely://settings/sidebar-tabs" => Some("Sidebar & Tabs Settings"),
        "ely://settings/search" => Some("Search Settings"),
        "ely://settings/privacy-security" => Some("Privacy & Security Settings"),
        "ely://settings/downloads" => Some("Downloads Settings"),
        "ely://settings/spaces" => Some("Space Settings"),
        "ely://settings/site-permissions" => Some("Site Permissions Settings"),
        "ely://settings/shortcuts" => Some("Shortcut Settings"),
        "ely://settings/plugins" => Some("Plugin Settings"),
        "ely://settings/profiles" => Some("Profile Settings"),
        "ely://settings/sync" => Some("Sync Settings"),
        "ely://sync/status" => Some("Sync Status"),
        _ => None,
    }
}

pub(crate) fn tab_matches_query(tab: &BrowserTab, normalized_query: &str) -> bool {
    tab.title().to_lowercase().contains(normalized_query)
        || tab.url().as_str().to_lowercase().contains(normalized_query)
        || tab.display_url().to_lowercase().contains(normalized_query)
}

pub(crate) fn records_history(url: &UrlText) -> bool {
    Url::parse(url.as_str()).map(|parsed_url| parsed_url.scheme() != "ely").unwrap_or(false)
}

pub(crate) fn new_space_name(command: &str) -> Option<&str> {
    command_argument(command, &["new-space ", "new space "])
}

pub(crate) fn move_tab_space_name(command: &str) -> Option<&str> {
    command_argument(
        command,
        &["move-tab ", "move tab ", "move-tab-to-space ", "move tab to space "],
    )
}

pub(crate) fn archive_idle_days(command: &str) -> Option<u16> {
    command_argument(command, &["archive-idle-tabs ", "archive idle tabs "])
        .and_then(|value| value.parse().ok())
}

pub(crate) fn reading_progress_percent(
    command: &str,
) -> Result<Option<ReadingProgressPercent>, CoreError> {
    let Some(value) = command_argument(
        command,
        &[
            "reading-progress ",
            "reading progress ",
            "set-reading-progress ",
            "set reading progress ",
        ],
    ) else {
        return Ok(None);
    };

    let percent_text = value.trim().strip_suffix('%').unwrap_or(value.trim()).trim();
    let percent = percent_text.parse::<u8>().map_err(|_| {
        DomainError::InvalidReadingProgressPercent { value: value.trim().to_string() }
    })?;
    ReadingProgressPercent::new(percent).map(Some).map_err(CoreError::from)
}

pub(crate) fn note_body(command: &str) -> Option<&str> {
    command_argument(command, &["note ", "add-note ", "add note "])
}

pub(crate) fn tab_note_body(command: &str) -> Option<&str> {
    command_argument(command, &["tab-note ", "tab note ", "note-tab ", "note tab "])
}

pub(crate) fn tab_group_name(command: &str) -> Option<&str> {
    command_argument(command, &["group-tab ", "group tab ", "tab-group ", "tab group "])
}

pub(crate) fn new_profile_name(command: &str) -> Option<&str> {
    command_argument(command, &["new-profile ", "new profile "])
}

pub(crate) fn new_private_profile_name(command: &str) -> Option<&str> {
    command_argument(command, &["new-private-profile ", "new private profile "])
}

pub(crate) fn switch_profile_name(command: &str) -> Option<&str> {
    command_argument(command, &["switch-profile ", "switch profile "])
}

fn command_argument<'a>(command: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    let normalized_command = command.to_ascii_lowercase();
    for prefix in prefixes {
        if normalized_command.starts_with(prefix) {
            let name = command[prefix.len()..].trim();
            return (!name.is_empty()).then_some(name);
        }
    }
    None
}

pub(crate) fn space_icon(name: &str) -> String {
    name.chars().next().map_or_else(String::new, |value| value.to_string())
}

pub(crate) fn search_url(query: &str, search_engine: SearchEngine) -> Result<UrlText, CoreError> {
    search_engine.search_url(query).map_err(CoreError::from)
}

pub(crate) fn downloads_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://downloads")
}

pub(crate) fn bookmarks_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://bookmarks")
}

pub(crate) fn reading_list_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://reading-list")
}

pub(crate) fn notes_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://notes")
}

pub(crate) fn history_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://history")
}

pub(crate) fn archive_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://archive")
}

pub(crate) fn task_manager_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://task-manager")
}

pub(crate) fn plugins_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://plugins")
}

pub(crate) fn plugin_detail_url(plugin_id: &PluginId) -> Result<UrlText, CoreError> {
    let route = format!("ely://plugin/{}", plugin_id.as_str());
    internal_page_url(&route)
}

pub(crate) fn about_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://about")
}

pub(crate) fn settings_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://settings")
}

pub(crate) fn shortcut_settings_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://settings/shortcuts")
}

pub(crate) fn sync_status_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://sync/status")
}

pub(crate) fn settings_page_url(query: &str) -> Result<Option<UrlText>, CoreError> {
    let normalized_query = query.trim().to_ascii_lowercase();
    let Some(url) = settings_page_route(&normalized_query) else {
        return Ok(None);
    };

    internal_page_url(url).map(Some)
}

fn settings_page_route(query: &str) -> Option<&'static str> {
    match query {
        "settings" => Some("ely://settings"),
        "general" | "browser" | "new tab" | "new-tab" | "startup" => Some("ely://settings/general"),
        "about" | "about ely browser" => Some("ely://about"),
        "sidebar" | "tabs" | "sidebar tabs" | "sidebar & tabs" => {
            Some("ely://settings/sidebar-tabs")
        }
        "search" | "search engine" | "default search" | "default search engine" => {
            Some("ely://settings/search")
        }
        "privacy" | "security" | "privacy security" | "privacy & security" | "history"
        | "history recording" => Some("ely://settings/privacy-security"),
        "download" | "downloads" | "download settings" | "downloads settings" => {
            Some("ely://settings/downloads")
        }
        "space" | "spaces" | "space settings" | "spaces settings" => Some("ely://settings/spaces"),
        "site permission"
        | "site permissions"
        | "site permissions settings"
        | "permissions"
        | "permission settings" => Some("ely://settings/site-permissions"),
        "shortcut" | "shortcuts" | "keyboard" | "keyboard shortcuts" => {
            Some("ely://settings/shortcuts")
        }
        "sync" | "sync settings" => Some("ely://settings/sync"),
        "profile" | "profiles" | "profile settings" | "profiles settings" => {
            Some("ely://settings/profiles")
        }
        "plugin" | "plugins" | "plugin settings" | "plugins settings" => {
            Some("ely://settings/plugins")
        }
        _ => None,
    }
}

fn internal_page_url(value: &str) -> Result<UrlText, CoreError> {
    UrlText::parse(value).map_err(CoreError::from)
}

fn plugin_detail_route_id(url: &str) -> Option<&str> {
    let plugin_id = url.strip_prefix("ely://plugin/")?;
    (!plugin_id.is_empty() && !plugin_id.contains('/')).then_some(plugin_id)
}
