use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{SyncConnectionState, SyncObjectKind, SyncObjectState, SyncObjectStatus, UrlText};

#[test]
fn default_sync_status_reflects_local_browser_state() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.create_space("Research", "R", 0xf54e00)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.bookmark_active_tab()?;
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
            SyncObjectStatus::new(SyncObjectKind::ReadingList, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::Profiles, 1, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::SitePermissions, 0, SyncObjectState::LocalOnly),
            SyncObjectStatus::new(SyncObjectKind::History, 1, SyncObjectState::PrivacyControlled),
            SyncObjectStatus::new(SyncObjectKind::PluginSettings, 0, SyncObjectState::LocalOnly),
        ],
    );
    Ok(())
}
