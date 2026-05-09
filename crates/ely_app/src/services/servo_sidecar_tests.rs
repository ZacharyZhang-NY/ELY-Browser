use std::error::Error;

use super::*;

#[cfg(feature = "live-site-smoke")]
use crate::services::prd_live_sites::{
    LiveSiteCase, PRD_REFERENCE_SITE_CASES, PRD_TOP_SITE_CASES,
    assert_prd_reference_urls_are_covered,
};

#[cfg(feature = "live-site-smoke")]
const LIVE_SITE_WIDTH: u32 = 934;
#[cfg(feature = "live-site-smoke")]
const LIVE_SITE_HEIGHT: u32 = 657;
#[cfg(feature = "live-site-smoke")]
const MINIMUM_CONTENT_PIXELS: u64 = 1_000;

#[test]
fn accepts_loading_report_with_visible_content() -> Result<(), ServoSidecarError> {
    let snapshot = SidecarSnapshot::from_report(report_with_state("loading"), visible_frame())?;

    assert_eq!(snapshot.loaded_url(), Some("https://example.com/"));
    assert_eq!(snapshot.title(), Some("Example Domain"));
    assert_eq!(snapshot.width(), 2);
    assert_eq!(snapshot.height(), 1);
    assert_eq!(snapshot.non_white_pixel_count, 1);
    assert_eq!(snapshot.content_pixel_count, 1);
    assert_eq!(snapshot.sample_hash, 42);
    Ok(())
}

#[test]
fn rejects_created_report_with_visible_content() {
    let result = SidecarSnapshot::from_report(report_with_state("created"), visible_frame());

    assert!(
        matches!(result, Err(ServoSidecarError::IncompleteRender { state }) if state == "created")
    );
}

#[test]
fn retries_navigation_snapshots() -> Result<(), Box<dyn Error>> {
    let request = SidecarSnapshotRequest::new(UrlText::parse("https://example.com")?, 2, 1);

    assert_eq!(request.max_attempts(), SIDECAR_NAVIGATION_ATTEMPTS);
    Ok(())
}

#[test]
fn keeps_page_interactions_single_attempt() -> Result<(), Box<dyn Error>> {
    let request = SidecarSnapshotRequest::new(UrlText::parse("https://example.com")?, 2, 1)
        .with_click_point(1, 1)
        .with_typed_text("ely".to_string());

    assert_eq!(request.max_attempts(), SIDECAR_INTERACTION_ATTEMPTS);
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
        let request = SidecarSnapshotRequest::new(
            UrlText::parse(case.url)?,
            LIVE_SITE_WIDTH,
            LIVE_SITE_HEIGHT,
        );
        let snapshot = client.snapshot(request)?;

        assert_eq!(snapshot.width(), LIVE_SITE_WIDTH, "{}", case.url);
        assert_eq!(snapshot.height(), LIVE_SITE_HEIGHT, "{}", case.url);
        assert_loaded_url_contains(&snapshot, case.url)?;
        assert_title_contains(&snapshot, case.title_fragment)?;
        assert!(snapshot.non_white_pixel_count > 0, "{}", case.url);
        assert!(snapshot.content_pixel_count >= MINIMUM_CONTENT_PIXELS, "{}", case.url);
        assert!(snapshot.sample_hash > 0, "{}", case.url);

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
fn assert_loaded_url_contains(
    snapshot: &SidecarSnapshot,
    fragment: &str,
) -> Result<(), Box<dyn Error>> {
    let loaded_url =
        snapshot.loaded_url().ok_or_else(|| format!("missing loaded URL for {fragment}"))?;
    assert!(loaded_url.contains(fragment), "loaded_url: {loaded_url}");
    Ok(())
}

#[cfg(feature = "live-site-smoke")]
fn assert_title_contains(snapshot: &SidecarSnapshot, fragment: &str) -> Result<(), Box<dyn Error>> {
    let title = snapshot.title().ok_or_else(|| format!("missing title containing {fragment}"))?;
    assert!(title.contains(fragment), "title: {title}");
    Ok(())
}

fn report_with_state(state: &str) -> SidecarReport {
    SidecarReport {
        requested_url: "https://example.com".to_string(),
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
