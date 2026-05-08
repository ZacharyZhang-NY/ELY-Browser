use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    CommandIntent, ProfileKind, SiteOrigin, SitePermissionAuditAction, SitePermissionDecision,
    SitePermissionFeature, UrlText,
};

#[test]
fn set_site_permission_records_active_profile_origin_and_audit() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let origin = SiteOrigin::parse("https://example.com")?;

    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;

    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.site_permissions.len(), 1);
    let entry = &snapshot.site_permissions[0];
    assert_eq!(entry.profile_id(), &profile_id);
    assert_eq!(entry.origin(), &origin);
    assert_eq!(entry.feature(), SitePermissionFeature::Camera);
    assert_eq!(entry.decision(), SitePermissionDecision::AllowAlways);

    assert_eq!(snapshot.site_permission_audit_events.len(), 1);
    let audit_event = &snapshot.site_permission_audit_events[0];
    assert_eq!(audit_event.profile_id(), &profile_id);
    assert_eq!(audit_event.origin(), &origin);
    assert_eq!(audit_event.feature(), SitePermissionFeature::Camera);
    assert_eq!(
        audit_event.action(),
        &SitePermissionAuditAction::Set(SitePermissionDecision::AllowAlways),
    );
    Ok(())
}

#[test]
fn site_permissions_stay_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;
    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    let origin = SiteOrigin::parse("https://example.com")?;

    core.select_profile(&personal_profile_id)?;
    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Notifications,
        SitePermissionDecision::DenyAlways,
    )?;

    let personal_snapshot = core.snapshot()?;
    assert_eq!(personal_snapshot.site_permissions.len(), 1);
    assert_eq!(personal_snapshot.site_permissions[0].profile_id(), &personal_profile_id);

    core.select_profile(&default_profile_id)?;
    let default_snapshot = core.snapshot()?;
    assert!(default_snapshot.site_permissions.is_empty());
    assert!(default_snapshot.site_permission_audit_events.is_empty());

    core.select_profile(&personal_profile_id)?;
    let personal_snapshot = core.snapshot()?;
    assert_eq!(personal_snapshot.site_permissions.len(), 1);
    assert_eq!(personal_snapshot.site_permission_audit_events.len(), 1);
    Ok(())
}

#[test]
fn revoke_site_permission_removes_entry_and_records_audit() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let origin = SiteOrigin::parse("https://example.com")?;

    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Popups,
        SitePermissionDecision::DenyAlways,
    )?;
    core.revoke_site_permission(&origin, SitePermissionFeature::Popups)?;

    let snapshot = core.snapshot()?;
    assert!(snapshot.site_permissions.is_empty());
    assert_eq!(snapshot.site_permission_audit_events.len(), 2);
    assert_eq!(
        snapshot.site_permission_audit_events[1].action(),
        &SitePermissionAuditAction::Revoked,
    );
    Ok(())
}

#[test]
fn site_settings_command_opens_active_origin() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    core.open_tab(UrlText::parse("https://example.com/path")?);

    core.set_command_query(">site-settings");
    let intent = core.submit_command()?;

    assert_eq!(intent, Some(CommandIntent::Command("site-settings".to_string())),);
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;
    assert_eq!(snapshot.command_query, "");
    assert_eq!(active_tab.url().as_str(), "ely://site/https://example.com");
    assert_eq!(active_tab.title(), "Site Settings");
    Ok(())
}

#[test]
fn site_settings_command_preserves_query_for_internal_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">site-settings");
    let intent = core.submit_command()?;

    assert_eq!(intent, Some(CommandIntent::Command("site-settings".to_string())),);
    let snapshot = core.snapshot()?;
    let active_tab = core.active_tab()?;
    assert_eq!(snapshot.command_query, ">site-settings");
    assert_eq!(active_tab.url().as_str(), "ely://new-tab");
    Ok(())
}

#[test]
fn site_origin_from_route_and_url_require_web_origins() -> Result<(), Box<dyn Error>> {
    let origin = SiteOrigin::from_site_route("ely://site/https://example.com/path")?;
    let Some(origin) = origin else {
        return Err("missing site origin".into());
    };

    assert_eq!(origin.as_str(), "https://example.com");
    assert_eq!(SiteOrigin::from_url(&UrlText::parse("ely://settings")?)?, None,);
    assert_eq!(
        SiteOrigin::from_url(&UrlText::parse("https://example.com/path")?)?,
        Some(SiteOrigin::parse("https://example.com")?),
    );

    Ok(())
}
