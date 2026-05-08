use std::{collections::BTreeSet, error::Error};

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, TabGroupId, UrlText};

#[test]
fn rename_tab_group_command_updates_active_group_name() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.open_tab(UrlText::parse("https://example.com/a")?);
    core.group_active_tab("Docs")?;
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com/b")?);
    core.group_active_tab("Docs")?;

    core.set_command_query(">rename-tab-group Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing tab group")?;

    assert_eq!(intent, Some(CommandIntent::Command("rename-tab-group Research".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(group.name(), "Research");
    assert_eq!(tab_group_id(&snapshot, &first_tab_id), Some(group.id()));
    assert_eq!(tab_group_id(&snapshot, &second_tab_id), Some(group.id()));
    Ok(())
}

#[test]
fn rename_tab_group_command_preserves_query_without_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">rename-tab-group Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("rename-tab-group Research".to_string())));
    assert_eq!(snapshot.command_query, ">rename-tab-group Research");
    assert!(snapshot.tab_groups.is_empty());
    Ok(())
}

#[test]
fn tab_group_color_command_updates_active_group_color() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Docs")?;

    core.set_command_query(">tab-group-color #9FC9A2");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing tab group")?;

    assert_eq!(intent, Some(CommandIntent::Command("tab-group-color #9FC9A2".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(group.color_hex(), 0x9fc9a2);
    Ok(())
}

#[test]
fn tab_group_color_command_preserves_query_without_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">tab-group-color #9FC9A2");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("tab-group-color #9FC9A2".to_string())));
    assert_eq!(snapshot.command_query, ">tab-group-color #9FC9A2");
    assert!(snapshot.tab_groups.is_empty());
    Ok(())
}

#[test]
fn tab_group_color_command_preserves_query_for_invalid_hex() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let group_id = core.group_active_tab("Docs")?;

    core.set_command_query(">tab-group-color orange");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot
        .tab_groups
        .iter()
        .find(|group| group.id() == &group_id)
        .ok_or("missing tab group")?;

    assert_eq!(intent, Some(CommandIntent::Command("tab-group-color orange".to_string())));
    assert_eq!(snapshot.command_query, ">tab-group-color orange");
    assert_eq!(group.color_hex(), 0xf54e00);
    Ok(())
}

#[test]
fn move_tab_group_commands_reorder_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Docs")?;
    core.open_tab(UrlText::parse("https://example.com")?);
    core.group_active_tab("Research")?;

    core.set_command_query(">move-tab-group-up");
    let up_intent = core.submit_command()?;
    let up_snapshot = core.snapshot()?;

    assert_eq!(up_intent, Some(CommandIntent::Command("move-tab-group-up".to_string())));
    assert_eq!(up_snapshot.command_query, "");
    assert_eq!(tab_group_names(&up_snapshot), vec!["Research", "Docs"]);
    assert_eq!(sidebar_group_names(&up_snapshot), vec!["Research", "Docs"]);

    core.set_command_query(">move-tab-group-down");
    let down_intent = core.submit_command()?;
    let down_snapshot = core.snapshot()?;

    assert_eq!(down_intent, Some(CommandIntent::Command("move-tab-group-down".to_string())));
    assert_eq!(down_snapshot.command_query, "");
    assert_eq!(tab_group_names(&down_snapshot), vec!["Docs", "Research"]);
    assert_eq!(sidebar_group_names(&down_snapshot), vec!["Docs", "Research"]);
    Ok(())
}

#[test]
fn move_tab_group_command_preserves_query_at_boundary() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    core.group_active_tab("Docs")?;
    core.open_tab(UrlText::parse("https://example.com")?);
    core.group_active_tab("Research")?;
    core.select_tab(&first_tab_id)?;

    core.set_command_query(">move-tab-group-up");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("move-tab-group-up".to_string())));
    assert_eq!(snapshot.command_query, ">move-tab-group-up");
    assert_eq!(tab_group_names(&snapshot), vec!["Docs", "Research"]);
    Ok(())
}

#[test]
fn move_tab_group_command_preserves_query_without_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">move-tab-group-up");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("move-tab-group-up".to_string())));
    assert_eq!(snapshot.command_query, ">move-tab-group-up");
    assert!(snapshot.tab_groups.is_empty());
    Ok(())
}

fn tab_group_id<'a>(
    snapshot: &'a ely_browser_core::BrowserSnapshot,
    tab_id: &ely_domain::TabId,
) -> Option<&'a TabGroupId> {
    snapshot.tabs.iter().find(|tab| tab.id() == tab_id).and_then(|tab| tab.group_id())
}

fn tab_group_names(snapshot: &ely_browser_core::BrowserSnapshot) -> Vec<&str> {
    snapshot.tab_groups.iter().map(|group| group.name()).collect()
}

fn sidebar_group_names(snapshot: &ely_browser_core::BrowserSnapshot) -> Vec<&str> {
    let mut rendered_group_ids = BTreeSet::new();
    let mut names = Vec::new();

    for tab in &snapshot.tabs {
        let Some(group_id) = tab.group_id() else {
            continue;
        };
        if !rendered_group_ids.insert(group_id.clone()) {
            continue;
        }
        if let Some(group) = snapshot.tab_groups.iter().find(|group| group.id() == group_id) {
            names.push(group.name());
        }
    }

    names
}
