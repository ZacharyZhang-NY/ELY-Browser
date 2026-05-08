use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ArchiveSource, CommandIntent, TabState, UrlText};

#[test]
fn archive_tab_group_command_archives_active_group_tabs() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let ungrouped_tab_id = core.open_tab(UrlText::parse("https://servo.org")?);
    let first_group_tab_id = core.open_tab(UrlText::parse("https://example.com/a")?);
    core.group_active_tab("Docs")?;
    let second_group_tab_id = core.open_tab(UrlText::parse("https://example.com/b")?);
    core.group_active_tab("Docs")?;

    core.set_command_query(">archive-tab-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let archived_tab_ids =
        snapshot.archived_tabs.iter().map(|archived| archived.tab().id()).collect::<Vec<_>>();

    assert_eq!(intent, Some(CommandIntent::Command("archive-tab-group".to_string())));
    assert_eq!(snapshot.command_query, "");
    assert!(snapshot.tab_groups.is_empty());
    assert_eq!(tab_state(&snapshot, &ungrouped_tab_id), Some(&TabState::Ready));
    assert!(archived_tab_ids.contains(&&first_group_tab_id));
    assert!(archived_tab_ids.contains(&&second_group_tab_id));
    assert!(
        snapshot
            .archived_tabs
            .iter()
            .all(|archived| archived.source() == &ArchiveSource::ManualClose)
    );
    assert!(snapshot.archived_tabs.iter().all(|archived| archived.tab().group_id().is_none()));
    Ok(())
}

#[test]
fn archive_tab_group_command_preserves_query_without_active_group() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.open_tab(UrlText::parse("https://example.com")?);

    core.set_command_query(">archive-tab-group");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("archive-tab-group".to_string())));
    assert_eq!(snapshot.command_query, ">archive-tab-group");
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert!(snapshot.archived_tabs.is_empty());
    Ok(())
}

fn tab_state<'a>(
    snapshot: &'a ely_browser_core::BrowserSnapshot,
    tab_id: &ely_domain::TabId,
) -> Option<&'a TabState> {
    snapshot.tabs.iter().find(|tab| tab.id() == tab_id).map(|tab| tab.state())
}
