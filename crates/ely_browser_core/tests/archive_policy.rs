use std::{
    error::Error,
    time::{Duration, SystemTime},
};

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ArchivePolicy, ArchiveSource, CommandIntent, UrlText};

#[test]
fn archive_idle_tabs_uses_active_space_policy() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();
    let idle_tab_id = core.open_tab(UrlText::parse("https://example.com/idle")?);
    let pinned_tab_id = core.open_tab(UrlText::parse("https://example.com/pinned")?);
    core.toggle_active_tab_pinned()?;
    core.select_tab(&active_tab_id)?;
    core.set_active_space_archive_policy(ArchivePolicy::IdleDays(1))?;

    let archived_count = core.archive_idle_tabs(days_from_now(2))?;
    let snapshot = core.snapshot()?;

    assert_eq!(archived_count, 1);
    assert!(snapshot.tabs.iter().any(|tab| tab.id() == &active_tab_id));
    assert!(snapshot.tabs.iter().any(|tab| tab.id() == &pinned_tab_id));
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &idle_tab_id);
    assert_eq!(snapshot.archived_tabs[0].source(), &ArchiveSource::AutoArchive);
    Ok(())
}

#[test]
fn archive_idle_tabs_manual_policy_leaves_tabs_open() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();
    let idle_tab_id = core.open_tab(UrlText::parse("https://example.com/idle")?);
    core.select_tab(&active_tab_id)?;

    let archived_count = core.archive_idle_tabs(days_from_now(30))?;
    let snapshot = core.snapshot()?;

    assert_eq!(archived_count, 0);
    assert!(snapshot.tabs.iter().any(|tab| tab.id() == &idle_tab_id));
    assert!(snapshot.archived_tabs.is_empty());
    Ok(())
}

#[test]
fn archive_idle_tabs_command_sets_days_policy() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.active_tab()?.id().clone();
    let archived_tab_id = core.open_tab(UrlText::parse("https://example.com/command")?);
    core.select_tab(&active_tab_id)?;

    core.set_command_query(">archive-idle-tabs 0");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(intent, Some(CommandIntent::Command("archive-idle-tabs 0".to_string())));
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.archived_tabs.len(), 1);
    assert_eq!(snapshot.archived_tabs[0].tab().id(), &archived_tab_id);
    assert_eq!(snapshot.archived_tabs[0].source(), &ArchiveSource::AutoArchive);
    assert_eq!(snapshot.command_query, "");
    Ok(())
}

fn days_from_now(days: u64) -> SystemTime {
    SystemTime::now() + Duration::from_secs(days * 86_400)
}
