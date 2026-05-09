use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ProfileKind, SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText};

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
