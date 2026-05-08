use std::collections::BTreeMap;

use ely_domain::{BrowserTab, TabId};
use gpui::{Bounds, Pixels, Point};

use crate::services::servo_sidecar::SidecarSnapshotRequest;

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{
        WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceScrollOffset, WebSurfaceSize,
    },
    web_surface_state::{
        WebSurfaceClickState, WebSurfaceClient, WebSurfaceKeyboardFocusState, WebSurfaceRequest,
        WebSurfaceScrollState, WebSurfaceState, WebSurfaceTextInputState,
    },
};

pub(super) struct WebSurfaceStore {
    client: WebSurfaceClient,
    pending_viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    click_points: BTreeMap<TabId, WebSurfaceClickState>,
    keyboard_focus: Option<WebSurfaceKeyboardFocusState>,
    scroll_offsets: BTreeMap<TabId, WebSurfaceScrollState>,
    typed_texts: BTreeMap<TabId, WebSurfaceTextInputState>,
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
            keyboard_focus: None,
            scroll_offsets: BTreeMap::new(),
            typed_texts: BTreeMap::new(),
            viewport_bounds: BTreeMap::new(),
            viewport_sizes: BTreeMap::new(),
            states: BTreeMap::new(),
        }
    }

    pub(super) fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.states.get(tab_id)
    }

    pub(super) fn prepare_request(&mut self, tab: &BrowserTab) -> Option<WebSurfaceRequest> {
        if !is_external_web_url(tab.url().as_str()) {
            return None;
        }

        let size = self.viewport_sizes.get(tab.id()).copied()?;
        let requested_url = tab.url().as_str().to_string();
        let scroll_offset = self.scroll_offset_for(tab.id(), requested_url.as_str());
        let click_point = self.click_point_for(tab.id(), requested_url.as_str(), scroll_offset);
        let typed_text =
            self.typed_text_for(tab.id(), requested_url.as_str(), scroll_offset, click_point);
        if self.is_loading_requested_url(tab.id(), requested_url.as_str()) {
            return None;
        }
        if self.has_current_state(
            tab.id(),
            &requested_url,
            size,
            scroll_offset,
            click_point,
            typed_text.as_deref(),
        ) {
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
                        typed_text: typed_text.clone(),
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
                typed_text: typed_text.clone(),
                previous_frame: self.previous_ready_frame(tab.id(), requested_url.as_str()),
            },
        );

        let mut snapshot_request =
            SidecarSnapshotRequest::new(tab.url().clone(), size.width, size.height)
                .with_scroll_offset(scroll_offset.x(), scroll_offset.y());
        if let Some(click_point) = click_point {
            snapshot_request = snapshot_request.with_click_point(click_point.x(), click_point.y());
        }
        if let Some(typed_text) = typed_text.clone() {
            snapshot_request = snapshot_request.with_typed_text(typed_text);
        }

        Some(WebSurfaceRequest {
            tab_id: tab.id().clone(),
            requested_url,
            size,
            scroll_offset,
            click_point,
            typed_text,
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
        typed_text: Option<&str>,
    ) -> bool {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                typed_text: current_typed_text,
                ..
            }) => {
                current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
                    && current_typed_text.as_deref() == typed_text
            }
            Some(WebSurfaceState::Ready(frame)) => {
                frame.requested_url == requested_url
                    && frame.size() == size
                    && frame.scroll_offset() == scroll_offset
                    && frame.click_point() == click_point
                    && frame.typed_text() == typed_text
            }
            Some(WebSurfaceState::Failed {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                typed_text: current_typed_text,
                ..
            }) => {
                current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
                    && current_typed_text.as_deref() == typed_text
            }
            None => false,
        }
    }

    pub(super) fn is_loading(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        size: WebSurfaceSize,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
        typed_text: Option<&str>,
    ) -> bool {
        matches!(
            self.states.get(tab_id),
            Some(WebSurfaceState::Loading {
                requested_url: current_url,
                size: current_size,
                scroll_offset: current_scroll_offset,
                click_point: current_click_point,
                typed_text: current_typed_text,
                ..
            })
                if current_url == requested_url
                    && *current_size == size
                    && *current_scroll_offset == scroll_offset
                    && *current_click_point == click_point
                    && current_typed_text.as_deref() == typed_text
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

    pub(super) fn finish(&mut self, tab_id: TabId, state: WebSurfaceState) {
        self.states.insert(tab_id, state);
    }

    pub(super) fn record_scroll_delta(
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
        self.typed_texts.remove(tab_id);
        self.keyboard_focus = None;
        true
    }

    pub(super) fn record_viewport_size(&mut self, tab_id: &TabId, bounds: Bounds<Pixels>) -> bool {
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

    pub(super) fn record_click_point(
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
        self.keyboard_focus = Some(WebSurfaceKeyboardFocusState {
            tab_id: tab_id.clone(),
            requested_url: requested_url.to_string(),
            scroll_offset: state.scroll_offset,
            click_point: state.point,
        });
        if self.click_points.get(tab_id) == Some(&state) {
            return false;
        }

        self.typed_texts.remove(tab_id);
        self.click_points.insert(tab_id.clone(), state);
        true
    }

    pub(super) fn record_typed_text(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        text: &str,
    ) -> bool {
        if text.is_empty() {
            return false;
        }
        let Some(focus) = self.keyboard_focus.as_ref() else {
            return false;
        };
        if focus.tab_id != *tab_id || focus.requested_url != requested_url {
            return false;
        }

        let entry =
            self.typed_texts.entry(tab_id.clone()).or_insert_with(|| WebSurfaceTextInputState {
                requested_url: requested_url.to_string(),
                scroll_offset: focus.scroll_offset,
                click_point: focus.click_point,
                text: String::new(),
            });
        if entry.requested_url != requested_url
            || entry.scroll_offset != focus.scroll_offset
            || entry.click_point != focus.click_point
        {
            *entry = WebSurfaceTextInputState {
                requested_url: requested_url.to_string(),
                scroll_offset: focus.scroll_offset,
                click_point: focus.click_point,
                text: String::new(),
            };
        }

        entry.text.push_str(text);
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

    fn typed_text_for(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        scroll_offset: WebSurfaceScrollOffset,
        click_point: Option<WebSurfaceClickPoint>,
    ) -> Option<String> {
        let click_point = click_point?;
        self.typed_texts
            .get(tab_id)
            .filter(|state| {
                state.requested_url == requested_url
                    && state.scroll_offset == scroll_offset
                    && state.click_point == click_point
                    && !state.text.is_empty()
            })
            .map(|state| state.text.clone())
    }
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ely_domain::{BrowserTab, ProfileId, SpaceId, TabId, UrlText};
    use gpui::{Bounds, point, px, size};

    use super::WebSurfaceStore;

    #[test]
    fn typed_text_enters_snapshot_request_after_clicked_viewport() -> Result<(), Box<dyn Error>> {
        let mut store = WebSurfaceStore::new();
        let tab = web_tab("https://example.com/form")?;

        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(640.0), px(480.0)));
        assert!(store.record_viewport_size(tab.id(), bounds));
        assert!(store.record_click_point(
            tab.id(),
            tab.url().as_str(),
            point(px(160.0), px(120.0))
        ));
        assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "e"));
        assert!(store.record_typed_text(tab.id(), tab.url().as_str(), "l"));

        let request = store.prepare_request(&tab).ok_or("missing web surface request")?;

        assert_eq!(request.typed_text.as_deref(), Some("el"));
        assert_eq!(request.snapshot_request.typed_text_for_test(), Some("el"));
        Ok(())
    }

    fn web_tab(url: &str) -> Result<BrowserTab, Box<dyn Error>> {
        Ok(BrowserTab::new(
            TabId::new(),
            SpaceId::new(),
            ProfileId::new(),
            "Web",
            UrlText::parse(url)?,
        ))
    }
}
