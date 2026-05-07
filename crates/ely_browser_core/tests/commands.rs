use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, UrlText};

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
