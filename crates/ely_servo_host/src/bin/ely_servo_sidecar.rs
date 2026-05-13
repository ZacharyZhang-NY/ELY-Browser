use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use ely_domain::TabId;
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, MouseDragRequest, NavigationRequest, PageZoomRequest,
    PermissionRequest, ScrollRequest, ServoHost, ServoHostError, ServoSurfaceSize,
    SoftwareServoHost, TouchTapRequest, WebViewSnapshot, WebViewState,
};
use thiserror::Error;

#[path = "ely_servo_sidecar/args.rs"]
mod args;
#[path = "ely_servo_sidecar/live.rs"]
mod live;
#[path = "ely_servo_sidecar/live_output.rs"]
mod live_output;
#[path = "ely_servo_sidecar/live_protocol.rs"]
mod live_protocol;
#[path = "ely_servo_sidecar/perf.rs"]
mod perf;
#[path = "ely_servo_sidecar/report.rs"]
mod report;

use args::{SidecarCommand, SnapshotArgs};
use report::{SnapshotInputChanges, SnapshotReport};

const WAIT_ITERATIONS: usize = 5_000;
const WAIT_INTERVAL: Duration = Duration::from_millis(2);
const RENDER_TIMEOUT: Duration = Duration::from_secs(20);
const VISIBLE_FRAME_SETTLE_TIMEOUT: Duration = Duration::from_millis(700);
const INPUT_SETTLE_TIMEOUT: Duration = Duration::from_millis(700);

fn main() -> Result<(), SidecarError> {
    match args::parse_env_command()? {
        SidecarCommand::Live(args) => live::run_live(args).map_err(SidecarError::Live),
        SidecarCommand::Snapshot(args) => run_snapshot(args),
    }
}

#[derive(Debug, Error)]
enum SidecarError {
    #[error("timed out rendering {url}: {snapshot:?}")]
    RenderTimeout { url: String, snapshot: Box<WebViewSnapshot> },

    #[error(transparent)]
    Args(#[from] args::SidecarArgsError),

    #[error(transparent)]
    Host(#[from] ServoHostError),

    #[error(transparent)]
    Live(#[from] live::LiveSidecarError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn run_snapshot(args: SnapshotArgs) -> Result<(), SidecarError> {
    std::fs::create_dir_all(&args.profile_data_dir)?;
    let mut host = SoftwareServoHost::new_with_config_dir(
        ServoSurfaceSize::new(args.width, args.height),
        Some(args.profile_data_dir.clone()),
    )?;
    let tab_id = TabId::new();
    let webview_id = host.create_webview(tab_id.clone(), args.profile_id.clone())?;
    apply_site_permissions(&mut host, &webview_id, &args)?;
    host.set_page_zoom(PageZoomRequest {
        webview_id: webview_id.clone(),
        zoom_factor: f32::from(args.page_zoom_percent) / 100.0,
    })?;

    host.navigate(NavigationRequest {
        webview_id: webview_id.clone(),
        tab_id,
        url: args.url.clone(),
    })?;

    let snapshot = wait_for_frame(&mut host, &webview_id, args.url.as_str())?;
    let (snapshot, scroll_changed_frame) =
        apply_scroll_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, click_changed_frame) =
        apply_click_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, drag_changed_frame) =
        apply_drag_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, touch_changed_frame) =
        apply_touch_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let (snapshot, text_changed_frame) =
        apply_text_if_requested(&mut host, &webview_id, &args, snapshot)?;
    let frame = host.last_rendered_frame()?;
    std::fs::write(&args.rgba_out, frame.rgba_bytes())?;

    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(
        &mut stdout,
        &SnapshotReport::new(
            &args,
            &snapshot,
            &frame,
            SnapshotInputChanges {
                scroll: scroll_changed_frame,
                click: click_changed_frame,
                drag: drag_changed_frame,
                touch: touch_changed_frame,
                text: text_changed_frame,
            },
        ),
    )?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    std::process::exit(0);
}

fn apply_site_permissions(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
) -> Result<(), SidecarError> {
    for permission in &args.site_permissions {
        host.set_permission(
            PermissionRequest {
                webview_id: webview_id.clone(),
                profile_id: args.profile_id.clone(),
                origin: permission.origin.clone(),
                feature: permission.feature,
            },
            permission.decision.into(),
        )?;
    }

    Ok(())
}

fn apply_scroll_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    if args.scroll_x == 0 && args.scroll_y == 0 {
        return Ok((snapshot, false));
    }

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.scroll(ScrollRequest {
        webview_id: webview_id.clone(),
        delta_x: args.scroll_x,
        delta_y: args.scroll_y,
        point_x: 0,
        point_y: 0,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_click_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(click_point) = args.click_point else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.click(MouseClickRequest {
        webview_id: webview_id.clone(),
        x: click_point.x,
        y: click_point.y,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_drag_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(drag_points) = args.drag_points else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.drag(MouseDragRequest {
        webview_id: webview_id.clone(),
        from_x: drag_points.from.x,
        from_y: drag_points.from.y,
        to_x: drag_points.to.x,
        to_y: drag_points.to.y,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_touch_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(touch_point) = args.touch_point else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.touch_tap(TouchTapRequest {
        webview_id: webview_id.clone(),
        x: touch_point.x,
        y: touch_point.y,
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn apply_text_if_requested(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    args: &SnapshotArgs,
    snapshot: WebViewSnapshot,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let Some(typed_text) = args.typed_text.as_ref() else {
        return Ok((snapshot, false));
    };

    let previous_frame_hash = host.last_rendered_frame()?.sample_hash();
    host.type_text(KeyboardTextRequest {
        webview_id: webview_id.clone(),
        text: typed_text.clone(),
    })?;
    wait_for_changed_or_settled_frame(host, webview_id, previous_frame_hash)
}

fn wait_for_frame(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    url: &str,
) -> Result<WebViewSnapshot, SidecarError> {
    let started_at = Instant::now();
    let mut latest_rendered_snapshot = None;
    let mut visible_frame_hash = None;
    let mut visible_frame_last_changed_at = None;

    for _ in 0..WAIT_ITERATIONS {
        if started_at.elapsed() >= RENDER_TIMEOUT {
            break;
        }

        host.tick();
        let snapshot = host.snapshot(webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(webview_id)?;
        }

        let snapshot = host.snapshot(webview_id)?;
        if let Ok(frame) = host.last_rendered_frame()
            && frame.non_white_pixel_count() > 0
            && frame.content_pixel_count() > 0
        {
            let current_hash = frame.sample_hash();
            if visible_frame_hash != Some(current_hash) {
                visible_frame_hash = Some(current_hash);
                visible_frame_last_changed_at = Some(Instant::now());
            }

            if snapshot.state() == &WebViewState::Complete {
                return Ok(snapshot);
            }

            latest_rendered_snapshot = Some(snapshot.clone());
            if snapshot.url().is_some()
                && visible_frame_last_changed_at
                    .is_some_and(|changed_at| changed_at.elapsed() >= VISIBLE_FRAME_SETTLE_TIMEOUT)
            {
                return Ok(snapshot);
            }
        }

        thread::sleep(WAIT_INTERVAL);
    }

    if let Some(snapshot) = latest_rendered_snapshot {
        return Ok(snapshot);
    }

    Err(SidecarError::RenderTimeout {
        url: url.to_string(),
        snapshot: Box::new(host.snapshot(webview_id)?),
    })
}

fn wait_for_changed_or_settled_frame(
    host: &mut SoftwareServoHost,
    webview_id: &ely_domain::WebViewId,
    previous_frame_hash: u64,
) -> Result<(WebViewSnapshot, bool), SidecarError> {
    let started_at = Instant::now();
    let mut latest_snapshot = host.snapshot(webview_id)?;

    for _ in 0..WAIT_ITERATIONS {
        if started_at.elapsed() >= INPUT_SETTLE_TIMEOUT {
            break;
        }

        host.tick();
        let snapshot = host.snapshot(webview_id)?;
        if snapshot.has_pending_frame() {
            host.paint(webview_id)?;
        }

        latest_snapshot = host.snapshot(webview_id)?;
        let changed_frame = host.last_rendered_frame().is_ok_and(|frame| {
            frame.non_white_pixel_count() > 0 && frame.sample_hash() != previous_frame_hash
        });
        if changed_frame {
            return Ok((latest_snapshot, true));
        }

        thread::sleep(WAIT_INTERVAL);
    }

    Ok((latest_snapshot, false))
}
