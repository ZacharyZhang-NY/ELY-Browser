use std::{error::Error, path::PathBuf};

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    ArchivePolicy, DEFAULT_SIDEBAR_WIDTH_PX, DownloadPolicy, FavoriteLimit, HistoryRecordingPolicy,
    NewTabDestination, ProfileKind, ProfileSyncPolicy, SearchEngine, SyncObjectKind,
    SyncObjectPolicy,
};

#[test]
fn section_resets_restore_settings_defaults() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_new_tab_destination(NewTabDestination::Bookmarks);
    core.reset_general_settings();
    assert_eq!(core.snapshot()?.new_tab_destination, NewTabDestination::ElyNewTab);

    core.set_search_engine(SearchEngine::Google);
    core.reset_search_settings();
    assert_eq!(core.snapshot()?.search_engine, SearchEngine::DuckDuckGo);

    core.set_history_recording_policy(HistoryRecordingPolicy::Pause);
    core.reset_privacy_settings();
    assert_eq!(core.snapshot()?.history_recording_policy, HistoryRecordingPolicy::Record);

    let active_space_id = core.snapshot()?.active_space_id;
    core.set_active_space_archive_policy(ArchivePolicy::IdleDays(30))?;
    core.set_space_sidebar_width(&active_space_id, 56)?;
    core.set_favorite_limit(FavoriteLimit::Six);
    core.reset_sidebar_tabs_settings()?;

    let snapshot = core.snapshot()?;
    let Some(active_space) = snapshot.spaces.iter().find(|space| space.id() == &active_space_id)
    else {
        return Err("missing active space".into());
    };
    assert_eq!(active_space.archive_policy(), &ArchivePolicy::Manual);
    assert_eq!(active_space.sidebar_width_px(), DEFAULT_SIDEBAR_WIDTH_PX);
    assert_eq!(snapshot.favorite_limit, FavoriteLimit::Twelve);

    core.set_active_profile_download_policy(DownloadPolicy::fixed_directory(PathBuf::from(
        "/tmp",
    ))?)?;
    core.reset_active_profile_download_settings()?;
    assert_eq!(core.snapshot()?.active_download_policy, DownloadPolicy::ask_every_time());

    core.set_sync_object_policy(SyncObjectKind::Tabs, SyncObjectPolicy::Paused);
    core.reset_sync_settings();
    assert_eq!(core.sync_object_policy(SyncObjectKind::Tabs), SyncObjectPolicy::Enabled);
    Ok(())
}

#[test]
fn profile_sync_reset_restores_profile_kind_defaults() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let standard_profile_id = core.snapshot()?.active_profile_id;
    let private_profile_id = core.create_profile("Private", 0x807d72, ProfileKind::Private)?;

    core.select_profile(&standard_profile_id)?;
    core.set_profile_sync_policy(&standard_profile_id, ProfileSyncPolicy::Paused)?;
    core.reset_profile_sync_settings();

    let snapshot = core.snapshot()?;
    let Some(standard_profile) =
        snapshot.profiles.iter().find(|profile| profile.id() == &standard_profile_id)
    else {
        return Err("missing standard profile".into());
    };
    let Some(private_profile) =
        snapshot.profiles.iter().find(|profile| profile.id() == &private_profile_id)
    else {
        return Err("missing private profile".into());
    };

    assert_eq!(standard_profile.sync_policy(), ProfileSyncPolicy::Enabled);
    assert_eq!(private_profile.sync_policy(), ProfileSyncPolicy::Paused);
    Ok(())
}
