#![cfg(feature = "servo-engine")]

use std::{error::Error, thread, time::Duration};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    NavigationRequest, ScrollRequest, ServoHost, ServoHostError, ServoSurfaceSize,
    SoftwareServoHost, WebViewState,
};

const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const PRD_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase { url: "https://example.com", title_fragment: "Example Domain" },
    PrdSiteCompatibilityCase { url: "https://servo.org", title_fragment: "Servo" },
];

struct PrdSiteCompatibilityCase {
    url: &'static str,
    title_fragment: &'static str,
}

#[test]
fn manages_real_servo_webview_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(640, 480))?;
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();

    let webview_id = host.create_webview(tab_id.clone(), profile_id.clone())?;
    let snapshot = host.snapshot(&webview_id)?;

    assert_eq!(snapshot.webview_id(), &webview_id);
    assert_eq!(snapshot.tab_id(), &tab_id);
    assert_eq!(snapshot.profile_id(), &profile_id);
    assert_eq!(snapshot.state(), &WebViewState::Created);

    let url = UrlText::parse(
        "data:text/html,%3Ctitle%3EELY%20Host%3C%2Ftitle%3E%3Cmain%3EReady%3C%2Fmain%3E",
    )?;

    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;

    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert!(
        snapshot.url().is_some_and(|value| value.starts_with("data:text/html,")),
        "snapshot: {snapshot:?}"
    );
    assert_rendered_frame_has_content(&host, "data:text/html", 1)?;

    let mut previous_frame_hash = Some(host.last_rendered_frame()?.sample_hash());
    for site in PRD_SITE_COMPATIBILITY_CASES {
        let tab_id = TabId::new();
        let url = UrlText::parse(site.url)?;

        host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;
        let snapshot = wait_for_rendered_webview(&mut host, &webview_id, previous_frame_hash)?;

        assert_eq!(snapshot.state(), &WebViewState::Complete, "{}: {snapshot:?}", site.url);
        assert!(
            snapshot.url().is_some_and(|value| value.starts_with(site.url)),
            "{}: {snapshot:?}",
            site.url
        );
        assert!(
            snapshot.title().is_some_and(|value| value.contains(site.title_fragment)),
            "{}: {snapshot:?}",
            site.url
        );
        assert_rendered_frame_has_content(&host, site.url, MINIMUM_CONTENT_PIXELS)?;
        previous_frame_hash = Some(host.last_rendered_frame()?.sample_hash());
    }

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.scroll(ScrollRequest { webview_id: webview_id.clone(), delta_x: 0, delta_y: 480 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "https://servo.org scrolled", MINIMUM_CONTENT_PIXELS)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    assert!(matches!(
        SoftwareServoHost::new(ServoSurfaceSize::new(640, 480)),
        Err(ServoHostError::RuntimeAlreadyStarted)
    ));
    Ok(())
}

fn wait_for_rendered_webview(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: Option<u64>,
) -> Result<ely_servo_host::WebViewSnapshot, Box<dyn Error>> {
    let mut painted_since_request = false;

    for _ in 0..5_000 {
        host.tick();
        let snapshot = host.snapshot(webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(webview_id)?;
            painted_since_request = true;
        }

        let snapshot = host.snapshot(webview_id)?;
        let has_rendered_current_request = host.last_rendered_frame().is_ok_and(|frame| {
            painted_since_request
                && Some(frame.sample_hash()) != previous_frame_hash
                && frame.non_white_pixel_count() > 0
        });

        if snapshot.state() == &WebViewState::Complete && has_rendered_current_request {
            return Ok(snapshot);
        }

        thread::sleep(Duration::from_millis(2));
    }

    Err(format!("timed out waiting for rendered webview: {:?}", host.snapshot(webview_id)?).into())
}

fn assert_rendered_frame_has_content(
    host: &SoftwareServoHost,
    label: &str,
    minimum_content_pixels: u64,
) -> Result<(), Box<dyn Error>> {
    let frame = host.last_rendered_frame()?;

    assert_eq!(frame.width(), 640, "{label}: {frame:?}");
    assert_eq!(frame.height(), 480, "{label}: {frame:?}");
    assert!(frame.opaque_pixel_count() > 0, "{label}: {frame:?}");
    assert!(frame.non_white_pixel_count() > 0, "{label}: {frame:?}");
    assert!(frame.content_pixel_count() >= minimum_content_pixels, "{label}: {frame:?}");
    assert_ne!(frame.sample_hash(), 0, "{label}: {frame:?}");
    Ok(())
}
