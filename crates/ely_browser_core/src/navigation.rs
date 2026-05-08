use ely_domain::{BrowserTab, DomainError, UrlText};
use url::Url;

use crate::CoreError;

const DEFAULT_SEARCH_URL: &str = "https://duckduckgo.com/";

pub(crate) fn tab_title(url: &UrlText) -> String {
    if url.as_str() == "ely://new-tab" {
        return "New Tab".to_string();
    }

    url.display_host()
}

pub(crate) fn tab_matches_query(tab: &BrowserTab, normalized_query: &str) -> bool {
    tab.title().to_lowercase().contains(normalized_query)
        || tab.url().as_str().to_lowercase().contains(normalized_query)
        || tab.display_url().to_lowercase().contains(normalized_query)
}

pub(crate) fn new_space_name(command: &str) -> Option<&str> {
    let normalized_command = command.to_ascii_lowercase();
    for prefix in ["new-space ", "new space "] {
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
