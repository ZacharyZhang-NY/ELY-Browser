use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{
    CommandIntent, ProfileKind, SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText,
};

#[test]
fn clear_site_data_command_scopes_to_active_origin_and_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;
    let work_space_id = core.snapshot()?.active_space_id;
    let example_origin = SiteOrigin::parse("https://example.com")?;
    let other_origin = SiteOrigin::parse("https://example.org")?;

    let example_tab_id = core.open_tab(UrlText::parse("https://example.com/work")?);
    core.open_tab(UrlText::parse("https://example.org/work")?);
    core.create_space("Research", "R", 0xf54e00)?;
    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.set_site_permission(
        example_origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    core.set_site_permission(
        other_origin,
        SitePermissionFeature::Notifications,
        SitePermissionDecision::DenyAlways,
    )?;

    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    core.select_profile(&personal_profile_id)?;
    core.open_tab(UrlText::parse("https://example.com/personal")?);
    core.set_site_permission(
        example_origin,
        SitePermissionFeature::Microphone,
        SitePermissionDecision::DenyAlways,
    )?;

    core.select_profile(&default_profile_id)?;
    core.select_space(&work_space_id)?;
    core.select_tab(&example_tab_id)?;
    core.set_command_query(">clear-site-data-for-this-profile");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::Command("clear-site-data-for-this-profile".to_string()))
    );
    assert_eq!(snapshot.command_query, "");
    assert_eq!(snapshot.history_entries.len(), 1);
    assert_eq!(snapshot.active_profile_history_entry_count, 1);
    assert_eq!(snapshot.history_entries[0].url().as_str(), "https://example.org/work");
    assert_eq!(snapshot.site_permissions.len(), 1);
    assert_eq!(snapshot.site_permissions[0].origin().as_str(), "https://example.org");
    assert_eq!(snapshot.site_permission_audit_events.len(), 3);

    core.select_profile(&personal_profile_id)?;
    let personal_snapshot = core.snapshot()?;
    assert_eq!(personal_snapshot.active_profile_history_entry_count, 1);
    assert_eq!(personal_snapshot.site_permissions.len(), 1);
    assert_eq!(personal_snapshot.site_permissions[0].origin().as_str(), "https://example.com");
    Ok(())
}

#[test]
fn clear_site_data_command_preserves_query_for_internal_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">clear-site-data-for-this-profile");
    let intent = core.submit_command()?;
    let snapshot = core.snapshot()?;

    assert_eq!(
        intent,
        Some(CommandIntent::Command("clear-site-data-for-this-profile".to_string()))
    );
    assert_eq!(snapshot.command_query, ">clear-site-data-for-this-profile");
    assert!(snapshot.history_entries.is_empty());
    assert!(snapshot.site_permissions.is_empty());
    Ok(())
}

#[test]
fn clear_active_profile_site_data_reports_removed_counts() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let origin = SiteOrigin::parse("https://example.com")?;

    core.open_tab(UrlText::parse("https://example.com/account")?);
    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    core.set_site_permission(
        origin,
        SitePermissionFeature::Notifications,
        SitePermissionDecision::DenyAlways,
    )?;

    let clearance =
        core.clear_active_profile_site_data()?.ok_or("missing active site data clearance")?;

    assert_eq!(clearance.history_entries(), 1);
    assert_eq!(clearance.site_permissions(), 2);
    assert_eq!(clearance.total_items(), 3);
    Ok(())
}
