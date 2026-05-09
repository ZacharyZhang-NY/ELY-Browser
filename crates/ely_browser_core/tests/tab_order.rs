use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, UrlText};

#[test]
fn move_active_tab_commands_reorder_visible_tabs() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let first_tab_id = core.active_tab()?.id().clone();
    let second_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let third_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.set_command_query(">move-tab-up");
    let up_intent = core.submit_command()?;
    let up_snapshot = core.snapshot()?;
    let up_order = up_snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(up_intent, Some(CommandIntent::Command("move-tab-up".to_string())));
    assert_eq!(up_order, vec![first_tab_id.clone(), third_tab_id.clone(), second_tab_id.clone()]);
    assert_eq!(up_snapshot.active_tab_id, third_tab_id);
    assert_eq!(up_snapshot.command_query, "");

    core.set_command_query(">move-tab-down");
    let down_intent = core.submit_command()?;
    let down_snapshot = core.snapshot()?;
    let down_order = down_snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(down_intent, Some(CommandIntent::Command("move-tab-down".to_string())));
    assert_eq!(down_order, vec![first_tab_id, second_tab_id, third_tab_id]);
    assert_eq!(down_snapshot.command_query, "");
    Ok(())
}

#[test]
fn move_active_tab_command_preserves_query_at_boundary() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();
    core.open_tab(UrlText::parse("https://example.com")?);
    core.select_tab(&active_tab_id)?;

    core.set_command_query(">move-tab-up");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let ordered_ids = snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("move-tab-up".to_string())));
    assert_eq!(snapshot.command_query, ">move-tab-up");
    assert_eq!(ordered_ids[0], active_tab_id);
    Ok(())
}

#[test]
fn moving_active_tab_stays_within_active_space() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let work_space_id = core.snapshot()?.active_space_id;
    let work_tab_id = core.active_tab()?.id().clone();
    let work_example_tab_id = core.open_tab(UrlText::parse("https://example.com")?);
    let research_space_id = core.create_space("Research", "R", 0xf54e00)?;
    let research_tab_id = core.active_tab()?.id().clone();
    let research_servo_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);

    core.move_active_tab_up()?;
    let research_snapshot = core.snapshot()?;
    let research_order =
        research_snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(research_snapshot.active_space_id, research_space_id);
    assert_eq!(research_order, vec![research_servo_tab_id, research_tab_id]);

    core.select_space(&work_space_id)?;
    let work_snapshot = core.snapshot()?;
    let work_order = work_snapshot.tabs.iter().map(|tab| tab.id().clone()).collect::<Vec<_>>();

    assert_eq!(work_order, vec![work_tab_id, work_example_tab_id]);
    Ok(())
}
