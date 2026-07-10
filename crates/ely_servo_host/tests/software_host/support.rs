use std::{error::Error, thread, time::Duration};

use ely_servo_host::{RenderedFrame, ServoHost, SoftwareServoHost, WebViewSnapshot, WebViewState};

use super::{INITIAL_HEIGHT, INITIAL_WIDTH};

pub(super) fn wait_for_rendered_webview(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: Option<u64>,
) -> Result<WebViewSnapshot, Box<dyn Error>> {
    wait_for_rendered_webview_matching(host, webview_id, previous_frame_hash, |_| true, |_| true)
}

pub(super) fn wait_for_rendered_webview_with_title(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: Option<u64>,
    expected_title: &str,
) -> Result<WebViewSnapshot, Box<dyn Error>> {
    wait_for_rendered_webview_matching(
        host,
        webview_id,
        previous_frame_hash,
        |snapshot| snapshot.title() == Some(expected_title),
        |_| true,
    )
}

pub(super) fn wait_for_rendered_webview_with_center_pixel(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: Option<u64>,
    expected_rgb: [u8; 3],
) -> Result<WebViewSnapshot, Box<dyn Error>> {
    wait_for_rendered_webview_matching(
        host,
        webview_id,
        previous_frame_hash,
        |_| true,
        |frame| center_pixel_rgb(frame) == expected_rgb,
    )
}

fn wait_for_rendered_webview_matching(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: Option<u64>,
    snapshot_matches: impl Fn(&WebViewSnapshot) -> bool,
    frame_matches: impl Fn(&RenderedFrame) -> bool,
) -> Result<WebViewSnapshot, Box<dyn Error>> {
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
                && frame_matches(&frame)
        });

        if snapshot.state() == &WebViewState::Complete
            && has_rendered_current_request
            && snapshot_matches(&snapshot)
        {
            return Ok(snapshot);
        }

        thread::sleep(Duration::from_millis(2));
    }

    Err(format!("timed out waiting for rendered webview: {:?}", host.snapshot(webview_id)?).into())
}

pub(super) fn assert_rendered_frame_has_content(
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

pub(super) fn assert_rendered_frame_has_dimensions_and_content(
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
    frame: &RenderedFrame,
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

pub(super) fn center_pixel_rgb(frame: &RenderedFrame) -> [u8; 3] {
    let x = frame.width() / 2;
    let y = frame.height() / 2;
    let index = ((y * frame.width() + x) * 4) as usize;
    let rgba = &frame.rgba_bytes()[index..index + 4];
    [rgba[0], rgba[1], rgba[2]]
}

pub(super) fn viewport_probe_url(min_width_threshold: u32) -> String {
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
