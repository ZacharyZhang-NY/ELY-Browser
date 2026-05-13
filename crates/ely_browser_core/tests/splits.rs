use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, SplitAxis, UrlText};

#[test]
fn split_right_creates_two_pane_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();

    let split_id = core.split_active_tab_right()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;

    assert_eq!(snapshot.tabs.len(), 2);
    assert_eq!(layout.axis(), &SplitAxis::Horizontal);
    assert_eq!(layout.pane_count(), 2);
    assert_eq!(layout.panes()[0].tab_id(), &first_tab_id);
    assert_eq!(layout.panes()[1].tab_id(), &snapshot.active_tab_id);
    assert!(snapshot.tabs.iter().all(|tab| tab.split_id() == Some(&split_id)));
    Ok(())
}

#[test]
fn visible_content_tab_ids_returns_active_tab_without_split() -> Result<(), Box<dyn Error>> {
    let core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    assert_eq!(core.visible_content_tab_ids()?, vec![active_tab_id]);
    Ok(())
}

#[test]
fn visible_content_tab_ids_returns_split_panes() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let split_id = core.split_active_tab_right()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id().clone()).collect::<Vec<_>>();

    assert_eq!(core.visible_content_tab_ids()?, pane_ids);
    Ok(())
}

#[test]
fn split_right_command_focuses_new_pane() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">split-right");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("split-right".to_string())));
    assert_eq!(snapshot.tabs.len(), 2);
    assert_eq!(snapshot.split_layouts.len(), 1);
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn save_split_view_command_marks_active_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;

    core.set_command_query(">save-split-view");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot.split_layouts.first().ok_or("missing split layout")?;

    assert_eq!(intent, Some(CommandIntent::Command("save-split-view".to_string())));
    assert!(layout.saved());
    assert_eq!(layout.title(), "Split View: New Tab + New Tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn save_split_view_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">save-split-view");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("save-split-view".to_string())));
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, ">save-split-view");
    Ok(())
}

#[test]
fn split_axis_commands_update_active_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;

    core.set_command_query(">split-vertical");
    let vertical_intent = core.submit_command()?;
    let vertical_snapshot = core.snapshot()?;
    let vertical_layout = vertical_snapshot.split_layouts.first().ok_or("missing split layout")?;

    assert_eq!(vertical_intent, Some(CommandIntent::Command("split-vertical".to_string())));
    assert_eq!(vertical_layout.axis(), &SplitAxis::Vertical);
    assert_eq!(vertical_snapshot.command_query, "");

    core.set_command_query(">split-grid");
    let grid_intent = core.submit_command()?;
    let grid_snapshot = core.snapshot()?;
    let grid_layout = grid_snapshot.split_layouts.first().ok_or("missing split layout")?;

    assert_eq!(grid_intent, Some(CommandIntent::Command("split-grid".to_string())));
    assert_eq!(grid_layout.axis(), &SplitAxis::Grid);
    assert_eq!(grid_snapshot.command_query, "");

    core.set_command_query(">split-horizontal");
    let horizontal_intent = core.submit_command()?;
    let horizontal_snapshot = core.snapshot()?;
    let horizontal_layout =
        horizontal_snapshot.split_layouts.first().ok_or("missing split layout")?;

    assert_eq!(horizontal_intent, Some(CommandIntent::Command("split-horizontal".to_string())));
    assert_eq!(horizontal_layout.axis(), &SplitAxis::Horizontal);
    assert_eq!(horizontal_snapshot.command_query, "");
    Ok(())
}

#[test]
fn split_axis_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">split-grid");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("split-grid".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.command_query, ">split-grid");
    Ok(())
}

#[test]
fn split_grid_command_keeps_four_pane_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;
    core.split_active_tab_right()?;
    core.split_active_tab_right()?;

    core.set_command_query(">split-grid");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot.split_layouts.first().ok_or("missing split layout")?;

    assert_eq!(intent, Some(CommandIntent::Command("split-grid".to_string())));
    assert_eq!(layout.axis(), &SplitAxis::Grid);
    assert_eq!(layout.pane_count(), 4);
    assert_eq!(snapshot.tabs.len(), 4);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn detach_split_pane_command_removes_active_pane_from_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let split_id = core.split_active_tab_right()?;
    core.split_active_tab_right()?;
    let detached_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">detach-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot.split_layouts.first().ok_or("missing split layout")?;
    let detached_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &detached_tab_id)
        .ok_or("missing detached tab")?;

    assert_eq!(intent, Some(CommandIntent::Command("detach-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, detached_tab_id);
    assert_eq!(detached_tab.split_id(), None);
    assert_eq!(layout.id(), &split_id);
    assert_eq!(layout.pane_count(), 2);
    assert!(!layout.panes().iter().any(|pane| pane.tab_id() == &detached_tab_id));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn detach_split_pane_command_dissolves_two_pane_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;
    let detached_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">detach-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("detach-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, detached_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert!(snapshot.tabs.iter().all(|tab| tab.split_id().is_none()));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn detach_split_pane_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">detach-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("detach-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.command_query, ">detach-split-pane");
    Ok(())
}

#[test]
fn detach_split_pane_refreshes_saved_split_title() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/docs")?);
    core.group_active_tab("Research")?;
    let detached_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);
    core.group_active_tab("Research")?;
    let split_id = core.split_active_tab_group()?.ok_or("missing split id")?;
    core.split_active_tab_right()?;
    core.save_active_split_view()?;

    core.select_tab(&detached_tab_id)?;
    core.detach_active_split_pane()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing saved split layout")?;
    let detached_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.id() == &detached_tab_id)
        .ok_or("missing detached tab")?;

    assert_eq!(detached_tab.split_id(), None);
    assert!(layout.saved());
    assert_eq!(layout.title(), "Split View: example.com + New Tab");
    assert_eq!(layout.pane_count(), 2);
    Ok(())
}

#[test]
fn close_active_tab_archives_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let remaining_tab_id = core.active_tab()?.id().clone();
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    core.split_active_tab_right()?;
    core.save_active_split_view()?;

    let active_tab_id = core.close_active_tab()?;
    let snapshot = core.snapshot()?;

    assert_eq!(active_tab_id, remaining_tab_id);
    assert_eq!(snapshot.active_tab_id, remaining_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs.len(), 2);
    let archived_split_id =
        snapshot.archived_tabs[0].tab().split_id().ok_or("missing archived split id")?;
    assert!(
        snapshot
            .archived_tabs
            .iter()
            .all(|archived| archived.tab().split_id() == Some(archived_split_id))
    );
    Ok(())
}

#[test]
fn close_saved_split_member_archives_entire_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let remaining_tab_id = core.active_tab()?.id().clone();
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    core.split_active_tab_right()?;
    core.save_active_split_view()?;
    let left_pane_id = core.snapshot()?.split_layouts[0].panes()[0].tab_id().clone();

    let active_tab_id = core.close_tab(&left_pane_id)?;
    let snapshot = core.snapshot()?;

    assert_eq!(active_tab_id, remaining_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs.len(), 2);
    Ok(())
}

#[test]
fn close_split_view_command_archives_active_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let remaining_tab_id = core.active_tab()?.id().clone();
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    core.split_active_tab_right()?;
    core.save_active_split_view()?;

    core.set_command_query(">close-split-view");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("close-split-view".to_string())));
    assert_eq!(snapshot.active_tab_id, remaining_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.archived_tabs.len(), 2);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn close_split_view_command_preserves_query_without_saved_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">close-split-view");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("close-split-view".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.split_layouts.len(), 1);
    assert_eq!(snapshot.command_query, ">close-split-view");
    Ok(())
}

#[test]
fn split_view_to_group_command_converts_active_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    core.split_active_tab_right()?;
    let second_tab_id = core.active_tab()?.id().clone();
    core.save_active_split_view()?;

    core.set_command_query(">split-view-to-group Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing tab group")?;
    let grouped_tab_ids = snapshot
        .tabs
        .iter()
        .filter(|tab| tab.group_id() == Some(group.id()))
        .map(|tab| tab.id().clone())
        .collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("split-view-to-group Research".to_string())));
    assert_eq!(snapshot.active_tab_id, second_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.tab_groups.len(), 1);
    assert_eq!(group.name(), "Research");
    assert_eq!(grouped_tab_ids, vec![first_tab_id, second_tab_id]);
    assert!(snapshot.tabs.iter().all(|tab| tab.split_id().is_none()));
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn split_view_to_group_command_uses_saved_split_title_without_name() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.split_active_tab_right()?;
    core.save_active_split_view()?;

    core.set_command_query(">split-view-to-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let group = snapshot.tab_groups.first().ok_or("missing tab group")?;

    assert_eq!(intent, Some(CommandIntent::Command("split-view-to-group".to_string())));
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(group.name(), "Split View: New Tab + New Tab");
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn split_view_to_group_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>>
{
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">split-view-to-group Research");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("split-view-to-group Research".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.tab_groups.is_empty());
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.command_query, ">split-view-to-group Research");
    Ok(())
}

#[test]
fn restore_last_archived_tab_restores_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let remaining_tab_id = core.active_tab()?.id().clone();
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    let split_id = core.split_active_tab_right()?;
    let focused_pane_id = core.active_tab()?.id().clone();
    core.save_active_split_view()?;
    core.close_active_tab()?;

    let restored_tab_id = core.restore_last_archived_tab()?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, focused_pane_id);
    assert_eq!(snapshot.active_tab_id, focused_pane_id);
    assert_eq!(snapshot.tabs.len(), 3);
    assert!(snapshot.archived_tabs.is_empty());
    let restored_layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing restored split layout")?;
    assert!(restored_layout.saved());
    assert_eq!(restored_layout.pane_count(), 2);
    assert!(snapshot.tabs.iter().any(|tab| tab.id() == &remaining_tab_id));
    Ok(())
}

#[test]
fn restore_archived_split_member_restores_entire_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    let split_id = core.split_active_tab_right()?;
    core.save_active_split_view()?;
    let left_pane_id = core.snapshot()?.split_layouts[0].panes()[0].tab_id().clone();
    core.close_active_tab()?;

    let restored_tab_id = core.restore_archived_tab(&left_pane_id)?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, left_pane_id);
    assert_eq!(snapshot.active_tab_id, left_pane_id);
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.split_layouts.len(), 1);
    assert_eq!(snapshot.split_layouts[0].id(), &split_id);
    assert_eq!(snapshot.split_layouts[0].pane_count(), 2);
    Ok(())
}

#[test]
fn archived_split_search_restores_entire_saved_split_view() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/saved-split")?);
    let split_id = core.split_active_tab_right()?;
    core.save_active_split_view()?;
    let left_pane_id = core.snapshot()?.split_layouts[0].panes()[0].tab_id().clone();
    core.close_active_tab()?;

    let restored_tab_id = core.restore_archived_tab_match("saved-split")?;
    let snapshot = core.snapshot()?;

    assert_eq!(restored_tab_id, Some(left_pane_id));
    assert!(snapshot.archived_tabs.is_empty());
    assert_eq!(snapshot.split_layouts.len(), 1);
    assert_eq!(snapshot.split_layouts[0].id(), &split_id);
    Ok(())
}

#[test]
fn closing_split_pane_dissolves_two_pane_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let remaining_tab_id = core.active_tab()?.id().clone();
    core.split_active_tab_right()?;

    core.close_active_tab()?;
    let snapshot = core.snapshot()?;

    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.active_tab_id, remaining_tab_id);
    assert_eq!(snapshot.tabs[0].split_id(), None);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().split_id(), None);
    Ok(())
}

#[test]
fn moving_split_pane_dissolves_source_layout() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let moved_tab_id = {
        let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
        core.select_space(&work_space_id)?;
        core.split_active_tab_right()?;
        let moved_tab_id = core.active_tab()?.id().clone();
        core.move_active_tab_to_space(&research_space_id)?;
        moved_tab_id
    };

    let research_snapshot = core.snapshot()?;
    assert_eq!(research_snapshot.active_tab_id, moved_tab_id);
    assert!(research_snapshot.split_layouts.is_empty());

    core.select_space(&work_space_id)?;
    let work_snapshot = core.snapshot()?;
    assert!(work_snapshot.split_layouts.is_empty());
    assert!(work_snapshot.tabs.iter().all(|tab| tab.split_id().is_none()));
    Ok(())
}
