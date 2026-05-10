use std::collections::BTreeMap;

use ely_domain::{BrowserTab, TabId};
use gpui::{Bounds, Pixels, Point};

use crate::services::ProfileDataMode;

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceSize},
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime::{WebSurfaceRuntime, WebSurfaceRuntimeFrame},
    web_surface_state::{
        PerTabSurface, WebSurfaceClickState, WebSurfaceInputOutcome, WebSurfaceKeyboardFocusState,
        WebSurfacePendingInput, WebSurfaceScrollState, WebSurfaceState, WebSurfaceTextInputState,
    },
};

pub(super) struct WebSurfaceStore {
    runtime: WebSurfaceRuntime,
    /// Single owner of every per-tab invariant. See [`PerTabSurface`].
    surfaces: BTreeMap<TabId, PerTabSurface>,
    /// Singleton because only one tab at a time holds keyboard focus
    /// across the whole window. Lives on the store, not per-tab.
    keyboard_focus: Option<WebSurfaceKeyboardFocusState>,
}

impl WebSurfaceStore {
    pub(super) fn new() -> Self {
        Self {
            runtime: WebSurfaceRuntime::new(),
            surfaces: BTreeMap::new(),
            keyboard_focus: None,
        }
    }

    pub(super) fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref())
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
        let Some(size) = self.surfaces.get(tab.id()).and_then(|surface| surface.viewport_size)
        else {
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
                self.surface_mut(tab.id()).state = Some(WebSurfaceState::Ready(frame));
            }
            Ok(result) if result.started_loading => {
                self.surface_mut(tab.id()).state = Some(WebSurfaceState::Loading {
                    requested_url: result.requested_url,
                    previous_frame,
                });
            }
            Ok(_) => {}
            Err(message) => {
                self.surface_mut(tab.id()).state = Some(WebSurfaceState::Failed { message });
            }
        }
    }

    pub(super) fn tick(&mut self) -> bool {
        let frames = self.runtime.tick();
        let mut changed = false;

        for frame in frames {
            match frame {
                WebSurfaceRuntimeFrame::Ready { tab_id, frame } => {
                    self.surface_mut(&tab_id).state = Some(WebSurfaceState::Ready(frame));
                    changed = true;
                }
                WebSurfaceRuntimeFrame::Failed { tab_id, message } => {
                    self.surface_mut(&tab_id).state = Some(WebSurfaceState::Failed { message });
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
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let Some(delta) = WebSurfaceScrollDelta::from_point(delta, scale_factor) else {
            return WebSurfaceInputOutcome::DroppedZeroDelta;
        };

        let surface = self.surface_mut(tab_id);
        let scroll = surface
            .scroll_offset
            .get_or_insert_with(|| WebSurfaceScrollState::new(requested_url.to_string()));
        if scroll.requested_url != requested_url {
            *scroll = WebSurfaceScrollState::new(requested_url.to_string());
        }

        scroll.offset = scroll.offset.scrolled_by(delta);
        surface.pending_scroll_delta = Some(match surface.pending_scroll_delta {
            Some(current) => current.combined_with(delta),
            None => delta,
        });
        // Drop any buffered click — its viewport coordinates were
        // captured against the pre-scroll page, so applying it after
        // the scroll would land on the wrong DOM element. Keep
        // `keyboard_focus` and `typed_text` though: Servo maintains
        // its own DOM focus across scrolls, so a focused input keeps
        // accepting the user's keystrokes after they wheel-scroll.
        surface.click_point = None;
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_viewport_size(
        &mut self,
        tab_id: &TabId,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let Some(size) = WebSurfaceSize::from_bounds(bounds, scale_factor) else {
            return WebSurfaceInputOutcome::DroppedInvalidBounds;
        };
        let surface = self.surface_mut(tab_id);
        surface.viewport_bounds = Some(bounds);

        let Some(current_size) = surface.viewport_size else {
            surface.viewport_size = Some(size);
            surface.pending_viewport_size = None;
            return WebSurfaceInputOutcome::Applied;
        };

        if current_size == size {
            surface.pending_viewport_size = None;
            return WebSurfaceInputOutcome::NoChange;
        }

        if surface.pending_viewport_size != Some(size) {
            surface.pending_viewport_size = Some(size);
            return WebSurfaceInputOutcome::Buffered;
        }

        surface.pending_viewport_size = None;
        surface.viewport_size = Some(size);
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_hover_point(
        &mut self,
        tab_id: &TabId,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let surface = self.surfaces.get_mut(tab_id).filter(|surface| surface.viewport_bounds.is_some());
        let Some(surface) = surface else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let bounds = surface.viewport_bounds.expect("viewport_bounds checked above");
        let Some(point) = WebSurfaceClickPoint::from_window_position(bounds, position, scale_factor)
        else {
            return WebSurfaceInputOutcome::DroppedOutOfBounds;
        };
        surface.hover_point = Some(point);
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_click_point(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let Some(bounds) =
            self.surfaces.get(tab_id).and_then(|surface| surface.viewport_bounds)
        else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let Some(point) = WebSurfaceClickPoint::from_window_position(bounds, position, scale_factor)
        else {
            return WebSurfaceInputOutcome::DroppedOutOfBounds;
        };

        let scroll_offset = self
            .surfaces
            .get(tab_id)
            .map(|surface| surface.scroll_offset_for(requested_url))
            .unwrap_or_default();

        let state = WebSurfaceClickState {
            requested_url: requested_url.to_string(),
            scroll_offset,
            point,
        };
        self.keyboard_focus = Some(WebSurfaceKeyboardFocusState {
            tab_id: tab_id.clone(),
            requested_url: requested_url.to_string(),
            scroll_offset: state.scroll_offset,
            click_point: state.point,
        });
        let surface = self.surface_mut(tab_id);
        surface.typed_text = None;
        surface.click_point = Some(state);
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_typed_text(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        text: &str,
    ) -> WebSurfaceInputOutcome {
        if text.is_empty() {
            return WebSurfaceInputOutcome::DroppedEmptyText;
        }
        let Some(focus) = self.keyboard_focus.as_ref() else {
            return WebSurfaceInputOutcome::DroppedNoKeyboardFocus;
        };
        if focus.tab_id != *tab_id || focus.requested_url != requested_url {
            return WebSurfaceInputOutcome::DroppedFocusMismatch;
        }

        let scroll_offset = focus.scroll_offset;
        let click_point = focus.click_point;
        let surface = self.surface_mut(tab_id);

        let entry = surface.typed_text.get_or_insert_with(|| WebSurfaceTextInputState {
            requested_url: requested_url.to_string(),
            scroll_offset,
            click_point,
            text: String::new(),
        });
        if entry.requested_url != requested_url
            || entry.scroll_offset != scroll_offset
            || entry.click_point != click_point
        {
            *entry = WebSurfaceTextInputState {
                requested_url: requested_url.to_string(),
                scroll_offset,
                click_point,
                text: String::new(),
            };
        }

        entry.text.push_str(text);
        WebSurfaceInputOutcome::Applied
    }

    fn take_pending_input(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
    ) -> WebSurfacePendingInput {
        let surface = self.surface_mut(tab_id);
        let scroll_offset = surface.scroll_offset_for(requested_url);
        let scroll_delta = surface.pending_scroll_delta.take();
        let click_point = surface
            .click_point
            .take()
            .filter(|state| {
                state.requested_url == requested_url && state.scroll_offset == scroll_offset
            })
            .map(|state| state.point);
        let typed_text = surface
            .typed_text
            .take()
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.text);
        let hover_point = surface.hover_point.take();

        WebSurfacePendingInput { scroll_offset, scroll_delta, click_point, hover_point, typed_text }
    }

    fn previous_ready_frame(
        &self,
        tab_id: &TabId,
        requested_url: &str,
        zoom_percent: u16,
    ) -> Option<WebSurfaceFrame> {
        match self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref()) {
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

    fn surface_mut(&mut self, tab_id: &TabId) -> &mut PerTabSurface {
        self.surfaces.entry(tab_id.clone()).or_insert_with(PerTabSurface::new)
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
