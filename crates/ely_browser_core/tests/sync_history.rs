use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{SyncObjectKind, SyncObjectPolicy, UrlText};

#[test]
fn sync_snapshot_imports_remote_history_into_active_scope() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.set_tab_title(&source_tab_id, "Research Brief")?;
    source.set_tab_favicon_key(&source_tab_id, "favicons/example.ico")?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let target_space_id = target.snapshot()?.active_space_id;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.history_entries.as_slice() else {
        return Err(
            format!("expected 1 history entry, got {}", snapshot.history_entries.len()).into()
        );
    };

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.profile_id(), &target_profile_id);
    assert_eq!(entry.space_id(), &target_space_id);
    assert_eq!(entry.source_tab_id(), &source_tab_id);
    assert_eq!(entry.title(), "Research Brief");
    assert_eq!(entry.url().as_str(), "https://example.com/research");
    assert_eq!(entry.favicon_key(), Some("favicons/example.ico"));
    assert_eq!(entry.visit_count(), 1);
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_history_entry() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let first_source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&first_source_tab_id, false)?;
    let latest_source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&latest_source_tab_id, false)?;
    source.set_tab_title(&latest_source_tab_id, "Canonical Research")?;
    source.set_tab_favicon_key(&latest_source_tab_id, "favicons/canonical.ico")?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_tab_id = target.open_tab(UrlText::parse("https://example.com/research")?);
    target.set_tab_title(&target_tab_id, "Old Research")?;
    target.set_tab_favicon_key(&target_tab_id, "favicons/old.ico")?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.history_entries.as_slice() else {
        return Err(
            format!("expected 1 history entry, got {}", snapshot.history_entries.len()).into()
        );
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.source_tab_id(), &target_tab_id);
    assert_eq!(entry.title(), "Canonical Research");
    assert_eq!(entry.favicon_key(), Some("favicons/canonical.ico"));
    assert_eq!(entry.visit_count(), 2);
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_history() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.set_sync_object_policy(SyncObjectKind::History, SyncObjectPolicy::Paused);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.history_entries.is_empty());
    Ok(())
}
