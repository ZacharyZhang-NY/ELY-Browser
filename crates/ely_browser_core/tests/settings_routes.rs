use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope};

#[test]
fn settings_scoped_search_opens_sidebar_tabs_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings sidebar tabs");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "sidebar tabs".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Sidebar & Tabs Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/sidebar-tabs");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_general_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings general");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "general".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "General Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/general");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_appearance_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings appearance");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "appearance".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Appearance Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/appearance");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_shortcuts_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings shortcuts");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "shortcuts".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Shortcut Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/shortcuts");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_spaces_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings spaces");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "spaces".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Space Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/spaces");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_search_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings search");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "search".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Search Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/search");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_privacy_security_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings privacy");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "privacy".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Privacy & Security Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/privacy-security");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_downloads_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings downloads");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "downloads".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Downloads Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/downloads");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_site_permissions_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings site permissions");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "site permissions".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Site Permissions Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/site-permissions");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_matches_setting_description_terms() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings sync object scope");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "sync object scope".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Sync Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/sync");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_matches_setting_keywords() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings profile scoped permissions");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "profile scoped permissions".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Site Permissions Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/site-permissions");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_matches_shortcut_keywords() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings cmd comma");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "cmd comma".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Shortcut Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/shortcuts");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_prefers_exact_route_terms() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings profile");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "profile".to_string(),
        })
    );
    assert_eq!(active_tab.title(), "Profile Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/profiles");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
