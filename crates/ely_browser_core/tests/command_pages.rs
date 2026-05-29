use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::CommandIntent;

#[test]
fn open_downloads_command_opens_downloads_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-downloads");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-downloads".to_string())));
    assert_eq!(snapshot.tabs.len(), 2);
    assert_eq!(active_tab.title(), "Downloads");
    assert_eq!(active_tab.url().as_str(), "ely://downloads");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn download_current_page_command_keeps_active_tab_for_shell_download() -> Result<(), Box<dyn Error>>
{
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">download-current-page");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("download-current-page".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn open_history_command_opens_history_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-history");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-history".to_string())));
    assert_eq!(active_tab.title(), "History");
    assert_eq!(active_tab.url().as_str(), "ely://history");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_task_manager_command_opens_task_manager_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-task-manager");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-task-manager".to_string())));
    assert_eq!(active_tab.title(), "Task Manager");
    assert_eq!(active_tab.url().as_str(), "ely://task-manager");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_plugins_command_opens_plugin_marketplace_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">plugins");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("plugins".to_string())));
    assert_eq!(active_tab.title(), "Plugin Marketplace");
    assert_eq!(active_tab.url().as_str(), "ely://plugins");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_about_command_opens_about_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">about");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("about".to_string())));
    assert_eq!(active_tab.title(), "About ELY Browser");
    assert_eq!(active_tab.url().as_str(), "ely://about");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_settings_command_opens_settings_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-settings");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-settings".to_string())));
    assert_eq!(active_tab.title(), "Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_shortcuts_command_opens_shortcuts_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-shortcuts");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-shortcuts".to_string())));
    assert_eq!(active_tab.title(), "Shortcut Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/shortcuts");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn open_sync_status_command_opens_sync_status_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">open-sync-status");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("open-sync-status".to_string())));
    assert_eq!(active_tab.title(), "Sync Status");
    assert_eq!(active_tab.url().as_str(), "ely://sync/status");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}
