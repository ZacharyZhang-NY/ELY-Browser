use ely_domain::TabId;

use crate::services::servo_sidecar::{ServoSidecarClient, SidecarSnapshotRequest};

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollOffset, WebSurfaceSize},
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

pub(super) enum WebSurfaceClient {
    Ready(ServoSidecarClient),
    Unavailable(String),
}

impl WebSurfaceClient {
    pub(super) fn new() -> Self {
        match ServoSidecarClient::new() {
            Ok(client) => Self::Ready(client),
            Err(error) => Self::Unavailable(error.to_string()),
        }
    }
}

pub(super) struct WebSurfaceRequest {
    pub(super) tab_id: TabId,
    pub(super) requested_url: String,
    pub(super) size: WebSurfaceSize,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) click_point: Option<WebSurfaceClickPoint>,
    pub(super) typed_text: Option<String>,
    pub(super) client: ServoSidecarClient,
    pub(super) snapshot_request: SidecarSnapshotRequest,
}

pub(super) enum WebSurfaceState {
    Loading {
        requested_url: String,
        size: WebSurfaceSize,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
        typed_text: Option<String>,
        previous_frame: Option<WebSurfaceFrame>,
    },
    Ready(WebSurfaceFrame),
    Failed {
        requested_url: String,
        size: WebSurfaceSize,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
        typed_text: Option<String>,
        message: String,
    },
}
