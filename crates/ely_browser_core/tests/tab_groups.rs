use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{CommandIntent, UrlText};

#[test]
fn group_active_tab_creates_visible_space_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    let group_id = core.group_active_tab("Research")?;
    let snapshot = core.snapshot()?;
    let active_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;

    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(snapshot.tab_groups[0].id(), &group_id);
    assert_eq!(snapshot.tab_groups[0].name(), "Research");
    assert_eq!(snapshot.tab_groups[0].space_id(), &snapshot.active_space_id);
    assert_eq!(active_tab.group_id(), Some(&group_id));
    Ok(())
}

#[test]
fn group_active_tab_reuses_matching_space_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_group_id = core.group_active_tab("Research")?;

    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let second_group_id = core.group_active_tab("research")?;
    let snapshot = core.snapshot()?;
    let grouped_count =
        snapshot.tabs.iter().filter(|tab| tab.group_id() == Some(&first_group_id)).count();

    assert_eq!(first_group_id, second_group_id);
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(grouped_count, 2);
    assert_eq!(snapshot.active_tab_id, second_tab_id);
    Ok(())
}

#[test]
fn tab_groups_stay_with_active_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let group_id = core.group_active_tab("Research")?;
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;

    let research_snapshot = core.snapshot()?;
    assert_eq!(research_snapshot.active_space_id, research_space_id);
    assert!(research_snapshot.tab_groups.is_empty());

    core.select_space(&work_space_id)?;
    let work_snapshot = core.snapshot()?;

    assert_eq!(work_snapshot.tab_groups.len(), 1);
    assert_eq!(work_snapshot.tab_groups[0].id(), &group_id);
    Ok(())
}

#[test]
fn moving_grouped_tab_to_another_space_clears_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    core.select_space(&work_space_id)?;
    let moved_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    core.group_active_tab("Docs")?;

    core.move_active_tab_to_space(&research_space_id)?;
    let snapshot = core.snapshot()?;
    let moved_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &moved_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;

    assert_eq!(snapshot.active_space_id, research_space_id);
    assert_eq!(moved_tab.group_id(), None);
    Ok(())
}

#[test]
fn group_tab_command_groups_active_tab() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">group-tab Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;

    assert_eq!(intent, Some(CommandIntent::Command("group-tab Research".to_string())));
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(snapshot.tab_groups[0].name(), "Research");
    assert_eq!(active_tab.group_id(), Some(snapshot.tab_groups[0].id()));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}
