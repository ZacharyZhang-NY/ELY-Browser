use ely_domain::TabId;

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollOffset},
};

pub(super) struct WebSurfaceScrollState {
    pub(super) requested_url: String,
    pub(super) offset: WebSurfaceScrollOffset,
}

impl WebSurfaceScrollState {
    pub(super) fn new(requested_url: String) -> Self {
        Self { requested_url, offset: WebSurfaceScrollOffset::default() }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceClickState {
    pub(super) requested_url: String,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) point: WebSurfaceClickPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceKeyboardFocusState {
    pub(super) tab_id: TabId,
    pub(super) requested_url: String,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) click_point: WebSurfaceClickPoint,
}

pub(super) struct WebSurfaceTextInputState {
    pub(super) requested_url: String,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) click_point: WebSurfaceClickPoint,
    pub(super) text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfacePendingInput {
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) scroll_delta: Option<super::web_surface_geometry::WebSurfaceScrollDelta>,
    pub(super) click_point: Option<WebSurfaceClickPoint>,
    pub(super) hover_point: Option<WebSurfaceClickPoint>,
    pub(super) typed_text: Option<String>,
}

pub(super) enum WebSurfaceState {
    Loading { requested_url: String, previous_frame: Option<WebSurfaceFrame> },
    Ready(WebSurfaceFrame),
    Failed { message: String },
}
