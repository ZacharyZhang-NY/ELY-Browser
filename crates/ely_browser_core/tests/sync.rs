use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    SyncConnectionState, SyncObjectKind, SyncObjectPolicy, SyncObjectState, SyncObjectStatus,
    UrlText,
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
