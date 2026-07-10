use std::error::Error;

use ely_browser_core::{BrowserCore, ELYDATA_SCHEMA_VERSION, InitialBrowserConfig};
use ely_domain::{
    CommandIntent, ProfileKind, SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText,
};

#[test]
fn local_data_inventory_counts_active_profile_data() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;

    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.bookmark_active_tab()?;
    core.save_active_url_note("profile note")?;
    core.save_active_tab_to_reading_list()?;
    core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;
    core.set_site_permission(
        SiteOrigin::parse("https://example.com")?,
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    let archived_tab_id = core.open_tab(UrlText::parse("https://servo.org/")?);
    core.close_tab(&archived_tab_id)?;

    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    core.open_tab(UrlText::parse("https://personal.example/research")?);
    core.bookmark_active_tab()?;
    core.save_active_url_note("personal note")?;
    core.record_download_started(
        UrlText::parse("https://personal.example/report.pdf")?,
        "personal-report.pdf",
        Some(4096),
    )?;
    core.set_site_permission(
        SiteOrigin::parse("https://personal.example")?,
        SitePermissionFeature::Notifications,
        SitePermissionDecision::DenyAlways,
    )?;

    core.select_profile(&default_profile_id)?;
    let inventory = core.snapshot()?.local_data_inventory;

    assert_eq!(inventory.open_tabs(), 2);
    assert_eq!(inventory.archived_tabs(), 1);
    assert_eq!(inventory.bookmarks(), 1);
    assert_eq!(inventory.notes(), 1);
    assert_eq!(inventory.reading_list(), 1);
    assert_eq!(inventory.history_entries(), 2);
    assert_eq!(inventory.site_permissions(), 1);
    assert_eq!(inventory.site_permission_audit_events(), 1);
    assert_eq!(inventory.downloads(), 1);
    assert_eq!(inventory.total_items(), 11);

    core.select_profile(&personal_profile_id)?;
    let personal_inventory = core.snapshot()?.local_data_inventory;

    assert_eq!(personal_inventory.open_tabs(), 2);
    assert_eq!(personal_inventory.bookmarks(), 1);
    assert_eq!(personal_inventory.notes(), 1);
    assert_eq!(personal_inventory.history_entries(), 1);
    assert_eq!(personal_inventory.site_permissions(), 1);
    assert_eq!(personal_inventory.site_permission_audit_events(), 1);
    assert_eq!(personal_inventory.downloads(), 1);
    Ok(())
}

#[test]
fn local_data_export_contains_active_profile_records() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.snapshot()?.active_profile_id;

    core.open_tab(UrlText::parse("https://example.com/research")?);
    core.bookmark_active_tab()?;
    core.save_active_url_note("profile note")?;
    core.save_active_tab_to_reading_list()?;
    core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;
    core.set_site_permission(
        SiteOrigin::parse("https://example.com")?,
        SitePermissionFeature::Camera,
        SitePermissionDecision::AllowAlways,
    )?;
    let archived_tab_id = core.open_tab(UrlText::parse("https://servo.org/")?);
    core.close_tab(&archived_tab_id)?;

    core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    core.open_tab(UrlText::parse("https://personal.example/research")?);
    core.bookmark_active_tab()?;

    core.select_profile(&default_profile_id)?;
    let package = core.export_local_data_package()?;
    let package_json = core.export_local_data_package_json()?;
    let document: serde_json::Value = serde_json::from_str(&package_json)?;

    assert_eq!(package.version(), ELYDATA_SCHEMA_VERSION);
    assert_eq!(package.profile_id(), default_profile_id.as_str());
    assert_eq!(package.profile_name(), "Default");
    assert_eq!(package.inventory().total_items(), 11);
    assert_eq!(document["version"], ELYDATA_SCHEMA_VERSION);
    assert_eq!(document["profile"]["id"], default_profile_id.as_str());
    assert_eq!(array_len(&document, "open_tabs"), 2);
    assert_eq!(array_len(&document, "archived_tabs"), 1);
    assert_eq!(array_len(&document, "bookmarks"), 1);
    assert_eq!(array_len(&document, "notes"), 1);
    assert_eq!(array_len(&document, "reading_list"), 1);
    assert_eq!(array_len(&document, "history"), 2);
    assert_eq!(array_len(&document, "downloads"), 1);
    assert_eq!(array_len(&document, "site_permissions"), 1);
    assert_eq!(array_len(&document, "site_permission_audit_events"), 1);
    assert_eq!(document["bookmarks"][0]["url"], "https://example.com/research");
    assert_eq!(document["downloads"][0]["file_name"], "report.pdf");
    assert_eq!(document["site_permissions"][0]["origin"], "https://example.com");
    Ok(())
}

#[test]
fn export_local_data_command_opens_privacy_security_page() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;

    core.set_command_query(">export-local-data");
    let intent = core.submit_command()?;
    let active_tab = core.active_tab()?;

    assert_eq!(intent, Some(CommandIntent::Command("export-local-data".to_string())));
    assert_eq!(active_tab.title(), "Privacy & Security Settings");
    assert_eq!(active_tab.url().as_str(), "ely://settings/privacy-security");
    assert_eq!(core.snapshot()?.command_query, "");
    Ok(())
}

#[test]
fn local_data_export_omits_ephemeral_allow_once_state() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let origin = SiteOrigin::parse("https://example.com")?;
    let feature = SitePermissionFeature::Camera;
    core.set_site_permission(origin.clone(), feature, SitePermissionDecision::AllowOnce)?;

    let document: serde_json::Value =
        serde_json::from_str(&core.export_local_data_package_json()?)?;
    let inventory = core.active_profile_local_data_inventory();

    assert_eq!(inventory.site_permissions(), 0);
    assert_eq!(array_len(&document, "site_permissions"), 0);
    assert_eq!(array_len(&document, "site_permission_audit_events"), 1);

    let revision = core.site_permission_revision(&profile_id, &origin, feature);
    assert!(core.transfer_site_permission_once(&profile_id, &origin, feature, revision)?);
    assert!(core.finish_site_permission_once(&profile_id, &origin, feature, revision)?);
    let consumed: serde_json::Value =
        serde_json::from_str(&core.export_local_data_package_json()?)?;
    assert_eq!(array_len(&consumed, "site_permission_audit_events"), 3);
    assert_eq!(consumed["site_permission_audit_events"][2]["action"]["kind"], "consumed");
    Ok(())
}

fn array_len(document: &serde_json::Value, field: &str) -> usize {
    document[field].as_array().map_or(0, Vec::len)
}
