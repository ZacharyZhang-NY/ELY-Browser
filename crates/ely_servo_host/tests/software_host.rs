#![cfg(feature = "servo-engine")]

use std::{
    env,
    error::Error,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use ely_domain::{ProfileId, TabId, UrlText};
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, MouseDragRequest, NavigationRequest, ResizeRequest,
    ScreenshotRequest, ScrollRequest, ServoHost, ServoHostError, ServoSurfaceSize,
    SoftwareServoHost, TouchTapRequest, WebViewState,
};

const MINIMUM_CONTENT_PIXELS: u64 = 1_000;
const INITIAL_WIDTH: u32 = 640;
const INITIAL_HEIGHT: u32 = 480;
const RESIZED_WIDTH: u32 = 934;
const RESIZED_HEIGHT: u32 = 657;
const PRD_SITE_COMPATIBILITY_CASES: &[PrdSiteCompatibilityCase] = &[
    PrdSiteCompatibilityCase { url: "https://example.com", title_fragment: "Example Domain" },
    PrdSiteCompatibilityCase { url: "https://servo.org/", title_fragment: "Servo" },
];
const SOFTWARE_HOST_CHILD_ENV: &str = "ELY_SERVO_SOFTWARE_HOST_CHILD";
const CLICK_PROBE_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EClick%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Clicked%27%3Bthis.textContent%3D%27Clicked%27%3B%22%3ETap%3C%2Fbutton%3E";
const DRAG_PROBE_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3EDrag%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20id%3Dbox%3EDrag%3C%2Fbutton%3E%3Cscript%3Elet%20dragging%3Dfalse%3Bconst%20box%3Ddocument.getElementById%28%27box%27%29%3BaddEventListener%28%27mousedown%27%2Cevent%3D%3E%7Bif%28event.target%3D%3D%3Dbox%29%7Bdragging%3Dtrue%3B%7D%7D%29%3BaddEventListener%28%27mousemove%27%2Cevent%3D%3E%7Bif%28dragging%26%26event.clientX%3E280%29%7Bdocument.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Dragged%27%3Bbox.textContent%3D%27Dragged%27%3B%7D%7D%29%3BaddEventListener%28%27mouseup%27%2C%28%29%3D%3E%7Bdragging%3Dfalse%3B%7D%29%3B%3C%2Fscript%3E";
const TOUCH_PROBE_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3ETouch%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3Btouch-action%3Amanipulation%3B%7D%3C%2Fstyle%3E%3Cbutton%20ontouchstart%3D%22document.body.dataset.touch%3D%27start%27%3B%22%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Touched%27%3Bthis.textContent%3D%27Touched%27%3B%22%3ETap%3C%2Fbutton%3E";
const TEXT_PROBE_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EText%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3Bfont%3A28px%20sans-serif%3B%7Dinput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A260px%3Bheight%3A70px%3Bfont%3A28px%20sans-serif%3B%7Doutput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A180px%3Bfont%3A32px%20sans-serif%3B%7D%3C%2Fstyle%3E%3Cinput%20id%3Dq%20autofocus%20oninput%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.getElementById%28%27out%27%29.textContent%3Dthis.value%3B%22%3E%3Coutput%20id%3Dout%3Eempty%3C%2Foutput%3E";
const TEXT_PROBE_VALUE: &str = "ely42";

struct PrdSiteCompatibilityCase {
    url: &'static str,
    title_fragment: &'static str,
}

#[test]
fn manages_real_servo_webview_lifecycle() -> Result<(), Box<dyn Error>> {
    if env::var_os(SOFTWARE_HOST_CHILD_ENV).is_none() {
        return run_isolated_software_host_lifecycle();
    }

    exercise_real_servo_webview_lifecycle()
}

fn run_isolated_software_host_lifecycle() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("manages_real_servo_webview_lifecycle")
        .env(SOFTWARE_HOST_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "isolated software host lifecycle failed\nstatus: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn exercise_real_servo_webview_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(INITIAL_WIDTH, INITIAL_HEIGHT))?;
    let tab_id = TabId::new();
    let profile_id = ProfileId::new();

    let webview_id = host.create_webview(tab_id.clone(), profile_id.clone())?;
    let snapshot = host.snapshot(&webview_id)?;

    assert_eq!(snapshot.webview_id(), &webview_id);
    assert_eq!(snapshot.tab_id(), &tab_id);
    assert_eq!(snapshot.profile_id(), &profile_id);
    assert_eq!(snapshot.state(), &WebViewState::Created);

    let url = UrlText::parse(CLICK_PROBE_URL)?;

    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;

    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert!(
        snapshot.url().is_some_and(|value| value.starts_with("data:text/html,")),
        "snapshot: {snapshot:?}"
    );
    assert_rendered_frame_has_content(&host, "data:text/html", 1)?;

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.click(MouseClickRequest { webview_id: webview_id.clone(), x: 160, y: 120 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html clicked", 1)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    let tab_id = TabId::new();
    let url = UrlText::parse(DRAG_PROBE_URL)?;
    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html drag", 1)?;

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.drag(MouseDragRequest {
        webview_id: webview_id.clone(),
        from_x: 160,
        from_y: 120,
        to_x: 320,
        to_y: 120,
    })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html dragged", 1)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    let tab_id = TabId::new();
    let url = UrlText::parse(TOUCH_PROBE_URL)?;
    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html touch", 1)?;

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.touch_tap(TouchTapRequest { webview_id: webview_id.clone(), x: 160, y: 120 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html touched", 1)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    let tab_id = TabId::new();
    let url = UrlText::parse(TEXT_PROBE_URL)?;
    host.navigate(NavigationRequest { webview_id: webview_id.clone(), tab_id, url })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html input", 1)?;

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.click(MouseClickRequest { webview_id: webview_id.clone(), x: 160, y: 120 })?;
    host.type_text(KeyboardTextRequest {
        webview_id: webview_id.clone(),
        text: TEXT_PROBE_VALUE.to_string(),
    })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html typed", 1)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

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
        if site.url == "https://example.com" {
            let screenshot =
                host.capture_screenshot(ScreenshotRequest { webview_id: webview_id.clone() })?;
            assert_frame_has_dimensions_and_content(
                &screenshot,
                "https://example.com screenshot",
                INITIAL_WIDTH,
                INITIAL_HEIGHT,
                MINIMUM_CONTENT_PIXELS,
            );
        }
        previous_frame_hash = Some(host.last_rendered_frame()?.sample_hash());
    }

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.scroll(ScrollRequest { webview_id: webview_id.clone(), delta_x: 0, delta_y: 480 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "https://servo.org scrolled", MINIMUM_CONTENT_PIXELS)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.resize(ResizeRequest {
        webview_id: webview_id.clone(),
        width: RESIZED_WIDTH,
        height: RESIZED_HEIGHT,
    })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_dimensions_and_content(
        &host,
        "https://servo.org resized",
        RESIZED_WIDTH,
        RESIZED_HEIGHT,
        MINIMUM_CONTENT_PIXELS,
    )?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    assert!(matches!(
        SoftwareServoHost::new(ServoSurfaceSize::new(INITIAL_WIDTH, INITIAL_HEIGHT)),
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
    assert_rendered_frame_has_dimensions_and_content(
        host,
        label,
        INITIAL_WIDTH,
        INITIAL_HEIGHT,
        minimum_content_pixels,
    )
}

fn assert_rendered_frame_has_dimensions_and_content(
    host: &SoftwareServoHost,
    label: &str,
    expected_width: u32,
    expected_height: u32,
    minimum_content_pixels: u64,
) -> Result<(), Box<dyn Error>> {
    let frame = host.last_rendered_frame()?;
    assert_frame_has_dimensions_and_content(
        &frame,
        label,
        expected_width,
        expected_height,
        minimum_content_pixels,
    );
    Ok(())
}

fn assert_frame_has_dimensions_and_content(
    frame: &ely_servo_host::RenderedFrame,
    label: &str,
    expected_width: u32,
    expected_height: u32,
    minimum_content_pixels: u64,
) {
    assert_eq!(frame.width(), expected_width, "{label}: {frame:?}");
    assert_eq!(frame.height(), expected_height, "{label}: {frame:?}");
    assert!(frame.opaque_pixel_count() > 0, "{label}: {frame:?}");
    assert!(frame.non_white_pixel_count() > 0, "{label}: {frame:?}");
    assert!(frame.content_pixel_count() >= minimum_content_pixels, "{label}: {frame:?}");
    assert_ne!(frame.sample_hash(), 0, "{label}: {frame:?}");
}
