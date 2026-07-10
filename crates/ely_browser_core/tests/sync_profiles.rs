use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    ProfileKind, ProfileSyncPolicy, SiteOrigin, SitePermissionDecision, SitePermissionFeature,
    UrlText,
};

#[test]
fn sync_snapshot_imports_remote_profiles_before_spaces() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let profile_id = source.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;
    source.set_profile_sync_policy(&profile_id, ProfileSyncPolicy::Paused)?;
    source.create_space("Research Space", "R", 0x4477aa)?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let imported_profile = snapshot
        .profiles
        .iter()
        .find(|profile| profile.name() == "Research")
        .ok_or("missing imported profile")?;
    let imported_space = snapshot
        .spaces
        .iter()
        .find(|space| space.name() == "Research Space")
        .ok_or("missing imported space")?;

    assert_eq!(summary.imported(), 2);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(imported_profile.color_hex(), 0x9fc9a2);
    assert_eq!(imported_profile.kind(), &ProfileKind::Standard);
    assert_eq!(imported_profile.sync_policy(), ProfileSyncPolicy::Paused);
    assert_eq!(imported_space.default_profile_id(), imported_profile.id());
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_profile_metadata() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let source_profile_id = source.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;
    source.set_profile_sync_policy(&source_profile_id, ProfileSyncPolicy::Paused)?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.create_profile("Research", 0x111111, ProfileKind::Standard)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let updated_profile = snapshot
        .profiles
        .iter()
        .find(|profile| profile.id() == &target_profile_id)
        .ok_or("missing updated profile")?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(updated_profile.name(), "Research");
    assert_eq!(updated_profile.color_hex(), 0x9fc9a2);
    assert_eq!(updated_profile.sync_policy(), ProfileSyncPolicy::Paused);
    Ok(())
}

#[test]
fn paused_profile_data_is_omitted_from_sync_snapshots() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let profile_id = source.create_profile("Research", 0x9fc9a2, ProfileKind::Standard)?;
    source.set_profile_sync_policy(&profile_id, ProfileSyncPolicy::Paused)?;
    source.open_tab(UrlText::parse("https://example.com/paused-profile")?);
    source.bookmark_active_tab()?;
    source.save_active_url_note("profile paused")?;
    source.save_active_tab_to_reading_list()?;
    source.set_site_permission(
        SiteOrigin::parse("https://example.com")?,
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.profiles.iter().any(|profile| profile.name() == "Research"));
    let imported_profile_id = snapshot
        .profiles
        .iter()
        .find(|profile| profile.name() == "Research")
        .ok_or("missing imported profile")?
        .id()
        .clone();
    assert!(
        snapshot.tabs.iter().all(|tab| tab.url().as_str() != "https://example.com/paused-profile")
    );
    assert!(snapshot.bookmarks.is_empty());
    assert!(snapshot.notes.is_empty());
    assert!(snapshot.reading_list.is_empty());
    assert!(snapshot.site_permissions.is_empty());
    target.select_profile(&imported_profile_id)?;
    assert!(target.snapshot()?.history_entries.is_empty());
    Ok(())
}

#[test]
fn standard_profile_sync_preserves_a_same_named_private_profile() -> Result<(), Box<dyn Error>> {
    let mut source_config = InitialBrowserConfig::ely_defaults()?;
    source_config.profile_name = "Private".to_string();
    let mut source = BrowserCore::new(source_config)?;
    source.navigate_active_tab(UrlText::parse("https://example.com/remote")?)?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let private_profile_id = target.snapshot()?.active_profile_id;
    target.navigate_active_tab(UrlText::parse("https://private.example/secret")?)?;
    target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert!(snapshot.profiles.iter().any(|profile| {
        profile.id() == &private_profile_id && profile.kind() == &ProfileKind::Private
    }));
    assert!(snapshot.profiles.iter().any(|profile| {
        profile.id() != &private_profile_id
            && profile.name() == "Private"
            && profile.kind() == &ProfileKind::Standard
    }));
    let outbound = String::from_utf8(target.build_sync_snapshot_bytes()?)?;
    assert!(!outbound.contains("https://private.example/secret"));
    assert!(outbound.contains("https://example.com/remote"));
    Ok(())
}

#[test]
fn standard_profile_sync_remaps_a_private_profile_id_collision() -> Result<(), Box<dyn Error>> {
    let mut target = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let private_profile_id = target.snapshot()?.active_profile_id;
    target.navigate_active_tab(UrlText::parse("https://private.example/id-secret")?)?;

    let mut source_config = InitialBrowserConfig::ely_defaults()?;
    source_config.profile_id = Some(private_profile_id.clone());
    source_config.profile_name = " Synced ".to_string();
    let mut source = BrowserCore::new(source_config)?;
    source.navigate_active_tab(UrlText::parse("https://example.com/id-remote")?)?;
    let bytes = source.build_sync_snapshot_bytes()?;
    target.apply_sync_snapshot_bytes(&bytes)?;
    target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert!(snapshot.profiles.iter().any(|profile| {
        profile.id() == &private_profile_id && profile.kind() == &ProfileKind::Private
    }));
    assert_eq!(
        snapshot
            .profiles
            .iter()
            .filter(|profile| {
                profile.id() != &private_profile_id
                    && profile.name() == "Synced"
                    && profile.kind() == &ProfileKind::Standard
            })
            .count(),
        1
    );
    let outbound = String::from_utf8(target.build_sync_snapshot_bytes()?)?;
    assert!(!outbound.contains("https://private.example/id-secret"));
    assert!(outbound.contains("https://example.com/id-remote"));
    Ok(())
}
