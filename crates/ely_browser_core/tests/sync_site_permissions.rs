use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    SiteOrigin, SitePermissionDecision, SitePermissionFeature, SyncObjectKind, SyncObjectPolicy,
};

#[test]
fn sync_snapshot_imports_remote_site_permissions() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let origin = SiteOrigin::parse("https://example.com")?;
    source.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let target_profile_id = target.snapshot()?.active_profile_id;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.site_permissions.as_slice() else {
        return Err(
            format!("expected 1 site permission, got {}", snapshot.site_permissions.len()).into()
        );
    };

    assert_eq!(summary.imported(), 1);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.profile_id(), &target_profile_id);
    assert_eq!(entry.origin(), &origin);
    assert_eq!(entry.feature(), SitePermissionFeature::Camera);
    assert_eq!(entry.decision(), SitePermissionDecision::AllowAlways);
    assert!(snapshot.site_permission_audit_events.is_empty());
    Ok(())
}

#[test]
fn sync_snapshot_updates_existing_site_permission() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    let origin = SiteOrigin::parse("https://example.com")?;
    source.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Notifications,
        SitePermissionDecision::AllowAlways,
    )?;
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    target.set_site_permission(
        origin,
        SitePermissionFeature::Notifications,
        SitePermissionDecision::DenyAlways,
    )?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;
    let [entry] = snapshot.site_permissions.as_slice() else {
        return Err(
            format!("expected 1 site permission, got {}", snapshot.site_permissions.len()).into()
        );
    };

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 1);
    assert_eq!(summary.skipped(), 0);
    assert_eq!(entry.feature(), SitePermissionFeature::Notifications);
    assert_eq!(entry.decision(), SitePermissionDecision::AllowAlways);
    assert_eq!(snapshot.site_permission_audit_events.len(), 1);
    Ok(())
}

#[test]
fn sync_snapshot_omits_paused_site_permissions() -> Result<(), Box<dyn Error>> {
    let mut source = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let source_home_tab_id = source.snapshot()?.active_tab_id;
    source.set_tab_sync_enabled(&source_home_tab_id, false)?;
    source.set_site_permission(
        SiteOrigin::parse("https://example.com")?,
        SitePermissionFeature::Popups,
        SitePermissionDecision::DenyAlways,
    )?;
    source.set_sync_object_policy(SyncObjectKind::SitePermissions, SyncObjectPolicy::Paused);
    let bytes = source.build_sync_snapshot_bytes()?;

    let mut target = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let summary = target.apply_sync_snapshot_bytes(&bytes)?;
    let snapshot = target.snapshot()?;

    assert_eq!(summary.imported(), 0);
    assert_eq!(summary.updated(), 0);
    assert_eq!(summary.skipped(), 0);
    assert!(snapshot.site_permissions.is_empty());
    Ok(())
}
