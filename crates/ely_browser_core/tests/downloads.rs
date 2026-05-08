use std::error::Error;

use ely_browser_core::{BrowserCore, CoreError, InitialBrowserConfig};
use ely_domain::{
    DomainError, DownloadDestination, DownloadId, DownloadPolicy, DownloadSecurity, DownloadState,
    ProfileKind, UrlText,
};

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

#[test]
fn controls_download_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let download_id = core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;

    core.update_download_progress(&download_id, 1024)?;
    assert_eq!(active_download(&core)?.received_bytes(), 1024);

    core.pause_download(&download_id)?;
    assert_eq!(active_download(&core)?.state(), &DownloadState::Paused);

    core.resume_download(&download_id)?;
    assert_eq!(active_download(&core)?.state(), &DownloadState::InProgress);

    core.complete_download(&download_id, 2048)?;
    let completed = active_download(&core)?;
    assert_eq!(completed.state(), &DownloadState::Completed);
    assert_eq!(completed.received_bytes(), 2048);
    Ok(())
}

#[test]
fn records_active_profile_download_policy_on_started_entry() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.active_tab()?.profile_id().clone();
    let policy = DownloadPolicy::fixed_directory("/tmp/ely-work-downloads")?;
    core.set_profile_download_policy(&profile_id, policy.clone())?;

    core.record_download_started(
        UrlText::parse("https://example.com/installer.dmg")?,
        "installer.dmg",
        Some(4096),
    )?;

    let snapshot = core.snapshot()?;
    let entry = active_download(&core)?;
    assert_eq!(snapshot.active_download_policy, policy);
    assert_eq!(entry.destination(), policy.destination());
    assert_eq!(entry.security(), &DownloadSecurity::DangerousExtension);
    Ok(())
}

#[test]
fn download_policies_stay_with_profile() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let default_profile_id = core.active_tab()?.profile_id().clone();
    let default_policy = DownloadPolicy::fixed_directory("/tmp/ely-default-downloads")?;
    core.set_profile_download_policy(&default_profile_id, default_policy.clone())?;

    let personal_profile_id = core.create_profile("Personal", 0xf54e00, ProfileKind::Standard)?;
    assert_eq!(
        core.snapshot()?.active_download_policy.destination(),
        &DownloadDestination::AskEveryTime
    );

    let personal_policy = DownloadPolicy::fixed_directory("/tmp/ely-personal-downloads")?;
    core.set_profile_download_policy(&personal_profile_id, personal_policy.clone())?;
    assert_eq!(core.snapshot()?.active_download_policy, personal_policy);

    core.select_profile(&default_profile_id)?;
    assert_eq!(core.snapshot()?.active_download_policy, default_policy);
    Ok(())
}

#[test]
fn rejects_relative_download_directory() -> Result<(), Box<dyn Error>> {
    let error = match DownloadPolicy::fixed_directory("downloads") {
        Ok(_) => return Err("relative download directory should be rejected".into()),
        Err(error) => error,
    };

    assert_eq!(error, DomainError::InvalidDownloadDirectory { path: "downloads".to_string() });
    Ok(())
}

#[test]
fn rejects_path_like_download_file_name() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let error = match core.record_download_started(
        UrlText::parse("https://example.com/evil.sh")?,
        "../evil.sh",
        None,
    ) {
        Ok(_) => return Err("path-like download file name should be rejected".into()),
        Err(error) => error,
    };

    assert_eq!(
        error,
        CoreError::Domain(DomainError::InvalidFileName { value: "../evil.sh".to_string() })
    );
    Ok(())
}

#[test]
fn retries_cancelled_download_from_zero_bytes() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let download_id = core.record_download_started(
        UrlText::parse("https://example.com/archive.zip")?,
        "archive.zip",
        Some(4096),
    )?;

    core.update_download_progress(&download_id, 1024)?;
    core.cancel_download(&download_id)?;
    core.retry_download(&download_id)?;

    let retried = active_download(&core)?;
    assert_eq!(retried.state(), &DownloadState::InProgress);
    assert_eq!(retried.received_bytes(), 0);
    Ok(())
}

#[test]
fn rejects_invalid_download_transition() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let download_id = core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;

    core.complete_download(&download_id, 2048)?;
    let error = match core.pause_download(&download_id) {
        Ok(()) => return Err("completed download should reject pause".into()),
        Err(error) => error,
    };

    assert_eq!(
        error,
        CoreError::Domain(DomainError::InvalidDownloadTransition {
            action: "pause",
            state: "completed"
        })
    );
    Ok(())
}

#[test]
fn rejects_progress_above_total_bytes() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let download_id = core.record_download_started(
        UrlText::parse("https://example.com/report.pdf")?,
        "report.pdf",
        Some(2048),
    )?;

    let error = match core.update_download_progress(&download_id, 4096) {
        Ok(()) => return Err("progress above total should be rejected".into()),
        Err(error) => error,
    };

    assert_eq!(
        error,
        CoreError::Domain(DomainError::InvalidDownloadProgress {
            received_bytes: 4096,
            total_bytes: 2048,
        })
    );
    Ok(())
}

#[test]
fn rejects_unknown_download_id() -> Result<(), Box<dyn Error>> {
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let download_id = DownloadId::new();

    let error = match core.cancel_download(&download_id) {
        Ok(()) => return Err("unknown download should be rejected".into()),
        Err(error) => error,
    };

    assert_eq!(error, CoreError::DownloadNotFound { id: download_id });
    Ok(())
}

fn active_download(core: &BrowserCore) -> Result<ely_domain::DownloadEntry, Box<dyn Error>> {
    core.snapshot()?
        .download_entries
        .into_iter()
        .next()
        .ok_or_else(|| "download entry should exist".into())
}
