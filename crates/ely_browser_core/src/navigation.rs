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
        "ely://site-compatibility" => Some("Site Compatibility"),
        "ely://plugins" => Some("Plugin Marketplace"),
        url if auth_callback_route(url) => Some("Auth Callback"),
        url if crash_route_tab_id(url).is_some() => Some("Tab Recovery"),
        url if plugin_detail_route_id(url).is_some() => Some("Plugin Details"),
        url if SiteOrigin::from_site_route(url).ok().flatten().is_some() => Some("Site Settings"),
        "ely://about" => Some("About ELY Browser"),
        "ely://settings" => Some("Settings"),
        "ely://settings/advanced" => Some("Advanced Settings"),
        "ely://settings/general" => Some("General Settings"),
        "ely://settings/appearance" => Some("Appearance Settings"),
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
        "ely://settings/updates" => Some("Update Settings"),
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

pub(crate) fn rename_tab_group_name(command: &str) -> Option<&str> {
    command_argument(
        command,
        &["rename-tab-group ", "rename tab group ", "rename-group ", "rename group "],
    )
}

pub(crate) fn tab_group_color_hex(command: &str) -> Option<u32> {
    let value = command_argument(
        command,
        &["set-tab-group-color ", "set tab group color ", "tab-group-color ", "tab group color "],
    )?;
    parse_color_hex(value)
}

pub(crate) fn split_group_name(command: &str) -> Option<&str> {
    command_argument(
        command,
        &["group-split-view ", "group split view ", "split-view-to-group ", "split view to group "],
    )
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

fn parse_color_hex(value: &str) -> Option<u32> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if value.len() != 6 || !value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return None;
    }

    u32::from_str_radix(value, 16).ok()
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

pub(crate) fn site_compatibility_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://site-compatibility")
}

pub(crate) fn plugins_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://plugins")
}

pub(crate) fn plugin_settings_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://settings/plugins")
}

pub(crate) fn space_settings_url() -> Result<UrlText, CoreError> {
    internal_page_url("ely://settings/spaces")
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
    let Some(url) = settings_page_route(query) else {
        return Ok(None);
    };

    internal_page_url(url).map(Some)
}

struct SettingsRouteMatch {
    route: &'static str,
    exact_terms: &'static [&'static str],
    search_terms: &'static [&'static str],
}

impl SettingsRouteMatch {
    fn exact_match(&self, query: &str) -> bool {
        self.exact_terms.iter().any(|term| normalize_settings_query(term) == query)
    }

    fn search_match(&self, query: &str) -> bool {
        self.search_terms.iter().any(|term| settings_term_matches(query, term))
    }
}

const SETTINGS_ROUTE_MATCHES: &[SettingsRouteMatch] = &[
    SettingsRouteMatch {
        route: "ely://settings",
        exact_terms: &["settings"],
        search_terms: &["Settings center", "all browser settings"],
    },
    SettingsRouteMatch {
        route: "ely://settings/advanced",
        exact_terms: &[
            "advanced",
            "advanced settings",
            "runtime",
            "diagnostics",
            "diagnostic",
            "compatibility",
            "site compatibility",
        ],
        search_terms: &[
            "Advanced",
            "Local runtime policies and audit counters.",
            "runtime policy",
            "audit counters",
            "diagnostics",
            "site compatibility",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/general",
        exact_terms: &["general", "browser", "new tab", "new-tab", "startup"],
        search_terms: &[
            "General",
            "New Tab destination and browser startup defaults.",
            "default new tab",
            "startup defaults",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/appearance",
        exact_terms: &["appearance", "theme", "visual", "design", "colors"],
        search_terms: &[
            "Appearance",
            "Theme tokens, Space accent, and Profile color.",
            "theme tokens",
            "space accent",
            "profile color",
        ],
    },
    SettingsRouteMatch {
        route: "ely://about",
        exact_terms: &["about", "about ely browser"],
        search_terms: &["About", "Build, runtime, license, and product information."],
    },
    SettingsRouteMatch {
        route: "ely://settings/sidebar-tabs",
        exact_terms: &["sidebar", "tabs", "sidebar tabs", "sidebar & tabs"],
        search_terms: &[
            "Sidebar & Tabs",
            "Vertical tabs, pinned area, and auto archive policy.",
            "vertical tabs",
            "pinned area",
            "auto archive",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/search",
        exact_terms: &["search", "search engine", "default search", "default search engine"],
        search_terms: &[
            "Search",
            "Default search engine for Command Bar queries.",
            "command bar search",
            "query provider",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/privacy-security",
        exact_terms: &[
            "privacy",
            "security",
            "privacy security",
            "privacy & security",
            "history",
            "history recording",
        ],
        search_terms: &[
            "Privacy & Security",
            "History recording and profile-scoped privacy controls.",
            "profile privacy",
            "recording policy",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/downloads",
        exact_terms: &["download", "downloads", "download settings", "downloads settings"],
        search_terms: &[
            "Downloads",
            "Profile download location and save behavior.",
            "download location",
            "save behavior",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/spaces",
        exact_terms: &["space", "spaces", "space settings", "spaces settings"],
        search_terms: &[
            "Spaces",
            "Space identity, accent color, and active context.",
            "space identity",
            "active context",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/site-permissions",
        exact_terms: &[
            "site permission",
            "site permissions",
            "site permissions settings",
            "permissions",
            "permission settings",
        ],
        search_terms: &[
            "Site Permissions",
            "Profile-scoped site permissions and local audit state.",
            "profile scoped permissions",
            "local audit state",
            "storage persistence",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/shortcuts",
        exact_terms: &["shortcut", "shortcuts", "keyboard", "keyboard shortcuts"],
        search_terms: &[
            "Shortcuts",
            "Registered key bindings and conflict state.",
            "key bindings",
            "keyboard conflict",
            "cmd comma",
            "ctrl comma",
            "cmd shift p",
            "ctrl shift p",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/sync",
        exact_terms: &["sync", "sync settings"],
        search_terms: &["Sync", "Local sync state and object scope.", "sync object scope"],
    },
    SettingsRouteMatch {
        route: "ely://settings/updates",
        exact_terms: &["update", "updates", "auto update", "auto updates", "release", "releases"],
        search_terms: &[
            "Updates",
            "Build identity and Elydora release manifest contract.",
            "build identity",
            "release manifest",
            "artifact integrity",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/profiles",
        exact_terms: &["profile", "profiles", "profile settings", "profiles settings"],
        search_terms: &[
            "Profiles",
            "Profile identity, color, and download policy.",
            "profile identity",
            "profile download policy",
        ],
    },
    SettingsRouteMatch {
        route: "ely://settings/plugins",
        exact_terms: &["plugin", "plugins", "plugin settings", "plugins settings"],
        search_terms: &[
            "Plugins",
            "Installed plugin control and audit trail.",
            "installed plugin control",
            "plugin audit trail",
        ],
    },
];

fn settings_page_route(query: &str) -> Option<&'static str> {
    let normalized_query = normalize_settings_query(query);
    if normalized_query.is_empty() {
        return None;
    }

    SETTINGS_ROUTE_MATCHES
        .iter()
        .find(|route| route.exact_match(&normalized_query))
        .or_else(|| {
            SETTINGS_ROUTE_MATCHES.iter().find(|route| route.search_match(&normalized_query))
        })
        .map(|route| route.route)
}

fn settings_term_matches(query: &str, term: &str) -> bool {
    let normalized_term = normalize_settings_query(term);
    query == normalized_term || query.split_whitespace().all(|part| normalized_term.contains(part))
}

fn normalize_settings_query(value: &str) -> String {
    value
        .chars()
        .map(
            |character| {
                if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { ' ' }
            },
        )
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn internal_page_url(value: &str) -> Result<UrlText, CoreError> {
    UrlText::parse(value).map_err(CoreError::from)
}

fn plugin_detail_route_id(url: &str) -> Option<&str> {
    let plugin_id = url.strip_prefix("ely://plugin/")?;
    (!plugin_id.is_empty() && !plugin_id.contains('/')).then_some(plugin_id)
}

pub(crate) fn crash_route_tab_id(url: &str) -> Option<&str> {
    let tab_id = url.strip_prefix("ely://crash/")?;
    (!tab_id.is_empty() && !tab_id.contains('/')).then_some(tab_id)
}

fn auth_callback_route(url: &str) -> bool {
    matches!(url, "ely://auth/callback") || url.starts_with("ely://auth/callback?")
}
