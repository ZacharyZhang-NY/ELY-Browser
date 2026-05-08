use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope, ProfileKind, UrlText};

#[test]
fn favorite_command_toggles_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">favorite");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("favorite".to_string())));
    assert_eq!(snapshot.favorites.len(), 1);
    assert_eq!(snapshot.favorites[0].id(), &snapshot.active_tab_id);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn pin_command_toggles_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">pin");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("pin".to_string())));
    assert_eq!(snapshot.pinned_tabs.len(), 1);
    assert_eq!(snapshot.pinned_tabs[0].id(), &snapshot.active_tab_id);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn new_tab_command_opens_new_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">new-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("new-tab".to_string())));
    assert_eq!(snapshot.tabs.len(), 2);
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

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

#[test]
fn settings_scoped_search_opens_about_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings about");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "about".to_string()
        })
    );
    assert_eq!(active_tab.title(), "About ELY Browser");
    assert_eq!(active_tab.url().as_str(), "ely://about");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_opens_matching_settings_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query("@settings sync");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "sync".to_string()
        })
    );
    assert_eq!(active_tab.title(), "Sync Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/sync");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn settings_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query("@settings absent");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Settings,
            query: "absent".to_string()
        })
    );
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, "@settings absent");
    Ok(())
}

#[test]
fn new_space_command_creates_and_selects_named_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">new-space Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("new-space Research".to_string())));
    assert_eq!(snapshot.spaces.len(), 2);
    assert_eq!(snapshot.active_space_name, "Research");
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].space_id(), &snapshot.active_space_id);
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn new_profile_command_creates_and_selects_named_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();

    core.set_command_query(">new-profile Personal");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("new-profile Personal".to_string())));
    assert_eq!(snapshot.profiles.len(), 2);
    assert_eq!(snapshot.active_profile_name, "Personal");
    assert_eq!(snapshot.active_profile_id, active_tab.profile_id().clone());
    assert_ne!(active_tab.profile_id(), &default_profile_id);
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn switch_profile_command_selects_matching_profile_context() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    let personal_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    core.select_profile(&default_profile_id)?;

    core.set_command_query(">switch-profile Personal");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("switch-profile Personal".to_string())));
    assert_eq!(snapshot.active_profile_name, "Personal");
    assert_eq!(snapshot.active_tab_id, personal_tab_id);
    assert_eq!(active_tab.profile_id(), &personal_profile_id);
    assert_eq!(active_tab.url().as_str(), "https://example.com");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn switch_profile_command_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">switch-profile Missing");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("switch-profile Missing".to_string())));
    assert_eq!(snapshot.active_profile_name, "Default");
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, ">switch-profile Missing");
    Ok(())
}

#[test]
fn spaces_scoped_search_selects_matching_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    core.open_tab(UrlText::parse("https://servo.org")?);
    core.select_space(&work_space_id)?;

    core.set_command_query("@spaces Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Spaces,
            query: "Research".to_string()
        })
    );
    assert_eq!(snapshot.active_space_id, research_space_id);
    assert_eq!(snapshot.active_space_name, "Research");
    assert_eq!(active_tab.url().as_str(), "https://servo.org");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn spaces_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_space_id = core.snapshot()?.active_space_id;

    core.set_command_query("@spaces absent");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::Spaces,
            query: "absent".to_string()
        })
    );
    assert_eq!(snapshot.active_space_id, active_space_id);
    assert_eq!(snapshot.command_query, "@spaces absent");
    Ok(())
}

#[test]
fn move_tab_command_moves_active_tab_to_named_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    core.select_space(&work_space_id)?;
    let moved_tab_id = core.open_tab(UrlText::parse("https://example.com")?);

    core.set_command_query(">move-tab Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("move-tab Research".to_string())));
    assert_eq!(snapshot.active_space_id, research_space_id);
    assert_eq!(snapshot.active_tab_id, moved_tab_id);
    assert_eq!(active_tab.space_id(), &snapshot.active_space_id);
    assert_eq!(active_tab.url().as_str(), "https://example.com");
    assert_eq!(snapshot.command_query, "");

    core.select_space(&work_space_id)?;
    let work_snapshot = core.snapshot()?;
    assert!(work_snapshot.tabs.iter().all(|tab| tab.id() != &moved_tab_id));
    Ok(())
}

#[test]
fn move_tab_command_preserves_query_without_matching_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let active_space_id = core.snapshot()?.active_space_id;

    core.set_command_query(">move-tab Missing");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("move-tab Missing".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.active_space_id, active_space_id);
    assert_eq!(snapshot.command_query, ">move-tab Missing");
    Ok(())
}

#[test]
fn close_tab_command_closes_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);

    core.set_command_query(">close-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("close-tab".to_string())));
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.active_tab_id, first_tab_id);
    assert!(snapshot.tabs.iter().all(|tab| tab.id() != &second_tab_id));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn restore_tab_command_reopens_last_archived_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let closed_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    core.close_active_tab()?;

    core.set_command_query(">restore-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("restore-tab".to_string())));
    assert_eq!(snapshot.active_tab_id, closed_tab_id);
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn archive_scoped_search_restores_matching_archived_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com")?);
    let servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);
    core.close_active_tab()?;

    core.set_command_query("@archive servo");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: ely_domain::CommandScope::Archive,
            query: "servo".to_string()
        })
    );
    assert_eq!(snapshot.active_tab_id, servo_tab_id);
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn archive_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com")?);
    core.close_active_tab()?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query("@archive absent");
    core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.command_query, "@archive absent");
    Ok(())
}

#[test]
fn unknown_command_preserves_query() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">missing");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("missing".to_string())));
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, ">missing");
    Ok(())
}
