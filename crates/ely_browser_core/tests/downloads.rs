use std::error::Error;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{ProfileKind, UrlText};

#[test]
fn download_entries_stay_with_active_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();

    core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;

    let default_snapshot = core.snapshot()?;
    assert_eq!(default_snapshot.download_entries.len(), 1);
    assert_eq!(default_snapshot.download_entries[0].file_name(), "report.pdf");
    assert_eq!(default_snapshot.download_entries[0].profile_id(), &default_profile_id);

    core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    let personal_snapshot = core.snapshot()?;
    assert!(personal_snapshot.download_entries.is_empty());

    core.record_download_started(
        UrlText::parse("https://example.com/archive.zip")?,
        "archive.zip",
        None,
    )?;
    assert_eq!(core.snapshot()?.download_entries.len(), 1);

    core.select_profile(&default_profile_id)?;
    let default_snapshot = core.snapshot()?;
    assert_eq!(default_snapshot.download_entries.len(), 1);
    assert_eq!(default_snapshot.download_entries[0].file_name(), "report.pdf");
    Ok(())
}
