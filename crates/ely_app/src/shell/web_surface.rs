use std::{collections::BTreeMap, sync::Arc};

use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, App, Bounds, Context, Entity, ImageSource, IntoElement, ObjectFit, ParentElement,
    Pixels, RenderImage, Styled, StyledImage, Window, canvas, div, img, prelude::FluentBuilder, px,
    rgb,
};
use gpui_component::StyledExt;
use image::{ImageBuffer, Rgba};
use thiserror::Error;

use crate::services::servo_sidecar::{
    ServoSidecarClient, ServoSidecarError, SidecarSnapshot, SidecarSnapshotRequest,
};

use super::{ElyShell, web_surface_image::renderable_image_buffer};
use ely_design_system::{colors, spacing};

pub(super) struct WebSurfaceStore {
    client: WebSurfaceClient,
    pending_viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    states: BTreeMap<TabId, WebSurfaceState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self {
            client: WebSurfaceClient::new(),
            pending_viewport_sizes: BTreeMap::new(),
            viewport_sizes: BTreeMap::new(),
            states: BTreeMap::new(),
        }
    }

    fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.states.get(tab_id)
    }

    fn prepare_request(&mut self, tab: &BrowserTab) -> Option<WebSurfaceRequest> {
        if !is_external_web_url(tab.url().as_str()) {
            return None;
        }

        let size = self.viewport_sizes.get(tab.id()).copied()?;
        let requested_url = tab.url().as_str().to_string();
        if self.is_loading_requested_url(tab.id(), requested_url.as_str()) {
            return None;
        }
        if self.has_current_state(tab.id(), &requested_url, size) {
            return None;
        }

        let client = match &self.client {
            WebSurfaceClient::Ready(client) => client.clone(),
            WebSurfaceClient::Unavailable(message) => {
                self.states.insert(
                    tab.id().clone(),
                    WebSurfaceState::Failed { requested_url, size, message: message.clone() },
                );
                return None;
            }
        };

        self.states.insert(
            tab.id().clone(),
            WebSurfaceState::Loading {
                requested_url: requested_url.clone(),
                size,
                previous_frame: self.previous_ready_frame(tab.id(), requested_url.as_str()),
            },
        );

        Some(WebSurfaceRequest {
            tab_id: tab.id().clone(),
            requested_url,
            size,
            client,
            snapshot_request: SidecarSnapshotRequest::new(
                tab.url().clone(),
                size.width,
                size.height,
            ),
        })
    }

    fn has_current_state(&self, tab_id: &TabId, requested_url: &str, size: WebSurfaceSize) -> bool {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                ..
            }) => current_url == requested_url && *current_size == size,
            Some(WebSurfaceState::Ready(frame)) => {
                frame.requested_url == requested_url && frame.size() == size
            }
            Some(WebSurfaceState::Failed {
                requested_url: current_url,
                size: current_size,
                ..
            }) => current_url == requested_url && *current_size == size,
            None => false,
        }
    }

    fn is_loading(&self, tab_id: &TabId, requested_url: &str, size: WebSurfaceSize) -> bool {
        matches!(
            self.states.get(tab_id),
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                ..
            })
                if current_url == requested_url && *current_size == size
        )
    }

    fn is_loading_requested_url(&self, tab_id: &TabId, requested_url: &str) -> bool {
        matches!(
            self.states.get(tab_id),
            Some(WebSurfaceState::Loading { requested_url: current_url, .. })
                if current_url == requested_url
        )
    }

    fn previous_ready_frame(&self, tab_id: &TabId, requested_url: &str) -> Option<WebSurfaceFrame> {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Ready(frame)) if frame.requested_url == requested_url => {
                Some(frame.clone())
            }
            Some(WebSurfaceState::Loading {
                requested_url: current_url, previous_frame, ..
            }) if current_url == requested_url => previous_frame.clone(),
            _ => None,
        }
    }

    fn finish(&mut self, tab_id: TabId, state: WebSurfaceState) {
        self.states.insert(tab_id, state);
    }

    fn record_viewport_size(&mut self, tab_id: &TabId, bounds: Bounds<Pixels>) -> bool {
        let Some(size) = WebSurfaceSize::from_bounds(bounds) else {
            return false;
        };

        let Some(current_size) = self.viewport_sizes.get(tab_id).copied() else {
            self.viewport_sizes.insert(tab_id.clone(), size);
            self.pending_viewport_sizes.remove(tab_id);
            return true;
        };

        if current_size == size {
            self.pending_viewport_sizes.remove(tab_id);
            return false;
        }

        if self.pending_viewport_sizes.get(tab_id) != Some(&size) {
            self.pending_viewport_sizes.insert(tab_id.clone(), size);
            return false;
        }

        self.pending_viewport_sizes.remove(tab_id);
        self.viewport_sizes.insert(tab_id.clone(), size);
        true
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
    size: WebSurfaceSize,
    client: ServoSidecarClient,
    snapshot_request: SidecarSnapshotRequest,
}

enum WebSurfaceState {
    Loading { requested_url: String, size: WebSurfaceSize, previous_frame: Option<WebSurfaceFrame> },
    Ready(WebSurfaceFrame),
    Failed { requested_url: String, size: WebSurfaceSize, message: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WebSurfaceSize {
    width: u32,
    height: u32,
}

impl WebSurfaceSize {
    fn from_bounds(bounds: Bounds<Pixels>) -> Option<Self> {
        Some(Self {
            width: viewport_dimension(bounds.size.width)?,
            height: viewport_dimension(bounds.size.height)?,
        })
    }
}

#[derive(Clone)]
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

        let image_buffer = renderable_image_buffer(buffer);

        Ok(Self {
            requested_url,
            loaded_url,
            title,
            width,
            height,
            image: Arc::new(RenderImage::new([image::Frame::new(image_buffer)])),
        })
    }

    fn title_label(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.requested_url.clone())
    }

    fn url_label(&self) -> &str {
        self.loaded_url.as_deref().unwrap_or(self.requested_url.as_str())
    }

    fn size(&self) -> WebSurfaceSize {
        WebSurfaceSize { width: self.width, height: self.height }
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

        let state_entity = cx.entity().clone();
        match self.web_surfaces.state(tab.id()) {
            Some(WebSurfaceState::Ready(frame)) => {
                render_ready_web_surface(frame, tab, state_entity)
            }
            Some(WebSurfaceState::Failed { message, .. }) => {
                render_failed_web_surface(tab, message.as_str(), state_entity)
            }
            Some(WebSurfaceState::Loading { previous_frame: Some(frame), .. }) => {
                render_ready_web_surface(frame, tab, state_entity)
            }
            Some(WebSurfaceState::Loading { previous_frame: None, .. }) | None => {
                render_loading_web_surface(tab, state_entity)
            }
        }
    }

    fn ensure_external_web_frame(&mut self, tab: &BrowserTab, cx: &mut Context<Self>) {
        let Some(request) = self.web_surfaces.prepare_request(tab) else {
            return;
        };

        let WebSurfaceRequest { tab_id, requested_url, size, client, snapshot_request } = request;
        cx.spawn(async move |shell, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.snapshot(snapshot_request) })
                .await;

            _ = shell.update(cx, |shell, cx| {
                shell.handle_external_web_frame_result(tab_id, requested_url, size, result);
                cx.notify();
            });
        })
        .detach();
    }

    fn handle_external_web_frame_result(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        size: WebSurfaceSize,
        result: Result<SidecarSnapshot, ServoSidecarError>,
    ) {
        if !self.web_surfaces.is_loading(&tab_id, requested_url.as_str(), size) {
            return;
        }

        let state = match result {
            Ok(snapshot) => match WebSurfaceFrame::from_snapshot(requested_url.clone(), snapshot) {
                Ok(frame) => WebSurfaceState::Ready(frame),
                Err(error) => {
                    WebSurfaceState::Failed { requested_url, size, message: error.to_string() }
                }
            },
            Err(error) => {
                WebSurfaceState::Failed { requested_url, size, message: error.to_string() }
            }
        };
        self.web_surfaces.finish(tab_id, state);
    }

    fn record_external_web_viewport(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_viewport_size(&tab_id, bounds) {
            cx.notify();
        }
    }
}

fn render_ready_web_surface(
    frame: &WebSurfaceFrame,
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        frame.title_label(),
        frame.url_label().to_string(),
        Some(format!("{}x{}", frame.width, frame.height)),
        img(ImageSource::Render(frame.image.clone())).size_full().object_fit(ObjectFit::Contain),
    )
}

fn render_web_surface_header(title: String, url: String, detail: Option<String>) -> AnyElement {
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
                .child(title),
        )
        .when_some(detail, |this, detail| {
            this.child(div().text_xs().text_color(rgb(colors::MUTED)).child(detail))
        })
        .child(
            div().max_w(px(420.0)).truncate().text_xs().text_color(rgb(colors::MUTED)).child(url),
        )
        .into_any_element()
}

fn render_loading_web_surface(tab: &BrowserTab, state_entity: Entity<ElyShell>) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        tab.title().to_string(),
        tab.url().as_str().to_string(),
        Some("Rendering".to_string()),
        centered_status(tab.title(), tab.url().as_str(), "Rendering page with Servo", colors::BODY),
    )
}

fn render_failed_web_surface(
    tab: &BrowserTab,
    message: &str,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        tab.title().to_string(),
        tab.url().as_str().to_string(),
        Some("Render failed".to_string()),
        centered_status(tab.title(), tab.url().as_str(), message, colors::ERROR),
    )
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

fn render_web_surface(
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
    title: String,
    url: String,
    detail: Option<String>,
    content: impl IntoElement,
) -> AnyElement {
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
                .flex()
                .flex_col()
                .child(render_web_surface_header(title, url, detail))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .bg(rgb(colors::SURFACE_CARD))
                        .child(content)
                        .child(render_viewport_tracker(tab.id().clone(), state_entity)),
                ),
        )
        .into_any_element()
}

fn render_viewport_tracker(tab_id: TabId, state_entity: Entity<ElyShell>) -> impl IntoElement {
    canvas(
        move |bounds, _window: &mut Window, cx: &mut App| {
            state_entity.update(cx, |shell, cx| {
                shell.record_external_web_viewport(tab_id, bounds, cx);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

fn viewport_dimension(pixels: Pixels) -> Option<u32> {
    let value = f32::from(pixels.round());
    if !value.is_finite() || value < 1.0 || value > u32::MAX as f32 {
        return None;
    }

    Some(value as u32)
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
