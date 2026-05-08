use std::collections::BTreeMap;

use ely_domain::{BrowserTab, TabId};
use gpui::{AnyElement, Bounds, Context, Pixels, Point};

use crate::services::servo_sidecar::{ServoSidecarError, SidecarSnapshot, SidecarSnapshotRequest};

use super::{
    ElyShell,
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{
        WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceScrollOffset, WebSurfaceSize,
    },
    web_surface_state::{
        WebSurfaceClickState, WebSurfaceClient, WebSurfaceRequest, WebSurfaceScrollState,
        WebSurfaceState,
    },
    web_surface_view::{
        render_failed_web_surface, render_loading_web_surface, render_ready_web_surface,
    },
};

pub(super) struct WebSurfaceStore {
    client: WebSurfaceClient,
    pending_viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    click_points: BTreeMap<TabId, WebSurfaceClickState>,
    scroll_offsets: BTreeMap<TabId, WebSurfaceScrollState>,
    viewport_bounds: BTreeMap<TabId, Bounds<Pixels>>,
    viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    states: BTreeMap<TabId, WebSurfaceState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self {
            client: WebSurfaceClient::new(),
            pending_viewport_sizes: BTreeMap::new(),
            click_points: BTreeMap::new(),
            scroll_offsets: BTreeMap::new(),
            viewport_bounds: BTreeMap::new(),
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
        let scroll_offset = self.scroll_offset_for(tab.id(), requested_url.as_str());
        let click_point = self.click_point_for(tab.id(), requested_url.as_str(), scroll_offset);
        if self.is_loading_requested_url(tab.id(), requested_url.as_str()) {
            return None;
        }
        if self.has_current_state(tab.id(), &requested_url, size, scroll_offset, click_point) {
            return None;
        }

        let client = match &self.client {
            WebSurfaceClient::Ready(client) => client.clone(),
            WebSurfaceClient::Unavailable(message) => {
                self.states.insert(
                    tab.id().clone(),
                    WebSurfaceState::Failed {
                        requested_url,
                        size,
                        scroll_offset,
                        click_point,
                        message: message.clone(),
                    },
                );
                return None;
            }
        };

        self.states.insert(
            tab.id().clone(),
            WebSurfaceState::Loading {
                requested_url: requested_url.clone(),
                size,
                scroll_offset,
                click_point,
                previous_frame: self.previous_ready_frame(tab.id(), requested_url.as_str()),
            },
        );

        let mut snapshot_request =
            SidecarSnapshotRequest::new(tab.url().clone(), size.width, size.height)
                .with_scroll_offset(scroll_offset.x(), scroll_offset.y());
        if let Some(click_point) = click_point {
            snapshot_request = snapshot_request.with_click_point(click_point.x(), click_point.y());
        }

        Some(WebSurfaceRequest {
            tab_id: tab.id().clone(),
            requested_url,
            size,
            scroll_offset,
            click_point,
            client,
            snapshot_request,
        })
    }

    fn has_current_state(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        size: WebSurfaceSize,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
    ) -> bool {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                ..
            }) => {
                current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
            }
            Some(WebSurfaceState::Ready(frame)) => {
                frame.requested_url == requested_url
                    && frame.size() == size
                    && frame.scroll_offset() == scroll_offset
                    && frame.click_point() == click_point
            }
            Some(WebSurfaceState::Failed {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                ..
            }) => {
                current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
            }
            None => false,
        }
    }

    fn is_loading(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        size: WebSurfaceSize,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
    ) -> bool {
        matches!(
            self.states.get(tab_id),
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                ..
            })
                if current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
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

    fn record_scroll_delta(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        delta: gpui::Point<Pixels>,
    ) -> bool {
        let Some(delta) = WebSurfaceScrollDelta::from_point(delta) else {
            return false;
        };

        let state = self
            .scroll_offsets
            .entry(tab_id.clone())
            .or_insert_with(|| WebSurfaceScrollState::new(requested_url.to_string()));
        if state.requested_url != requested_url {
            *state = WebSurfaceScrollState::new(requested_url.to_string());
        }

        let next_offset = state.offset.scrolled_by(delta);
        if next_offset == state.offset {
            return false;
        }

        state.offset = next_offset;
        self.click_points.remove(tab_id);
        true
    }

    fn record_viewport_size(&mut self, tab_id: &TabId, bounds: Bounds<Pixels>) -> bool {
        let Some(size) = WebSurfaceSize::from_bounds(bounds) else {
            return false;
        };
        self.viewport_bounds.insert(tab_id.clone(), bounds);

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

    fn record_click_point(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        position: Point<Pixels>,
    ) -> bool {
        let Some(bounds) = self.viewport_bounds.get(tab_id).copied() else {
            return false;
        };
        let Some(point) = WebSurfaceClickPoint::from_window_position(bounds, position) else {
            return false;
        };

        let state = WebSurfaceClickState {
            requested_url: requested_url.to_string(),
            scroll_offset: self.scroll_offset_for(tab_id, requested_url),
            point,
        };
        if self.click_points.get(tab_id) == Some(&state) {
            return false;
        }

        self.click_points.insert(tab_id.clone(), state);
        true
    }

    fn scroll_offset_for(&self, tab_id: &TabId, requested_url: &str) -> WebSurfaceScrollOffset {
        self.scroll_offsets
            .get(tab_id)
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.offset)
            .unwrap_or_default()
    }

    fn click_point_for(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        scroll_offset: WebSurfaceScrollOffset,
    ) -> Option<WebSurfaceClickPoint> {
        self.click_points
            .get(tab_id)
            .filter(|state| {
                state.requested_url == requested_url && state.scroll_offset == scroll_offset
            })
            .map(|state| state.point)
    }
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

        let WebSurfaceRequest {
            tab_id,
            requested_url,
            size,
            scroll_offset,
            click_point,
            client,
            snapshot_request,
        } = request;
        cx.spawn(async move |shell, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.snapshot(snapshot_request) })
                .await;

            _ = shell.update(cx, |shell, cx| {
                shell.handle_external_web_frame_result(
                    tab_id,
                    requested_url,
                    size,
                    scroll_offset,
                    click_point,
                    result,
                );
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
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
        result: Result<SidecarSnapshot, ServoSidecarError>,
    ) {
        if !self.web_surfaces.is_loading(
            &tab_id,
            requested_url.as_str(),
            size,
            scroll_offset,
            click_point,
        ) {
            return;
        }

        let state = match result {
            Ok(snapshot) => match WebSurfaceFrame::from_snapshot(
                requested_url.clone(),
                scroll_offset,
                click_point,
                snapshot,
            ) {
                Ok(frame) => WebSurfaceState::Ready(frame),
                Err(error) => WebSurfaceState::Failed {
                    requested_url,
                    size,
                    scroll_offset,
                    click_point,
                    message: error.to_string(),
                },
            },
            Err(error) => WebSurfaceState::Failed {
                requested_url,
                size,
                scroll_offset,
                click_point,
                message: error.to_string(),
            },
        };
        self.web_surfaces.finish(tab_id, state);
    }

    pub(super) fn record_external_web_viewport(
        &mut self,
        tab_id: TabId,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_viewport_size(&tab_id, bounds) {
            cx.notify();
        }
    }

    pub(super) fn scroll_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        delta: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_scroll_delta(&tab_id, requested_url.as_str(), delta) {
            cx.notify();
        }
    }

    pub(super) fn click_external_web_viewport(
        &mut self,
        tab_id: TabId,
        requested_url: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.web_surfaces.record_click_point(&tab_id, requested_url.as_str(), position) {
            cx.notify();
        }
    }
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
