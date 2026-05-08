use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{CommandIntent, MAX_SPLIT_PANES, SplitAxis, UrlText};

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

#[test]
fn auto_group_active_space_tabs_by_domain_groups_matching_hosts() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.open_tab(UrlText::parse("https://example.com/a")?);
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com/b")?);
    let singleton_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    let grouped_count = core.auto_group_active_space_tabs_by_domain()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing domain tab group")?;

    assert_eq!(grouped_count, 2);
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(group.name(), "example.com");
    assert_eq!(group.space_id(), &snapshot.active_space_id);
    assert_eq!(tab_group_id(&snapshot, &first_tab_id), Some(group.id()));
    assert_eq!(tab_group_id(&snapshot, &second_tab_id), Some(group.id()));
    assert_eq!(tab_group_id(&snapshot, &singleton_tab_id), None);
    Ok(())
}

#[test]
fn auto_group_domain_command_groups_matching_hosts() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.open_tab(UrlText::parse("https://example.com/one")?);
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com/two")?);

    core.set_command_query(">auto-group-domains");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing domain tab group")?;

    assert_eq!(intent, Some(CommandIntent::Command("auto-group-domains".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(group.name(), "example.com");
    assert_eq!(tab_group_id(&snapshot, &first_tab_id), Some(group.id()));
    assert_eq!(tab_group_id(&snapshot, &second_tab_id), Some(group.id()));
    Ok(())
}

#[test]
fn auto_group_domains_preserves_existing_manual_groups() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let manual_tab_id = core.open_tab(UrlText::parse("https://example.com/manual")?);
    let manual_group_id = core.group_active_tab("Manual")?;
    let first_auto_tab_id = core.open_tab(UrlText::parse("https://example.com/auto-a")?);
    let second_auto_tab_id = core.open_tab(UrlText::parse("https://example.com/auto-b")?);

    let grouped_count = core.auto_group_active_space_tabs_by_domain()?;
    let snapshot = core.snapshot()?;
    let domain_group = snapshot
        .tab_groups
        .iter()
        .find(|group| group.name() == "example.com")
        .ok_or("missing domain tab group")?;

    assert_eq!(grouped_count, 2);
    assert_eq!(tab_group_id(&snapshot, &manual_tab_id), Some(&manual_group_id));
    assert_eq!(tab_group_id(&snapshot, &first_auto_tab_id), Some(domain_group.id()));
    assert_eq!(tab_group_id(&snapshot, &second_auto_tab_id), Some(domain_group.id()));
    Ok(())
}

#[test]
fn tab_group_collapse_commands_update_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let group_id = core.group_active_tab("Research")?;

    core.set_command_query(">collapse-tab-group");
    let collapse_intent = core.submit_command()?;
    let collapsed_snapshot = core.snapshot()?;

    assert_eq!(collapse_intent, Some(CommandIntent::Command("collapse-tab-group".to_string())));
    assert_eq!(
        collapsed_snapshot
            .tab_groups
            .iter()
            .find(|group| group.id() == &group_id)
            .map(|group| { group.collapsed() }),
        Some(true)
    );
    assert_eq!(collapsed_snapshot.command_query, "");

    core.set_command_query(">expand-tab-group");
    let expand_intent = core.submit_command()?;
    let expanded_snapshot = core.snapshot()?;

    assert_eq!(expand_intent, Some(CommandIntent::Command("expand-tab-group".to_string())));
    assert_eq!(
        expanded_snapshot
            .tab_groups
            .iter()
            .find(|group| group.id() == &group_id)
            .map(|group| { group.collapsed() }),
        Some(false)
    );
    assert_eq!(expanded_snapshot.command_query, "");
    Ok(())
}

#[test]
fn toggle_active_tab_group_collapsed_returns_next_state() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Research")?;

    assert_eq!(core.toggle_active_tab_group_collapsed()?, Some(true));
    assert_eq!(core.toggle_active_tab_group_collapsed()?, Some(false));
    Ok(())
}

#[test]
fn ungroup_tab_command_clears_active_tab_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Research")?;

    core.set_command_query(">ungroup-tab");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &snapshot.active_tab_id)
        .ok_or(CoreError::MissingActiveTab)?;

    assert_eq!(intent, Some(CommandIntent::Command("ungroup-tab".to_string())));
    assert_eq!(active_tab.group_id(), None);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn tab_group_command_preserves_query_without_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">toggle-tab-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("toggle-tab-group".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, ">toggle-tab-group");
    Ok(())
}

#[test]
fn split_tab_group_command_converts_active_group_to_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    core.group_active_tab("Research")?;
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    core.group_active_tab("research")?;

    core.set_command_query(">split-tab-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot.split_layouts.first().ok_or("missing split layout")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id().clone()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("split-tab-group".to_string())));
    assert_eq!(snapshot.active_tab_id, second_tab_id);
    assert_eq!(snapshot.command_query, "");
    assert!(snapshot.tab_groups.is_empty());
    assert_eq!(layout.axis(), &SplitAxis::Horizontal);
    assert_eq!(pane_ids, vec![first_tab_id, second_tab_id]);
    assert!(snapshot.tabs.iter().all(|tab| tab.group_id().is_none()));
    assert!(snapshot.tabs.iter().all(|tab| tab.split_id() == Some(layout.id())));
    Ok(())
}

#[test]
fn tab_group_to_split_alias_converts_active_group_to_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Research")?;
    core.open_tab(UrlText::parse("https://example.com")?);
    core.group_active_tab("Research")?;

    core.set_command_query(">tab group to split");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("tab group to split".to_string())));
    assert!(snapshot.tab_groups.is_empty());
    assert_eq!(snapshot.split_layouts.len(), 1);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn split_tab_group_command_preserves_query_for_single_tab_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.group_active_tab("Research")?;

    core.set_command_query(">split-tab-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("split-tab-group".to_string())));
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(snapshot.command_query, ">split-tab-group");
    Ok(())
}

#[test]
fn split_tab_group_rejects_groups_above_pane_limit() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    for index in 0..=MAX_SPLIT_PANES {
        if index > 0 {
            core.open_tab(UrlText::parse(format!("https://example.com/{index}"))?);
        }
        core.group_active_tab("Research")?;
    }

    let error = match core.split_active_tab_group() {
        Err(error) => error,
        Ok(_) => return Err("oversized tab group was converted to a split view".into()),
    };

    assert_eq!(error, CoreError::SplitPaneLimitReached { limit: MAX_SPLIT_PANES });
    Ok(())
}

fn tab_group_id<'a>(
    snapshot: &'a ely_browser_core::BrowserSnapshot,
    tab_id: &ely_domain::TabId,
) -> Option<&'a ely_domain::TabGroupId> {
    snapshot.tabs.iter().find(|tab| tab.id() == tab_id).and_then(|tab| tab.group_id())
}
