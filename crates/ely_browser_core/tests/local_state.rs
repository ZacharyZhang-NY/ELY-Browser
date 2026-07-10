use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    FavoriteLimit, HistoryRecordingPolicy, NewTabDestination, SearchEngine, SyncObjectKind,
    SyncObjectPolicy, ThemeMode, UrlText,
};

fn standard_core() -> Result<BrowserCore, Box<dyn Error>> {
    Ok(BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?)
}

#[test]
fn local_state_persists_scalar_settings() -> Result<(), Box<dyn Error>> {
    let mut before = standard_core()?;
    before.set_search_engine(SearchEngine::Google);
    before.set_new_tab_destination(NewTabDestination::Bookmarks);
    before.set_favorite_limit(FavoriteLimit::TwentyFour);
    before.set_history_recording_policy(HistoryRecordingPolicy::Pause);
    before.set_theme_mode(ThemeMode::Dark);
    // Privacy-critical: a paused sync toggle must not silently re-enable.
    before.set_sync_object_policy(SyncObjectKind::History, SyncObjectPolicy::Paused);
    let bytes = before.build_local_state_bytes()?;

    let mut after = standard_core()?;
    after.apply_local_state_bytes(&bytes)?;

    assert_eq!(after.search_engine(), SearchEngine::Google);
    assert_eq!(after.new_tab_destination(), NewTabDestination::Bookmarks);
    assert_eq!(after.favorite_limit(), FavoriteLimit::TwentyFour);
    assert_eq!(after.history_recording_policy(), HistoryRecordingPolicy::Pause);
    assert_eq!(after.appearance().theme_mode(), ThemeMode::Dark);
    assert_eq!(
        after.sync_object_policy(SyncObjectKind::History),
        SyncObjectPolicy::Paused,
        "a paused sync toggle must survive a restart",
    );
    Ok(())
}

#[test]
fn local_state_round_trips_across_a_restart() -> Result<(), Box<dyn Error>> {
    let mut before = standard_core()?;
    before.open_tab(UrlText::parse("https://servo.org/")?);
    before.bookmark_active_tab()?;
    before.set_command_query(">new-space Research");
    before.submit_command()?;
    let bytes = before.build_local_state_bytes()?;

    let mut after = standard_core()?;
    after.apply_local_state_bytes(&bytes)?;
    let snapshot = after.snapshot()?;

    assert!(snapshot.tabs.iter().any(|tab| tab.url().as_str() == "https://servo.org/"));
    assert!(snapshot.bookmarks.iter().any(|entry| entry.url().as_str() == "https://servo.org/"));
    assert!(snapshot.spaces.iter().any(|space| space.name() == "Research"));
    Ok(())
}

#[test]
fn paused_cloud_sync_does_not_reduce_local_state() -> Result<(), Box<dyn Error>> {
    let mut core = standard_core()?;
    core.open_tab(UrlText::parse("https://example.com/paused")?);
    core.set_sync_object_policy(SyncObjectKind::Tabs, SyncObjectPolicy::Paused);

    let bytes = core.build_local_state_bytes()?;
    let mut restored = standard_core()?;
    restored.apply_local_state_bytes(&bytes)?;

    assert!(
        restored
            .snapshot()?
            .tabs
            .iter()
            .any(|tab| tab.url().as_str() == "https://example.com/paused"),
        "tabs must persist locally even when cloud sync is paused"
    );
    Ok(())
}

#[test]
fn private_profile_data_never_persists() -> Result<(), Box<dyn Error>> {
    let mut core = standard_core()?;
    core.set_command_query(">new-private-profile Vault");
    core.submit_command()?;
    core.set_command_query(">switch-profile Vault");
    core.submit_command()?;
    core.open_tab(UrlText::parse("https://example.com/secret")?);

    let bytes = core.build_local_state_bytes()?;
    let document = String::from_utf8(bytes.clone())?;
    assert!(!document.contains("Vault"), "private profile must not persist");
    assert!(!document.contains("example.com/secret"), "private tab must not persist");

    let mut restored = standard_core()?;
    restored.apply_local_state_bytes(&bytes)?;
    Ok(())
}

#[test]
fn unknown_local_state_revisions_fail_closed() -> Result<(), Box<dyn Error>> {
    let mut core = standard_core()?;
    let result =
        core.apply_local_state_bytes(br#"{"local_rev":99,"body":{"schema_rev":1,"bookmarks":[]}}"#);
    assert!(result.is_err(), "unknown local_rev must be rejected");
    Ok(())
}
