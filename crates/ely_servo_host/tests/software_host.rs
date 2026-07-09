#![cfg(feature = "servo-engine")]

use std::{
    env,
    error::Error,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature, TabId, UrlText};
use ely_servo_host::{
    HidpiScaleRequest, KeyboardTextRequest, MouseClickRequest, MouseDragRequest, NavigationRequest,
    PageZoomRequest, PermissionDecision, PermissionRequest, ResizeRequest, ScrollRequest,
    ServoHost, ServoHostError, ServoSurfaceSize, SoftwareServoHost, TouchTapRequest, WebViewState,
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
const DPR_VIEWPORT_CHILD_ENV: &str = "ELY_SERVO_DPR_VIEWPORT_CHILD";
const CLICK_PROBE_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EClick%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20onclick%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Clicked%27%3Bthis.textContent%3D%27Clicked%27%3B%22%3ETap%3C%2Fbutton%3E";
const DRAG_PROBE_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3EDrag%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3B%7D%3C%2Fstyle%3E%3Cbutton%20id%3Dbox%3EDrag%3C%2Fbutton%3E%3Cscript%3Elet%20dragging%3Dfalse%3Bconst%20box%3Ddocument.getElementById%28%27box%27%29%3BaddEventListener%28%27mousedown%27%2Cevent%3D%3E%7Bif%28event.target%3D%3D%3Dbox%29%7Bdragging%3Dtrue%3B%7D%7D%29%3BaddEventListener%28%27mousemove%27%2Cevent%3D%3E%7Bif%28dragging%26%26event.clientX%3E280%29%7Bdocument.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Dragged%27%3Bbox.textContent%3D%27Dragged%27%3B%7D%7D%29%3BaddEventListener%28%27mouseup%27%2C%28%29%3D%3E%7Bdragging%3Dfalse%3B%7D%29%3B%3C%2Fscript%3E";
const TOUCH_PROBE_URL: &str = "data:text/html,%3C%21doctype%20html%3E%3Ctitle%3ETouch%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3B%7Dbutton%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A220px%3Bheight%3A90px%3Bfont%3A28px%20sans-serif%3Bbackground%3A%23ffffff%3Bcolor%3A%23111111%3Btouch-action%3Amanipulation%3B%7D%3C%2Fstyle%3E%3Cbutton%20ontouchstart%3D%22document.body.dataset.touch%3D%27start%27%3B%22%20onpointerdown%3D%22if%28%21document.body.dataset.pointerType%29%7Bdocument.body.dataset.pointerType%3Devent.pointerType%3B%7D%22%20onclick%3D%22if%28document.body.dataset.pointerType%21%3D%3D%27touch%27%29%7Bdocument.title%3Ddocument.body.dataset.pointerType%3Breturn%3B%7Ddocument.body.style.background%3D%27%230039ff%27%3Bdocument.title%3D%27Touched%27%3Bthis.textContent%3D%27Touched%27%3B%22%3ETap%3C%2Fbutton%3E";
const TEXT_PROBE_URL: &str = "data:text/html,%3C!doctype%20html%3E%3Ctitle%3EText%20Probe%3C%2Ftitle%3E%3Cstyle%3Ebody%7Bmargin%3A0%3Bbackground%3A%23f7f7f7%3Bfont%3A28px%20sans-serif%3B%7Dinput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A80px%3Bwidth%3A260px%3Bheight%3A70px%3Bfont%3A28px%20sans-serif%3B%7Doutput%7Bposition%3Aabsolute%3Bleft%3A80px%3Btop%3A180px%3Bfont%3A32px%20sans-serif%3B%7D%3C%2Fstyle%3E%3Cinput%20id%3Dq%20autofocus%20oninput%3D%22document.body.style.background%3D%27%230039ff%27%3Bdocument.getElementById%28%27out%27%29.textContent%3Dthis.value%3B%22%3E%3Coutput%20id%3Dout%3Eempty%3C%2Foutput%3E";
const TEXT_PROBE_VALUE: &str = "ely42";

struct PrdSiteCompatibilityCase {
    url: &'static str,
    title_fragment: &'static str,
}

struct DprViewportCase {
    physical_width: u32,
    physical_height: u32,
    dpr: f32,
    expected_css_width: u32,
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

#[test]
fn first_navigation_preserves_dpr_for_css_viewport() -> Result<(), Box<dyn Error>> {
    if env::var_os(DPR_VIEWPORT_CHILD_ENV).is_none() {
        return run_isolated_dpr_viewport_test();
    }

    exercise_dpr_viewport_cases()
}

fn run_isolated_dpr_viewport_test() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("first_navigation_preserves_dpr_for_css_viewport")
        .env(DPR_VIEWPORT_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "isolated DPR viewport test failed\nstatus: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn exercise_dpr_viewport_cases() -> Result<(), Box<dyn Error>> {
    let mut host = SoftwareServoHost::new(ServoSurfaceSize::new(1, 1))?;
    let profile_id = ProfileId::new();
    let cases = [
        DprViewportCase {
            physical_width: 800,
            physical_height: 600,
            dpr: 1.0,
            expected_css_width: 800,
        },
        DprViewportCase {
            physical_width: 960,
            physical_height: 640,
            dpr: 2.0,
            expected_css_width: 480,
        },
        DprViewportCase {
            physical_width: 1250,
            physical_height: 800,
            dpr: 1.25,
            expected_css_width: 1000,
        },
        DprViewportCase {
            physical_width: 1440,
            physical_height: 900,
            dpr: 1.5,
            expected_css_width: 960,
        },
        DprViewportCase {
            physical_width: 1750,
            physical_height: 1000,
            dpr: 1.75,
            expected_css_width: 1000,
        },
        DprViewportCase {
            physical_width: 1500,
            physical_height: 900,
            dpr: 2.5,
            expected_css_width: 600,
        },
        DprViewportCase {
            physical_width: 2160,
            physical_height: 1440,
            dpr: 3.0,
            expected_css_width: 720,
        },
    ];

    for case in cases {
        let tab_id = TabId::new();
        let webview_id = host.create_webview_with_size(
            tab_id.clone(),
            profile_id.clone(),
            ServoSurfaceSize::new(case.physical_width, case.physical_height),
        )?;

        host.set_hidpi_scale(HidpiScaleRequest {
            webview_id: webview_id.clone(),
            scale_factor: case.dpr,
        })?;
        host.navigate(NavigationRequest {
            webview_id: webview_id.clone(),
            tab_id,
            url: UrlText::parse(viewport_probe_url(case.expected_css_width + 1))?,
        })?;

        let snapshot = wait_for_rendered_webview(&mut host, &webview_id, None)?;
        assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");

        let frame = host.last_rendered_frame()?;
        assert_eq!(frame.width(), case.physical_width, "DPR case frame width");
        assert_eq!(frame.height(), case.physical_height, "DPR case frame height");
        assert_eq!(
            center_pixel_rgb(&frame),
            [238, 32, 77],
            "CSS viewport must equal physical width divided by DPR: physical={} dpr={} expected_css={}",
            case.physical_width,
            case.dpr,
            case.expected_css_width,
        );
        host.close_webview(&webview_id);
    }
    Ok(())
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

    host.set_permission(
        PermissionRequest {
            webview_id: webview_id.clone(),
            profile_id: profile_id.clone(),
            origin: SiteOrigin::parse("https://example.com")?,
            feature: SitePermissionFeature::Camera,
        },
        PermissionDecision::AllowOnce,
    )?;
    let other_profile_id = ProfileId::new();
    let mismatch = host.set_permission(
        PermissionRequest {
            webview_id: webview_id.clone(),
            profile_id: other_profile_id.clone(),
            origin: SiteOrigin::parse("https://example.com")?,
            feature: SitePermissionFeature::Camera,
        },
        PermissionDecision::AllowAlways,
    );
    assert!(
        matches!(
            mismatch,
            Err(ServoHostError::PermissionProfileMismatch { ref expected, ref actual, .. })
                if expected == &profile_id && actual == &other_profile_id
        ),
        "mismatch: {mismatch:?}"
    );

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
    host.set_page_zoom(PageZoomRequest { webview_id: webview_id.clone(), zoom_factor: 1.25 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");
    assert_rendered_frame_has_content(&host, "data:text/html zoomed", 1)?;
    assert_ne!(host.last_rendered_frame()?.sample_hash(), previous_frame_hash);

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.set_page_zoom(PageZoomRequest { webview_id: webview_id.clone(), zoom_factor: 1.0 })?;
    let snapshot = wait_for_rendered_webview(&mut host, &webview_id, Some(previous_frame_hash))?;
    assert_eq!(snapshot.state(), &WebViewState::Complete, "snapshot: {snapshot:?}");

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
    assert_eq!(snapshot.title(), Some("Touched"), "snapshot: {snapshot:?}");
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
        previous_frame_hash = Some(host.last_rendered_frame()?.sample_hash());
    }

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.scroll(ScrollRequest {
        webview_id: webview_id.clone(),
        delta_x: 0,
        delta_y: 480,
        point_x: 0,
        point_y: 0,
    })?;
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

    assert!(host.close_webview(&webview_id));
    assert!(!host.close_webview(&webview_id));
    assert!(matches!(host.snapshot(&webview_id), Err(ServoHostError::WebViewNotFound { .. })));

    let replacement_tab_id = TabId::new();
    let replacement_id = host.create_webview(replacement_tab_id.clone(), profile_id)?;
    host.navigate(NavigationRequest {
        webview_id: replacement_id.clone(),
        tab_id: replacement_tab_id,
        url: UrlText::parse(CLICK_PROBE_URL)?,
    })?;
    let replacement = wait_for_rendered_webview(&mut host, &replacement_id, None)?;
    assert_eq!(replacement.state(), &WebViewState::Complete, "replacement: {replacement:?}");

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

fn center_pixel_rgb(frame: &ely_servo_host::RenderedFrame) -> [u8; 3] {
    let x = frame.width() / 2;
    let y = frame.height() / 2;
    let index = ((y * frame.width() + x) * 4) as usize;
    let rgba = &frame.rgba_bytes()[index..index + 4];
    [rgba[0], rgba[1], rgba[2]]
}

fn viewport_probe_url(min_width_threshold: u32) -> String {
    let html = format!(
        "<!doctype html><title>DPR Probe</title><style>\
         html,body{{margin:0;width:100%;height:100%;background:rgb(238,32,77);}}\
         @media (min-width:{min_width_threshold}px){{html,body{{background:rgb(0,57,255);}}}}\
         </style>",
    );
    format!("data:text/html,{}", percent_encode_for_data_url(&html))
}

fn percent_encode_for_data_url(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
