use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText};

use crate::services::ProfileDataMode;

use super::{
    ServoLivePermissionGrant, apply_permission_consumption, external_web_surface_scopes,
    external_web_surface_tab_ids, visible_web_surface_tabs,
};

#[test]
fn consumption_receipt_finishes_a_pending_allow_once_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let origin = SiteOrigin::parse("https://example.com")?;
    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowOnce,
    )?;
    let revision =
        core.site_permission_revision(&profile_id, &origin, SitePermissionFeature::Camera);
    let consumed =
        ServoLivePermissionGrant::new(profile_id, origin, SitePermissionFeature::Camera, revision);

    assert!(apply_permission_consumption(&mut core, &consumed));
    assert!(core.snapshot()?.site_permissions.is_empty());
    Ok(())
}

#[test]
fn stale_consumption_keeps_a_newer_allow_once_grant() -> Result<(), Box<dyn std::error::Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let origin = SiteOrigin::parse("https://example.com")?;
    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowOnce,
    )?;
    let stale_revision =
        core.site_permission_revision(&profile_id, &origin, SitePermissionFeature::Camera);
    let stale = ServoLivePermissionGrant::new(
        profile_id.clone(),
        origin.clone(),
        SitePermissionFeature::Camera,
        stale_revision,
    );
    core.set_site_permission(
        origin.clone(),
        SitePermissionFeature::Camera,
        SitePermissionDecision::DenyAlways,
    )?;
    core.set_site_permission(
        origin,
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowOnce,
    )?;

    assert!(!apply_permission_consumption(&mut core, &stale));
    assert_eq!(
        core.site_permissions_for_profile(&profile_id)[0].decision(),
        SitePermissionDecision::AllowOnce,
    );
    Ok(())
}

#[test]
fn retired_permissions_are_settled_before_the_next_external_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    core.navigate_active_tab(UrlText::parse("https://example.com/a")?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let retiring_tab_id = core.snapshot()?.active_tab_id;
    let origin = SiteOrigin::parse("https://example.com")?;
    let mut retired = Vec::new();
    for feature in [SitePermissionFeature::Camera, SitePermissionFeature::Microphone] {
        core.set_site_permission(origin.clone(), feature, SitePermissionDecision::AllowOnce)?;
        let revision = core.site_permission_revision(&profile_id, &origin, feature);
        retired.push(ServoLivePermissionGrant::new(
            profile_id.clone(),
            origin.clone(),
            feature,
            revision,
        ));
    }
    assert!(core.transfer_site_permission_once(
        &profile_id,
        &origin,
        SitePermissionFeature::Microphone,
        retired[1].grant_revision(),
    )?);
    let visible_tab_id = core.open_tab(UrlText::parse("https://example.com/b")?);
    core.navigate_tab_to_loaded_url(&retiring_tab_id, UrlText::parse("ely://settings/general")?)?;
    let raw_visible_tabs = core.visible_content_tabs()?;

    for consumed in &retired {
        assert!(apply_permission_consumption(&mut core, consumed));
    }
    let visible_tabs = visible_web_surface_tabs(&core, raw_visible_tabs);

    assert_eq!(visible_tabs.len(), 1);
    assert_eq!(visible_tabs[0].tab.id(), &visible_tab_id);
    assert!(visible_tabs[0].permissions.is_empty());
    for consumed in &retired {
        assert!(!apply_permission_consumption(&mut core, consumed));
    }
    Ok(())
}

#[test]
fn external_tab_ids_exclude_internal_routes() -> Result<(), Box<dyn std::error::Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let tab_id = core.snapshot()?.active_tab_id;
    core.navigate_active_tab(UrlText::parse("https://example.com")?)?;
    assert_eq!(external_web_surface_tab_ids(core.open_tabs()), vec![tab_id.clone()]);

    core.navigate_active_tab(UrlText::parse("ely://settings/general")?)?;
    let snapshot = core.snapshot()?;
    assert_eq!(snapshot.active_tab_id, tab_id);
    assert!(external_web_surface_tab_ids(core.open_tabs()).is_empty());
    Ok(())
}

#[test]
fn external_tab_ids_include_inactive_spaces() -> Result<(), Box<dyn std::error::Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let external_tab_id = core.snapshot()?.active_tab_id;
    core.navigate_active_tab(UrlText::parse("https://example.com")?)?;
    core.create_space("Second", "circle", 0x807d72)?;

    assert!(external_web_surface_tab_ids(&core.snapshot()?.tabs).is_empty());
    assert_eq!(external_web_surface_tab_ids(core.open_tabs()), vec![external_tab_id.clone()]);
    assert_eq!(
        external_web_surface_scopes(&core),
        vec![(external_tab_id, profile_id, ProfileDataMode::Persistent,)]
    );
    Ok(())
}
