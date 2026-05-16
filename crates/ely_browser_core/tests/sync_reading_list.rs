use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    ReadingProgress, ReadingProgressPercent, SyncObjectKind, SyncObjectPolicy, UrlText,
};

#[test]
fn sync_snapshot_imports_remote_reading_list_into_active_scope() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/long-read")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.set_tab_title(&source_tab_id, "Long Read")?;
    let entry_id = source.save_active_tab_to_reading_list()?;
    source.set_reading_list_progress(
        &entry_id,
        ReadingProgress::InProgress(ReadingProgressPercent::new(42)?),
    )?;
    pause_history_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let target_space_id = target.snapshot()?.active_space_id;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.reading_list.as_slice() else {
        return Err(
            format!("expected 1 reading list entry, got {}", snapshot.reading_list.len()).into()
        );
    };

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.profile_id(), &target_profile_id);
    assert_eq!(entry.space_id(), &target_space_id);
    assert_eq!(entry.title(), "Long Read");
    assert_eq!(entry.source_url().as_str(), "https://example.com/long-read");
    assert_eq!(entry.progress(), &ReadingProgress::InProgress(ReadingProgressPercent::new(42)?));
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_reading_list_entry() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/long-read")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.set_tab_title(&source_tab_id, "Canonical Long Read")?;
    let source_entry_id = source.save_active_tab_to_reading_list()?;
    source.set_reading_list_progress(&source_entry_id, ReadingProgress::Finished)?;
    pause_history_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_tab_id = target.open_tab(UrlText::parse("https://example.com/long-read")?);
    target.set_tab_title(&target_tab_id, "Old Long Read")?;
    let target_entry_id = target.save_active_tab_to_reading_list()?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.reading_list.as_slice() else {
        return Err(
            format!("expected 1 reading list entry, got {}", snapshot.reading_list.len()).into()
        );
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.id(), &target_entry_id);
    assert_eq!(entry.title(), "Canonical Long Read");
    assert_eq!(entry.progress(), &ReadingProgress::Finished);
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_reading_list() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/long-read")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.save_active_tab_to_reading_list()?;
    source.set_sync_object_policy(SyncObjectKind::ReadingList, SyncObjectPolicy::Paused);
    pause_history_sync(&mut source);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.reading_list.is_empty());
    Ok(())
}

fn pause_history_sync(core: &mut BrowserCore) {
    core.set_sync_object_policy(SyncObjectKind::History, SyncObjectPolicy::Paused);
}
