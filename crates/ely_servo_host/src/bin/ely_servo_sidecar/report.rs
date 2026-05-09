use ely_servo_host::{RenderedFrame, WebViewSnapshot, WebViewState};
use serde::Serialize;

use super::args::SnapshotArgs;

pub(super) struct SnapshotInputChanges {
    pub(super) scroll: bool,
    pub(super) click: bool,
    pub(super) drag: bool,
    pub(super) touch: bool,
    pub(super) text: bool,
}

#[derive(Serialize)]
pub(super) struct SnapshotReport {
    requested_url: String,
    profile_id: String,
    loaded_url: Option<String>,
    title: Option<String>,
    rgba_path: String,
    state: &'static str,
    width: u32,
    height: u32,
    rgba_byte_count: usize,
    opaque_pixel_count: u64,
    non_white_pixel_count: u64,
    content_pixel_count: u64,
    sample_hash: u64,
    scroll_x: i32,
    scroll_y: i32,
    scroll_changed_frame: bool,
    click_x: Option<u32>,
    click_y: Option<u32>,
    click_changed_frame: bool,
    drag_from_x: Option<u32>,
    drag_from_y: Option<u32>,
    drag_to_x: Option<u32>,
    drag_to_y: Option<u32>,
    drag_changed_frame: bool,
    touch_x: Option<u32>,
    touch_y: Option<u32>,
    touch_changed_frame: bool,
    typed_text_byte_count: usize,
    text_changed_frame: bool,
}

impl SnapshotReport {
    pub(super) fn new(
        args: &SnapshotArgs,
        snapshot: &WebViewSnapshot,
        frame: &RenderedFrame,
        changes: SnapshotInputChanges,
    ) -> Self {
        Self {
            requested_url: args.url.as_str().to_string(),
            profile_id: snapshot.profile_id().as_str().to_string(),
            loaded_url: snapshot.url().map(str::to_string),
            title: snapshot.title().map(str::to_string),
            rgba_path: args.rgba_out.display().to_string(),
            state: state_label(snapshot.state()),
            width: frame.width(),
            height: frame.height(),
            rgba_byte_count: frame.rgba_bytes().len(),
            opaque_pixel_count: frame.opaque_pixel_count(),
            non_white_pixel_count: frame.non_white_pixel_count(),
            content_pixel_count: frame.content_pixel_count(),
            sample_hash: frame.sample_hash(),
            scroll_x: args.scroll_x,
            scroll_y: args.scroll_y,
            scroll_changed_frame: changes.scroll,
            click_x: args.click_point.map(|point| point.x),
            click_y: args.click_point.map(|point| point.y),
            click_changed_frame: changes.click,
            drag_from_x: args.drag_points.map(|points| points.from.x),
            drag_from_y: args.drag_points.map(|points| points.from.y),
            drag_to_x: args.drag_points.map(|points| points.to.x),
            drag_to_y: args.drag_points.map(|points| points.to.y),
            drag_changed_frame: changes.drag,
            touch_x: args.touch_point.map(|point| point.x),
            touch_y: args.touch_point.map(|point| point.y),
            touch_changed_frame: changes.touch,
            typed_text_byte_count: args.typed_text.as_ref().map_or(0, String::len),
            text_changed_frame: changes.text,
        }
    }
}

fn state_label(state: &WebViewState) -> &'static str {
    match state {
        WebViewState::Created => "created",
        WebViewState::Loading => "loading",
        WebViewState::Complete => "complete",
        WebViewState::Sleeping => "sleeping",
        WebViewState::Crashed => "crashed",
    }
}
