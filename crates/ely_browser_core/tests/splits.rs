use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, SplitAxis};

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
