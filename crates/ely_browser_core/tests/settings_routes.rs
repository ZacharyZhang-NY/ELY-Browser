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
