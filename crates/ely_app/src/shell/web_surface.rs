use std::{collections::BTreeMap, sync::Arc};

use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, Context, ImageSource, IntoElement, ObjectFit, ParentElement, RenderImage, Styled,
    StyledImage, div, img, px, rgb,
};
use gpui_component::StyledExt;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_sidecar::{
    ServoSidecarClient, ServoSidecarError, SidecarSnapshot, SidecarSnapshotRequest,
};

use super::ElyShell;
use ely_design_system::{colors, spacing};

const WEB_SURFACE_WIDTH: u32 = 1024;
const WEB_SURFACE_HEIGHT: u32 = 768;

pub(super) struct WebSurfaceStore {
    client: WebSurfaceClient,
    states: BTreeMap<TabId, WebSurfaceState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self { client: WebSurfaceClient::new(), states: BTreeMap::new() }
    }

    fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.states.get(tab_id)
    }

    fn prepare_request(&mut self, tab: &BrowserTab) -> Option<WebSurfaceRequest> {
        if !is_external_web_url(tab.url().as_str()) {
            return None;
        }

        let requested_url = tab.url().as_str().to_string();
        if self.has_current_state(tab.id(), &requested_url) {
            return None;
        }

        let client = match &self.client {
            WebSurfaceClient::Ready(client) => client.clone(),
            WebSurfaceClient::Unavailable(message) => {
                self.states.insert(
                    tab.id().clone(),
                    WebSurfaceState::Failed { requested_url, message: message.clone() },
                );
                return None;
            }
        };

        self.states.insert(
            tab.id().clone(),
            WebSurfaceState::Loading { requested_url: requested_url.clone() },
        );

        Some(WebSurfaceRequest {
            tab_id: tab.id().clone(),
            requested_url,
            client,
            snapshot_request: SidecarSnapshotRequest::new(
                tab.url().clone(),
                WEB_SURFACE_WIDTH,
                WEB_SURFACE_HEIGHT,
            ),
        })
    }

    fn has_current_state(&self, tab_id: &TabId, requested_url: &str) -> bool {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Loading { requested_url: current_url }) => {
                current_url == requested_url
            }
            Some(WebSurfaceState::Ready(frame)) => frame.requested_url == requested_url,
            Some(WebSurfaceState::Failed { requested_url: current_url, .. }) => {
                current_url == requested_url
            }
            None => false,
        }
    }

    fn is_loading(&self, tab_id: &TabId, requested_url: &str) -> bool {
        matches!(
            self.states.get(tab_id),
            Some(WebSurfaceState::Loading { requested_url: current_url })
                if current_url == requested_url
        )
    }

    fn finish(&mut self, tab_id: TabId, state: WebSurfaceState) {
        self.states.insert(tab_id, state);
    }
}

enum WebSurfaceClient {
    Ready(ServoSidecarClient),
    Unavailable(String),
}

impl WebSurfaceClient {
    fn new() -> Self {
        match ServoSidecarClient::new() {
            Ok(client) => Self::Ready(client),
            Err(error) => Self::Unavailable(error.to_string()),
        }
    }
}

struct WebSurfaceRequest {
    tab_id: TabId,
    requested_url: String,
    client: ServoSidecarClient,
    snapshot_request: SidecarSnapshotRequest,
}

enum WebSurfaceState {
    Loading { requested_url: String },
    Ready(WebSurfaceFrame),
    Failed { requested_url: String, message: String },
}

struct WebSurfaceFrame {
    requested_url: String,
    loaded_url: Option<String>,
    title: Option<String>,
    width: u32,
    height: u32,
    image: Arc<RenderImage>,
}

impl WebSurfaceFrame {
    fn from_snapshot(
        requested_url: String,
        snapshot: SidecarSnapshot,
    ) -> Result<Self, WebSurfaceError> {
        let width = snapshot.width();
        let height = snapshot.height();
        let loaded_url = snapshot.loaded_url().map(str::to_string);
        let title = snapshot.title().map(str::to_string);
        let rgba_bytes = snapshot.into_rgba_bytes();

        let Some(buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_bytes) else {
            return Err(WebSurfaceError::InvalidFrameBuffer { width, height });
        };

        Ok(Self {
            requested_url,
            loaded_url,
            title,
            width,
            height,
            image: Arc::new(RenderImage::new([image::Frame::new(buffer)])),
        })
    }

    fn title_label(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.requested_url.clone())
    }

    fn url_label(&self) -> &str {
        self.loaded_url.as_deref().unwrap_or(self.requested_url.as_str())
    }
}

#[derive(Debug, Error)]
enum WebSurfaceError {
    #[error("invalid servo frame buffer for {width}x{height}")]
    InvalidFrameBuffer { width: u32, height: u32 },
}

impl ElyShell {
    pub(super) fn render_external_web_canvas(
        &mut self,
        tab: &BrowserTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.ensure_external_web_frame(tab, cx);

        match self.web_surfaces.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame)) => render_ready_web_surface(frame),
            Some(WebSurfaceState::Failed { message, .. }) => {
                render_failed_web_surface(tab, message.as_str())
            }
            Some(WebSurfaceState::Loading { .. }) | None => render_loading_web_surface(tab),
        }
    }

    fn ensure_external_web_frame(&mut self, tab: &BrowserTab, cx: &mut Context<Self>) {
        let Some(request) = self.web_surfaces.prepare_request(tab) else {
            return;
        };

        let WebSurfaceRequest { tab_id, requested_url, client, snapshot_request } = request;
        cx.spawn(async move |shell, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.snapshot(snapshot_request) })
                .await;

            _ = shell.update(cx, |shell, cx| {
                shell.handle_external_web_frame_result(tab_id, requested_url, result);
                cx.notify();
            });
        })
        .detach();
    }

    fn handle_external_web_frame_result(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        result: Result<SidecarSnapshot, ServoSidecarError>,
    ) {
        if !self.web_surfaces.is_loading(&tab_id, requested_url.as_str()) {
            return;
        }

        let state = match result {
            Ok(snapshot) => match WebSurfaceFrame::from_snapshot(requested_url.clone(), snapshot) {
                Ok(frame) => WebSurfaceState::Ready(frame),
                Err(error) => WebSurfaceState::Failed { requested_url, message: error.to_string() },
            },
            Err(error) => WebSurfaceState::Failed { requested_url, message: error.to_string() },
        };
        self.web_surfaces.finish(tab_id, state);
    }
}

fn render_ready_web_surface(frame: &WebSurfaceFrame) -> AnyElement {
    render_web_surface(
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(colors::SURFACE_CARD))
            .child(render_web_surface_header(frame))
            .child(
                div().flex_1().min_h_0().overflow_hidden().bg(rgb(colors::SURFACE_CARD)).child(
                    img(ImageSource::Render(frame.image.clone()))
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
            ),
    )
}

fn render_web_surface_header(frame: &WebSurfaceFrame) -> AnyElement {
    div()
        .h(px(34.0))
        .px_3()
        .gap_3()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child(frame.title_label()),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(format!("{}x{}", frame.width, frame.height)),
        )
        .child(
            div()
                .max_w(px(420.0))
                .truncate()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(frame.url_label().to_string()),
        )
        .into_any_element()
}

fn render_loading_web_surface(tab: &BrowserTab) -> AnyElement {
    render_web_surface(centered_status(
        tab.title(),
        tab.url().as_str(),
        "Rendering page with Servo",
        colors::BODY,
    ))
}

fn render_failed_web_surface(tab: &BrowserTab, message: &str) -> AnyElement {
    render_web_surface(centered_status(tab.title(), tab.url().as_str(), message, colors::ERROR))
}

fn centered_status(title: &str, url: &str, detail: &str, detail_color: u32) -> impl IntoElement {
    div()
        .size_full()
        .p_8()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(rgb(colors::SURFACE_CARD))
        .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child(title.to_string()))
        .child(div().text_sm().text_color(rgb(colors::MUTED)).child(url.to_string()))
        .child(div().text_sm().text_color(rgb(detail_color)).child(detail.to_string()))
}

fn render_web_surface(content: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .p(px(spacing::SM))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .size_full()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .bg(rgb(colors::SURFACE_CARD))
                .child(content),
        )
        .into_any_element()
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
