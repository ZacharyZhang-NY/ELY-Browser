use std::collections::BTreeMap;

use ely_domain::{BrowserTab, TabId};
use gpui::{Bounds, Pixels, Point};

use crate::services::ProfileDataMode;

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{
        WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceScrollOffset, WebSurfaceSize,
    },
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime::{WebSurfaceRuntime, WebSurfaceRuntimeFrame},
    web_surface_state::{
        WebSurfaceClickState, WebSurfaceKeyboardFocusState, WebSurfacePendingInput,
        WebSurfaceScrollState, WebSurfaceState, WebSurfaceTextInputState,
    },
};

pub(super) struct WebSurfaceStore {
    runtime: WebSurfaceRuntime,
    pending_viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    click_points: BTreeMap<TabId, WebSurfaceClickState>,
    hover_points: BTreeMap<TabId, WebSurfaceClickPoint>,
    keyboard_focus: Option<WebSurfaceKeyboardFocusState>,
    pending_scroll_deltas: BTreeMap<TabId, WebSurfaceScrollDelta>,
    scroll_offsets: BTreeMap<TabId, WebSurfaceScrollState>,
    typed_texts: BTreeMap<TabId, WebSurfaceTextInputState>,
    viewport_bounds: BTreeMap<TabId, Bounds<Pixels>>,
    viewport_sizes: BTreeMap<TabId, WebSurfaceSize>,
    states: BTreeMap<TabId, WebSurfaceState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self {
            runtime: WebSurfaceRuntime::new(),
            pending_viewport_sizes: BTreeMap::new(),
            click_points: BTreeMap::new(),
            hover_points: BTreeMap::new(),
            keyboard_focus: None,
            pending_scroll_deltas: BTreeMap::new(),
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

    pub(super) fn ensure_surface(
        &mut self,
        tab: &BrowserTab,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
    ) {
        if !is_external_web_url(tab.url().as_str()) {
            return;
        }

        let requested_url = tab.url().as_str().to_string();
        let Some(size) = self.viewport_sizes.get(tab.id()).copied() else {
            return;
        };
        let input = self.take_pending_input(tab.id(), requested_url.as_str());
        let previous_frame =
            self.previous_ready_frame(tab.id(), requested_url.as_str(), tab.zoom_percent());

        match self.runtime.ensure_tab(tab, size, profile_data_mode, permissions, input) {
            Ok(result) if result.frame.is_some() => {
                let Some(frame) = result.frame else {
                    return;
                };
                self.states.insert(tab.id().clone(), WebSurfaceState::Ready(frame));
            }
            Ok(result) if result.started_loading => {
                self.states.insert(
                    tab.id().clone(),
                    WebSurfaceState::Loading {
                        requested_url: result.requested_url,
                        previous_frame,
                    },
                );
            }
            Ok(_) => {}
            Err(message) => {
                self.states.insert(tab.id().clone(), WebSurfaceState::Failed { message });
            }
        }
    }

    pub(super) fn tick(&mut self) -> bool {
        let frames = self.runtime.tick();
        let mut changed = false;

        for frame in frames {
            match frame {
                WebSurfaceRuntimeFrame::Ready { tab_id, frame } => {
                    self.states.insert(tab_id, WebSurfaceState::Ready(frame));
                    changed = true;
                }
                WebSurfaceRuntimeFrame::Failed { tab_id, message } => {
                    self.states.insert(tab_id, WebSurfaceState::Failed { message });
                    changed = true;
                }
            }
        }

        changed
    }

    pub(super) fn record_scroll_delta(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        delta: Point<Pixels>,
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

        state.offset = state.offset.scrolled_by(delta);
        self.pending_scroll_deltas
            .entry(tab_id.clone())
            .and_modify(|current| *current = current.combined_with(delta))
            .or_insert(delta);
        // Drop any buffered click — its viewport coordinates were
        // captured against the pre-scroll page, so applying it after
        // the scroll would land on the wrong DOM element. Keep
        // `keyboard_focus` and `typed_texts` though: Servo maintains
        // its own DOM focus across scrolls, so a focused input keeps
        // accepting the user's keystrokes after they wheel-scroll.
        self.click_points.remove(tab_id);
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

    pub(super) fn record_hover_point(
        &mut self,
        tab_id: &TabId,
        position: Point<Pixels>,
    ) -> bool {
        let Some(bounds) = self.viewport_bounds.get(tab_id).copied() else {
            return false;
        };
        let Some(point) = WebSurfaceClickPoint::from_window_position(bounds, position) else {
            return false;
        };
        self.hover_points.insert(tab_id.clone(), point);
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

    fn take_pending_input(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
    ) -> WebSurfacePendingInput {
        let scroll_offset = self.scroll_offset_for(tab_id, requested_url);
        let scroll_delta = self.pending_scroll_deltas.remove(tab_id);
        let click_point = self
            .click_points
            .remove(tab_id)
            .filter(|state| {
                state.requested_url == requested_url && state.scroll_offset == scroll_offset
            })
            .map(|state| state.point);
        let typed_text = self
            .typed_texts
            .remove(tab_id)
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.text);

        let hover_point = self.hover_points.remove(tab_id);

        WebSurfacePendingInput { scroll_offset, scroll_delta, click_point, hover_point, typed_text }
    }

    fn previous_ready_frame(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        zoom_percent: u16,
    ) -> Option<WebSurfaceFrame> {
        match self.states.get(tab_id) {
            Some(WebSurfaceState::Ready(frame))
                if frame.requested_url == requested_url && frame.zoom_percent() == zoom_percent =>
            {
                Some(frame.clone())
            }
            Some(WebSurfaceState::Loading {
                requested_url: current_url, previous_frame, ..
            }) if current_url == requested_url => previous_frame
                .as_ref()
                .filter(|frame| frame.zoom_percent() == zoom_percent)
                .cloned(),
            _ => None,
        }
    }

    fn scroll_offset_for(&self, tab_id: &TabId, requested_url: &str) -> WebSurfaceScrollOffset {
        self.scroll_offsets
            .get(tab_id)
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.offset)
            .unwrap_or_default()
    }
}

pub(super) fn is_external_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
#[path = "web_surface_tests.rs"]
mod tests;

#[cfg(all(test, feature = "live-site-smoke"))]
#[path = "web_surface_live_site_tests.rs"]
mod web_surface_live_site_tests;
