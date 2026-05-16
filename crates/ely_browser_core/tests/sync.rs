use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    ArchivePolicy, SyncConnectionState, SyncObjectKind, SyncObjectPolicy, SyncObjectState,
    SyncObjectStatus, UrlText,
};

#[test]
fn default_sync_status_reflects_local_browser_state() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.create_space("Research", "R", 0xf54e00)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.bookmark_active_tab()?;
    core.save_active_url_note("sync note")?;
    core.save_active_tab_to_reading_list()?;

    let snapshot = core.snapshot()?;
    let status = &snapshot.sync_status;

    assert_eq!(status.connection(), &SyncConnectionState::SignedOut);
    assert_eq!(status.pending_objects(), 0);
    assert_eq!(status.failed_objects(), 0);
    assert_eq!(
        status.objects(),
        &[
            SyncObjectStatus::new(SyncObjectKind::Spaces, 2, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::Tabs, 3, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::Bookmarks, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::Notes, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::ReadingList, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::Profiles, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::SitePermissions, 0, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::History, 1, SyncObjectState::PrivacyControlled),
            SyncObjectStatus::new(SyncObjectKind::PluginSettings, 0, SyncObjectState::LocalOnly),
        ],
    );
    Ok(())
}

#[test]
fn sync_object_policy_pauses_object_kind() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.set_sync_object_policy(SyncObjectKind::Tabs, SyncObjectPolicy::Paused);

    let snapshot = core.snapshot()?;
    let Some(tabs_status) =
        snapshot.sync_status.objects().iter().find(|status| status.kind() == SyncObjectKind::Tabs)
    else {
        return Err("missing tabs sync status".into());
    };
    let Some(spaces_status) = snapshot
        .sync_status
        .objects()
        .iter()
        .find(|status| status.kind() == SyncObjectKind::Spaces)
    else {
        return Err("missing spaces sync status".into());
    };

    assert_eq!(core.sync_object_policy(SyncObjectKind::Tabs), SyncObjectPolicy::Paused);
    assert_eq!(tabs_status.policy(), SyncObjectPolicy::Paused);
    assert_eq!(tabs_status.state(), SyncObjectState::Paused);
    assert_eq!(spaces_status.policy(), SyncObjectPolicy::Enabled);
    assert_eq!(spaces_status.state(), SyncObjectState::LocalOnly);
    Ok(())
}

#[test]
fn tab_sync_status_counts_sync_enabled_tabs() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let unsynced_tab_id = core.open_tab(UrlText::parse("https://example.com/local-only")?);

    core.set_tab_sync_enabled(&unsynced_tab_id, false)?;
    let snapshot = core.snapshot()?;
    let Some(tabs_status) =
        snapshot.sync_status.objects().iter().find(|status| status.kind() == SyncObjectKind::Tabs)
    else {
        return Err("missing tabs sync status".into());
    };
    let Some(unsynced_tab) = snapshot.tabs.iter().find(|tab| tab.id() == &unsynced_tab_id) else {
        return Err("missing unsynced tab".into());
    };

    assert!(!unsynced_tab.sync_enabled());
    assert_eq!(tabs_status.local_count(), 1);
    Ok(())
}

#[test]
fn sync_snapshot_imports_remote_bookmarks_into_active_scope() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    let bookmark_id = source.bookmark_active_tab()?;
    source.set_bookmark_collection_name(&bookmark_id, "Research")?;
    source.set_bookmark_tags(&bookmark_id, vec!["rust".to_string(), "gpui".to_string()])?;
    source.set_bookmark_note(&bookmark_id, "Read later")?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let target_space_id = target.snapshot()?.active_space_id;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(snapshot.bookmarks.len(), 1);
    assert_eq!(snapshot.bookmarks[0].profile_id(), &target_profile_id);
    assert_eq!(snapshot.bookmarks[0].space_id(), &target_space_id);
    assert_eq!(snapshot.bookmarks[0].collection_name(), "Research");
    assert_eq!(snapshot.bookmarks[0].tags(), &["rust".to_string(), "gpui".to_string()]);
    assert_eq!(snapshot.bookmarks[0].note(), Some("Read later"));
    Ok(())
}

#[test]
fn sync_snapshot_imports_remote_tabs_into_active_scope() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_title(&source_tab_id, "Research Brief")?;
    source.set_tab_favicon_key(&source_tab_id, "https://example.com/favicon.ico")?;
    source.toggle_active_tab_pinned()?;
    source.toggle_active_tab_favorite()?;
    source.set_active_tab_zoom_percent(125)?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let target_space_id = target.snapshot()?.active_space_id;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let imported = snapshot
        .tabs
        .iter()
        .find(|tab| tab.url().as_str() == "https://example.com/research")
        .ok_or("missing imported tab")?;

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(imported.profile_id(), &target_profile_id);
    assert_eq!(imported.space_id(), &target_space_id);
    assert_eq!(imported.title(), "Research Brief");
    assert_eq!(imported.favicon_key(), Some("https://example.com/favicon.ico"));
    assert!(imported.flags().pinned);
    assert!(imported.flags().favorite);
    assert_eq!(imported.zoom_percent(), 125);
    Ok(())
}

#[test]
fn sync_snapshot_imports_remote_spaces_before_tabs() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let research_space_id = source.create_space("Research", "R", 0xf54e00)?;
    source.set_active_space_archive_policy(ArchivePolicy::IdleDays(14))?;
    source.set_space_sidebar_width(&research_space_id, 320)?;
    let research_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&research_home_tab_id, false)?;
    source.open_tab(UrlText::parse("https://example.com/research")?);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let research_space = snapshot
        .spaces
        .iter()
        .find(|space| space.name() == "Research")
        .ok_or("missing imported research space")?;
    let research_space_id = research_space.id().clone();
    assert_eq!(research_space.archive_policy(), &ArchivePolicy::IdleDays(14));
    assert_eq!(research_space.sidebar_width_px(), 320);

    target.select_space(&research_space_id)?;
    let snapshot = target.snapshot()?;
    let imported_tab = snapshot
        .tabs
        .iter()
        .find(|tab| tab.url().as_str() == "https://example.com/research")
        .ok_or("missing imported research tab")?;

    assert_eq!(summary.imported(), 2);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(imported_tab.space_id(), &research_space_id);
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_tab_metadata() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_title(&source_tab_id, "Research Brief")?;
    source.set_tab_favicon_key(&source_tab_id, "https://example.com/favicon.ico")?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_tab_id = target.open_tab(UrlText::parse("https://example.com/research")?);
    target.set_tab_title(&target_tab_id, "Old Title")?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let updated =
        snapshot.tabs.iter().find(|tab| tab.id() == &target_tab_id).ok_or("missing updated tab")?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(updated.title(), "Research Brief");
    assert_eq!(updated.favicon_key(), Some("https://example.com/favicon.ico"));
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_tabs() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_sync_object_policy(SyncObjectKind::Tabs, SyncObjectPolicy::Paused);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.tabs.iter().all(|tab| tab.url().as_str() != "https://example.com/research"));
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_bookmark_metadata() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    let source_bookmark_id = source.bookmark_active_tab()?;
    source.set_bookmark_collection_name(&source_bookmark_id, "Research")?;
    source.set_bookmark_tags(&source_bookmark_id, vec!["servo".to_string()])?;
    source.set_bookmark_note(&source_bookmark_id, "Canonical")?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    target.open_tab(UrlText::parse("https://example.com/research")?);
    let target_bookmark_id = target.bookmark_active_tab()?;
    target.set_bookmark_collection_name(&target_bookmark_id, "Inbox")?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(snapshot.bookmarks.len(), 1);
    assert_eq!(snapshot.bookmarks[0].id(), &target_bookmark_id);
    assert_eq!(snapshot.bookmarks[0].collection_name(), "Research");
    assert_eq!(snapshot.bookmarks[0].tags(), &["servo".to_string()]);
    assert_eq!(snapshot.bookmarks[0].note(), Some("Canonical"));
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_bookmarks() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_tab_id = source.open_tab(UrlText::parse("https://example.com/research")?);
    source.set_tab_sync_enabled(&source_tab_id, false)?;
    source.bookmark_active_tab()?;
    source.set_sync_object_policy(SyncObjectKind::Bookmarks, SyncObjectPolicy::Paused);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.bookmarks.is_empty());
    Ok(())
}

#[test]
fn sync_snapshot_rejects_unknown_schema_rev() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let bytes = br#"{"schema_rev":999,"bookmarks":[]}"#;

    let Err(error) = core.apply_sync_snapshot_bytes(bytes) else {
        return Err("expected sync snapshot schema error".into());
    };

    assert!(error.to_string().contains("unsupported schema_rev 999"));
    Ok(())
}
