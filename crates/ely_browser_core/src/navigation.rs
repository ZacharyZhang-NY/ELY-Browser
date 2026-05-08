use ely_domain::{BrowserTab, DomainError, UrlText};
use url::Url;

use crate::CoreError;

const DEFAULT_SEARCH_URL: &str = "https://duckduckgo.com/";

pub(crate) fn tab_title(url: &UrlText) -> String {
    if let Some(title) = internal_page_title(url.as_str()) {
        return title.to_string();
    }

    url.display_host()
}

fn internal_page_title(url: &str) -> Option<&'static str> {
    match url {
        "ely://new-tab" => Some("New Tab"),
        "ely://downloads" => Some("Downloads"),
        "ely://history" => Some("History"),
        "ely://archive" => Some("Archived Tabs"),
        "ely://settings" => Some("Settings"),
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

pub(crate) fn new_profile_name(command: &str) -> Option<&str> {
    command_argument(command, &["new-profile ", "new profile "])
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

pub(crate) fn search_url(query: &str) -> Result<UrlText, CoreError> {
    let mut url = Url::parse(DEFAULT_SEARCH_URL)
        .map_err(|_| DomainError::InvalidUrl { value: DEFAULT_SEARCH_URL.to_string() })?;
    url.query_pairs_mut().append_pair("q", query);
    UrlText::parse(url.to_string()).map_err(CoreError::from)
}

pub(crate) fn downloads_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://downloads")
}

pub(crate) fn history_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://history")
}

pub(crate) fn settings_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://settings")
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
        "settings" | "general" | "browser" => Some("ely://settings"),
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
