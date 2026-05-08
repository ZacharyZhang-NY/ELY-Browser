use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, SplitId, TabId, UrlText};

#[test]
fn swap_split_pane_command_swaps_last_pane_with_previous() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let (split_id, first_tab_id, second_tab_id, third_tab_id) = create_three_pane_split(&mut core)?;

    core.set_command_query(">swap-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("swap-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, third_tab_id);
    assert_eq!(pane_ids, vec![&first_tab_id, &third_tab_id, &second_tab_id]);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn swap_split_pane_right_command_moves_active_pane_forward() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let (split_id, first_tab_id, second_tab_id, third_tab_id) = create_three_pane_split(&mut core)?;
    core.select_tab(&first_tab_id)?;

    core.set_command_query(">swap-split-pane-right");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("swap-split-pane-right".to_string())));
    assert_eq!(snapshot.active_tab_id, first_tab_id);
    assert_eq!(pane_ids, vec![&second_tab_id, &first_tab_id, &third_tab_id]);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn swap_split_pane_left_command_moves_active_pane_backward() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let (split_id, first_tab_id, second_tab_id, third_tab_id) = create_three_pane_split(&mut core)?;
    core.select_tab(&second_tab_id)?;

    core.set_command_query(">swap-split-pane-left");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing split layout")?;
    let pane_ids = layout.panes().iter().map(|pane| pane.tab_id()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("swap-split-pane-left".to_string())));
    assert_eq!(snapshot.active_tab_id, second_tab_id);
    assert_eq!(pane_ids, vec![&second_tab_id, &first_tab_id, &third_tab_id]);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

#[test]
fn swap_split_pane_command_preserves_query_without_active_split() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();

    core.set_command_query(">swap-split-pane");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("swap-split-pane".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.split_layouts.is_empty());
    assert_eq!(snapshot.command_query, ">swap-split-pane");
    Ok(())
}

#[test]
fn swap_split_pane_refreshes_saved_split_title() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/docs")?);
    core.group_active_tab("Research")?;
    core.open_tab(UrlText::parse("https://servo.org")?);
    core.group_active_tab("Research")?;
    let split_id = core.split_active_tab_group()?.ok_or("missing split id")?;
    core.save_active_split_view()?;

    core.swap_active_split_pane()?;
    let snapshot = core.snapshot()?;
    let layout = snapshot
        .split_layouts
        .iter()
        .find(|layout| layout.id() == &split_id)
        .ok_or("missing saved split layout")?;

    assert!(layout.saved());
    assert_eq!(layout.title(), "Split View: servo.org + example.com");
    assert_eq!(layout.pane_count(), 2);
    Ok(())
}

fn create_three_pane_split(
    core: &mut BrowserCore,
) -> Result<(SplitId, TabId, TabId, TabId), Box<dyn Error>> {
    let first_tab_id = core.active_tab()?.id().clone();
    let split_id = core.split_active_tab_right()?;
    let second_tab_id = core.active_tab()?.id().clone();
    core.split_active_tab_right()?;
    let third_tab_id = core.active_tab()?.id().clone();

    Ok((split_id, first_tab_id, second_tab_id, third_tab_id))
}
