use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, CommandScope, HistoryRecordingPolicy, ProfileKind, UrlText};

#[test]
fn navigation_records_profile_and_space_history() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_profile_id = core.active_tab()?.profile_id().clone();
    let active_space_id = core.snapshot()?.active_space_id;

    core.open_tab(UrlText::parse("https://example.com/research")?);
    let snapshot = core.snapshot()?;

    assert_eq!(snapshot.history_entries.len(), 1);
    assert_eq!(snapshot.history_entries[0].profile_id(), &active_profile_id);
    assert_eq!(snapshot.history_entries[0].space_id(), &active_space_id);
    assert_eq!(snapshot.history_entries[0].title(), "example.com");
    assert_eq!(snapshot.history_entries[0].url().as_str(), "https://example.com/research");
    Ok(())
}

#[test]
fn internal_pages_are_omitted_from_history() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.open_tab(UrlText::parse("ely://history")?);

    assert!(core.snapshot()?.history_entries.is_empty());
    Ok(())
}

#[test]
fn paused_history_recording_skips_new_history_entries() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.open_tab(UrlText::parse("https://example.com/recorded")?);
    core.set_history_recording_policy(HistoryRecordingPolicy::Pause);
    core.open_tab(UrlText::parse("https://example.com/private")?);

    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.history_recording_policy, HistoryRecordingPolicy::Pause);
    assert_eq!(snapshot.history_entries.len(), 1);
    assert_eq!(snapshot.history_entries[0].url().as_str(), "https://example.com/recorded");
    Ok(())
}

#[test]
fn history_scoped_search_opens_recent_matching_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);

    core.set_command_query("@history example");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::History,
            query: "example".to_string()
        })
    );
    assert_eq!(active_tab.url().as_str(), "https://example.com/research");
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.history_entries.len(), 2);
    Ok(())
}

#[test]
fn history_scoped_search_preserves_query_without_match() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let active_tab_id = core.open_tab(UrlText::parse("https://example.com/research")?);

    core.set_command_query("@history absent");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::History,
            query: "absent".to_string()
        })
    );
    assert_eq!(snapshot.active_tab_id, active_tab_id);
    assert_eq!(snapshot.command_query, "@history absent");
    Ok(())
}

#[test]
fn history_scoped_search_stays_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;

    core.open_tab(UrlText::parse("https://example.com/personal")?);
    core.select_profile(&default_profile_id)?;

    core.set_command_query("@history personal");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::ScopedSearch {
            scope: CommandScope::History,
            query: "personal".to_string()
        })
    );
    assert_eq!(core.active_tab()?.profile_id(), &default_profile_id);
    assert_ne!(core.active_tab()?.profile_id(), &personal_profile_id);
    assert_eq!(snapshot.command_query, "@history personal");
    Ok(())
}
