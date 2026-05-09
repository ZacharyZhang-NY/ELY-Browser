use std::error::Error;

use ely_domain::UrlText;

use super::*;

#[cfg(feature = "live-site-smoke")]
use crate::services::prd_live_sites::{
    LiveSiteCase, PRD_REFERENCE_SITE_CASES, PRD_TOP_SITE_CASES,
    assert_prd_reference_urls_are_covered,
};

#[cfg(feature = "live-site-smoke")]
const LIVE_SITE_RENDER_ATTEMPTS: usize = 3;
#[cfg(feature = "live-site-smoke")]
const LIVE_SITE_WIDTH: u32 = 934;
#[cfg(feature = "live-site-smoke")]
const LIVE_SITE_HEIGHT: u32 = 657;
#[cfg(feature = "live-site-smoke")]
const MINIMUM_CONTENT_PIXELS: u64 = 1_000;

#[test]
fn accepts_loading_report_with_visible_content() -> Result<(), ServoSidecarError> {
    let profile_id = ProfileId::new();
    let snapshot = SidecarSnapshot::from_report(
        report_with_state("loading", &profile_id),
        &profile_id,
        visible_frame(),
    )?;

    assert_eq!(snapshot.loaded_url(), Some("https://example.com/"));
    assert_eq!(snapshot.title(), Some("Example Domain"));
    assert_eq!(snapshot.render_state(), "loading");
    assert_eq!(snapshot.width(), 2);
    assert_eq!(snapshot.height(), 1);
    assert_eq!(snapshot.non_white_pixel_count, 1);
    assert_eq!(snapshot.content_pixel_count, 1);
    assert_eq!(snapshot.sample_hash, 42);
    Ok(())
}

#[test]
fn rejects_created_report_with_visible_content() {
    let profile_id = ProfileId::new();
    let result = SidecarSnapshot::from_report(
        report_with_state("created", &profile_id),
        &profile_id,
        visible_frame(),
    );

    assert!(
        matches!(result, Err(ServoSidecarError::IncompleteRender { state }) if state == "created")
    );
}

#[test]
fn rejects_report_from_different_profile() {
    let expected_profile_id = ProfileId::new();
    let actual_profile_id = ProfileId::new();
    let result = SidecarSnapshot::from_report(
        report_with_state("loading", &actual_profile_id),
        &expected_profile_id,
        visible_frame(),
    );

    assert!(matches!(result, Err(ServoSidecarError::ProfileMismatch { expected, actual })
            if expected == expected_profile_id.as_str() && actual == actual_profile_id.as_str()));
}

#[test]
fn retries_navigation_snapshots() -> Result<(), Box<dyn Error>> {
    let request =
        SidecarSnapshotRequest::new(UrlText::parse("https://example.com")?, ProfileId::new(), 2, 1);

    assert_eq!(request.max_attempts(), SIDECAR_NAVIGATION_ATTEMPTS);
    Ok(())
}

#[test]
fn keeps_page_interactions_single_attempt() -> Result<(), Box<dyn Error>> {
    let request =
        SidecarSnapshotRequest::new(UrlText::parse("https://example.com")?, ProfileId::new(), 2, 1)
            .with_click_point(1, 1)
            .with_typed_text("ely".to_string());

    assert_eq!(request.max_attempts(), SIDECAR_INTERACTION_ATTEMPTS);
    Ok(())
}

#[test]
fn persistent_profile_data_uses_profile_root() -> Result<(), Box<dyn Error>> {
    let profile_id = ProfileId::new();
    let request = SidecarSnapshotRequest::new(
        UrlText::parse("https://example.com")?,
        profile_id.clone(),
        2,
        1,
    );
    let root = std::env::temp_dir().join("ely-browser-profile-root-test");

    assert_eq!(request.profile_data_mode_for_test(), ProfileDataMode::Persistent);
    assert_eq!(request.profile_data_dir(&root)?, root.join(profile_id.as_str()).join("servo"));
    Ok(())
}

#[test]
fn transient_profile_data_uses_temporary_directory_and_cleans_up() -> Result<(), Box<dyn Error>> {
    let profile_id = ProfileId::new();
    let request = SidecarSnapshotRequest::new(
        UrlText::parse("https://example.com")?,
        profile_id.clone(),
        2,
        1,
    )
    .with_profile_data_mode(ProfileDataMode::Transient);
    let root = std::env::temp_dir().join("ely-browser-profile-root-test");
    let profile_data_dir = request.profile_data_dir(&root)?;

    assert!(profile_data_dir.starts_with(std::env::temp_dir().join("ely-browser-servo-profiles")));
    assert!(profile_data_dir.to_string_lossy().contains(profile_id.as_str()));
    std::fs::create_dir_all(&profile_data_dir)?;
    std::fs::write(profile_data_dir.join("probe"), b"private")?;

    request.cleanup_profile_data_dir(&profile_data_dir)?;

    assert!(!profile_data_dir.exists());
    Ok(())
}

#[cfg(feature = "live-site-smoke")]
#[test]
fn desktop_sidecar_opens_prd_top_sites() -> Result<(), Box<dyn Error>> {
    assert_live_sites_render(PRD_TOP_SITE_CASES)
}

#[cfg(feature = "live-site-smoke")]
#[test]
fn desktop_sidecar_opens_prd_reference_sites() -> Result<(), Box<dyn Error>> {
    assert_live_sites_render(PRD_REFERENCE_SITE_CASES)
}

#[cfg(feature = "live-site-smoke")]
#[test]
fn prd_reference_live_site_cases_cover_prd_urls() -> Result<(), Box<dyn Error>> {
    assert_prd_reference_urls_are_covered()
}

#[cfg(feature = "live-site-smoke")]
fn assert_live_sites_render(cases: &[LiveSiteCase]) -> Result<(), Box<dyn Error>> {
    let client = ServoSidecarClient::new()?;
    for case in cases {
        let snapshot = render_live_site_snapshot(&client, case)?;
        let rgba_bytes = snapshot.into_rgba_bytes();
        assert_eq!(
            rgba_bytes.len(),
            expected_rgba_byte_count(LIVE_SITE_WIDTH, LIVE_SITE_HEIGHT)?,
            "{}",
            case.url
        );
    }
    Ok(())
}

#[cfg(feature = "live-site-smoke")]
fn render_live_site_snapshot(
    client: &ServoSidecarClient,
    case: &LiveSiteCase,
) -> Result<SidecarSnapshot, Box<dyn Error>> {
    let mut last_error = String::new();

    for attempt in 0..LIVE_SITE_RENDER_ATTEMPTS {
        let request = SidecarSnapshotRequest::new(
            UrlText::parse(case.url)?,
            ProfileId::new(),
            LIVE_SITE_WIDTH,
            LIVE_SITE_HEIGHT,
        );

        match client.snapshot(request) {
            Ok(snapshot) => match validate_live_site_snapshot(&snapshot, case) {
                Ok(()) => return Ok(snapshot),
                Err(error) => last_error = error,
            },
            Err(error) => last_error = error.to_string(),
        }

        if attempt + 1 < LIVE_SITE_RENDER_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    Err(last_error.into())
}

#[cfg(feature = "live-site-smoke")]
fn validate_live_site_snapshot(
    snapshot: &SidecarSnapshot,
    case: &LiveSiteCase,
) -> Result<(), String> {
    require(
        snapshot.width() == LIVE_SITE_WIDTH,
        format!("{} width: {}", case.url, snapshot.width()),
    )?;
    require(
        snapshot.height() == LIVE_SITE_HEIGHT,
        format!("{} height: {}", case.url, snapshot.height()),
    )?;
    require_render_state_is_open(snapshot.render_state(), case.url)?;
    require_loaded_url_contains(snapshot, case.url)?;
    require_title_contains(snapshot, case.title_fragment)?;
    require(snapshot.non_white_pixel_count > 0, case.url.to_string())?;
    require(
        snapshot.content_pixel_count >= MINIMUM_CONTENT_PIXELS,
        format!("{} content pixels: {}", case.url, snapshot.content_pixel_count),
    )?;
    require(snapshot.sample_hash > 0, case.url.to_string())
}

#[cfg(feature = "live-site-smoke")]
fn require_render_state_is_open(state: &str, url: &str) -> Result<(), String> {
    require(matches!(state, "complete" | "loading"), format!("{url} state: {state}"))
}

#[cfg(feature = "live-site-smoke")]
fn require_loaded_url_contains(snapshot: &SidecarSnapshot, fragment: &str) -> Result<(), String> {
    let loaded_url =
        snapshot.loaded_url().ok_or_else(|| format!("missing loaded URL for {fragment}"))?;
    require(loaded_url.contains(fragment), format!("loaded_url: {loaded_url}"))
}

#[cfg(feature = "live-site-smoke")]
fn require_title_contains(snapshot: &SidecarSnapshot, fragment: &str) -> Result<(), String> {
    let title = snapshot.title().ok_or_else(|| format!("missing title containing {fragment}"))?;
    require(title.contains(fragment), format!("title: {title}"))
}

#[cfg(feature = "live-site-smoke")]
fn require(condition: bool, message: String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message) }
}

fn report_with_state(state: &str, profile_id: &ProfileId) -> SidecarReport {
    SidecarReport {
        requested_url: "https://example.com".to_string(),
        profile_id: profile_id.as_str().to_string(),
        loaded_url: Some("https://example.com/".to_string()),
        title: Some("Example Domain".to_string()),
        state: state.to_string(),
        width: 2,
        height: 1,
        rgba_byte_count: 8,
        non_white_pixel_count: 1,
        content_pixel_count: 1,
        sample_hash: 42,
    }
}

fn visible_frame() -> Vec<u8> {
    vec![0, 0, 0, 255, 255, 255, 255, 255]
}
