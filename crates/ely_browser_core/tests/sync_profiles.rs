use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    ProfileKind, ProfileSyncPolicy, SiteOrigin, SitePermissionDecision, SitePermissionFeature,
    SyncConnectionState, UrlText,
};
use ely_sync_client::SyncClientError;

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
    target.create_profile("Local", 0x26251e, ProfileKind::Standard)?;
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
    let outbound_bytes = target.build_sync_snapshot_bytes()?;
    let outbound = String::from_utf8(outbound_bytes.clone())?;
    assert!(!outbound.contains("https://private.example/secret"));
    assert!(!outbound.contains(private_profile_id.as_str()));
    assert!(!snapshot_contains_space_named(&outbound_bytes, "Private")?);
    assert!(outbound.contains("https://example.com/remote"));
    Ok(())
}

#[test]
fn standard_profile_sync_remaps_a_private_profile_id_collision() -> Result<(), Box<dyn Error>> {
    let mut target = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let private_snapshot = target.snapshot()?;
    let private_profile_id = private_snapshot.active_profile_id;
    let private_space_id = private_snapshot.active_space_id;
    target.navigate_active_tab(UrlText::parse("https://private.example/id-secret")?)?;
    target.create_profile("Local", 0x26251e, ProfileKind::Standard)?;
    target.navigate_active_tab(UrlText::parse(
        "https://private-space.example/standard-profile-secret",
    )?)?;
    target.bookmark_active_tab()?;
    target.save_active_url_note("private space note")?;
    target.save_active_tab_to_reading_list()?;

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
    let outbound_bytes = target.build_sync_snapshot_bytes()?;
    let outbound = String::from_utf8(outbound_bytes.clone())?;
    assert!(!outbound.contains("https://private.example/id-secret"));
    assert!(!outbound.contains("https://private-space.example/standard-profile-secret"));
    assert!(!outbound.contains(private_profile_id.as_str()));
    assert!(!outbound.contains(private_space_id.as_str()));
    assert!(!snapshot_contains_space_named(&outbound_bytes, "Private")?);
    assert!(!outbound.contains("\"space_name\":\"Private\""));
    assert!(outbound.contains("https://example.com/id-remote"));
    Ok(())
}

#[test]
fn private_profile_blocks_snapshot_input_and_output() -> Result<(), Box<dyn Error>> {
    let source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let bytes = source.build_sync_snapshot_bytes()?;
    let mut private = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    private.set_sync_connection_state(SyncConnectionState::SignedIn);

    assert!(!private.active_profile_allows_sync());
    assert!(!private.cloud_sync_upload_enabled());
    assert!(matches!(private.build_sync_snapshot_bytes(), Err(SyncClientError::SyncPolicy { .. })));
    assert!(matches!(
        private.apply_sync_snapshot_bytes(&bytes),
        Err(SyncClientError::SyncPolicy { .. })
    ));

    Ok(())
}

#[test]
fn sync_snapshot_rejects_records_targeting_a_private_profile() -> Result<(), Box<dyn Error>> {
    let mut target = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let private_profile_id = target.snapshot()?.active_profile_id;
    target.create_profile("Local", 0x26251e, ProfileKind::Standard)?;

    let mut source_config = InitialBrowserConfig::ely_defaults()?;
    source_config.profile_id = Some(private_profile_id);
    let source = BrowserCore::new(source_config)?;
    let mut document: serde_json::Value =
        serde_json::from_slice(&source.build_sync_snapshot_bytes()?)?;
    document["profiles"] = serde_json::json!([]);
    let bytes = serde_json::to_vec(&document)?;

    assert!(matches!(
        target.apply_sync_snapshot_bytes(&bytes),
        Err(SyncClientError::SyncPolicy { .. })
    ));

    Ok(())
}

#[test]
fn sync_snapshot_blocks_references_to_a_remote_private_profile() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    source.navigate_active_tab(UrlText::parse("https://private.example/remote-record")?)?;
    let mut document: serde_json::Value =
        serde_json::from_slice(&source.build_sync_snapshot_bytes()?)?;
    let profiles = document["profiles"].as_array_mut().ok_or("sync profiles must be an array")?;
    let profile = profiles.first_mut().ok_or("sync snapshot must include a profile")?;
    profile["kind"] = serde_json::json!("private");
    let bytes = serde_json::to_vec(&document)?;
    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    assert!(matches!(
        target.apply_sync_snapshot_bytes(&bytes),
        Err(SyncClientError::SyncPolicy { .. })
    ));
    assert!(
        target
            .snapshot()?
            .tabs
            .iter()
            .all(|tab| tab.url().as_str() != "https://private.example/remote-record")
    );

    Ok(())
}

#[test]
fn sync_snapshot_rejects_records_targeting_a_private_space() -> Result<(), Box<dyn Error>> {
    let mut target = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let private_space_id = target.snapshot()?.active_space_id;
    let standard_profile_id = target.create_profile("Local", 0x26251e, ProfileKind::Standard)?;

    let mut source_config = InitialBrowserConfig::ely_defaults()?;
    source_config.profile_id = Some(standard_profile_id);
    let source = BrowserCore::new(source_config)?;
    let mut document: serde_json::Value =
        serde_json::from_slice(&source.build_sync_snapshot_bytes()?)?;
    document["profiles"] = serde_json::json!([]);
    document["spaces"] = serde_json::json!([]);
    for tab in document["tabs"].as_array_mut().ok_or("sync tabs must be an array")? {
        tab["space_id"] = serde_json::json!(private_space_id.as_str());
        tab["space_name"] = serde_json::json!("Private");
    }
    let bytes = serde_json::to_vec(&document)?;

    assert!(matches!(
        target.apply_sync_snapshot_bytes(&bytes),
        Err(SyncClientError::SyncPolicy { .. })
    ));

    Ok(())
}

fn snapshot_contains_space_named(bytes: &[u8], name: &str) -> Result<bool, Box<dyn Error>> {
    let document: serde_json::Value = serde_json::from_slice(bytes)?;
    let spaces = document["spaces"].as_array().ok_or("sync snapshot spaces must be an array")?;
    Ok(spaces.iter().any(|space| space["name"].as_str() == Some(name)))
}
