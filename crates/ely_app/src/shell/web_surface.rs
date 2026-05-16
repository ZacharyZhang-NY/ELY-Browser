use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ely_domain::{BrowserTab, TabId};
use gpui::{Bounds, Pixels, Point};

use crate::services::ProfileDataMode;

use super::{
    web_surface_cadence::IDLE_POLL_INTERVAL,
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceSize},
    web_surface_metadata::WebSurfacePageMetadata,
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime::{WebSurfaceRuntime, WebSurfaceRuntimeFrame},
    web_surface_state::{
        PerTabSurface, WebSurfaceClickState, WebSurfaceEnsureKey, WebSurfaceInputOutcome,
        WebSurfaceKeyboardFocusState, WebSurfacePendingInput, WebSurfaceScrollState,
        WebSurfaceState, WebSurfaceTextInputState, WebSurfaceTickResult,
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
        Self { runtime: WebSurfaceRuntime::new(), surfaces: BTreeMap::new(), keyboard_focus: None }
    }

    #[cfg(test)]
    pub(super) fn new_with_runtime(runtime: WebSurfaceRuntime) -> Self {
        Self { runtime, surfaces: BTreeMap::new(), keyboard_focus: None }
    }

    pub(super) fn state(&self, tab_id: &TabId) -> Option<&WebSurfaceState> {
        self.surfaces.get(tab_id).and_then(|surface| surface.state.as_ref())
    }

    pub(super) fn ensure_surface(
        &mut self,
        tab: &BrowserTab,
        profile_data_mode: ProfileDataMode,
        permissions: &[WebSurfaceSitePermission],
    ) -> bool {
        if !is_external_web_url(tab.url().as_str()) {
            return false;
        }
        let requested_url = tab.url().as_str().to_string();
        let Some(size) = self.surfaces.get(tab.id()).and_then(|surface| surface.viewport_size)
        else {
            return false;
        };
        let ensure_key =
            WebSurfaceEnsureKey::new(requested_url.clone(), size, tab.zoom_percent(), permissions);
        if self.surfaces.get(tab.id()).is_some_and(|surface| !surface.should_ensure(&ensure_key)) {
            return false;
        }
        let input = self.take_pending_input(tab.id(), requested_url.as_str());
        let previous_frame =
            self.previous_ready_frame(tab.id(), requested_url.as_str(), tab.zoom_percent());

        match self.runtime.ensure_tab(tab, size, profile_data_mode, permissions, input) {
            Ok(result) => {
                self.surface_mut(tab.id()).mark_ensured(ensure_key);
                if result.started_loading {
                    self.surface_mut(tab.id()).state = Some(WebSurfaceState::Loading {
                        requested_url: result.requested_url,
                        previous_frame,
                    });
                    return true;
                }
                false
            }
            Err(message) => {
                self.surface_mut(tab.id()).mark_ensured(ensure_key);
                self.surface_mut(tab.id()).state = Some(WebSurfaceState::Failed { message });
                true
            }
        }
    }

    pub(super) fn tick(&mut self, visible_tab_ids: &[TabId]) -> WebSurfaceTickResult {
        let frames = self.runtime.tick(visible_tab_ids);
        let mut result = WebSurfaceTickResult::default();

        for frame in frames {
            match frame {
                WebSurfaceRuntimeFrame::Ready { tab_id, frame, url_change } => {
                    let had_ready = matches!(
                        self.surfaces.get(&tab_id).and_then(|surface| surface.state.as_ref()),
                        Some(WebSurfaceState::Ready(_))
                    );
                    match self.initial_display_gate_message(&tab_id, &frame, had_ready) {
                        Ok(()) => {}
                        Err(message) => {
                            if !had_ready {
                                self.surface_mut(&tab_id).state =
                                    Some(WebSurfaceState::Failed { message });
                                result.changed = true;
                            }
                            continue;
                        }
                    }
                    if self.should_hold_initial_frame(&tab_id, &frame, had_ready) {
                        if !had_ready {
                            self.surface_mut(&tab_id).state = Some(WebSurfaceState::Loading {
                                requested_url: frame.requested_url.clone(),
                                previous_frame: None,
                            });
                            result.changed = true;
                        }
                        continue;
                    }
                    result
                        .page_metadata
                        .extend(WebSurfacePageMetadata::from_frame(&tab_id, &frame));
                    if self
                        .surfaces
                        .get(&tab_id)
                        .is_none_or(|surface| !surface.matches_ready(&frame))
                    {
                        self.surface_mut(&tab_id).state = Some(WebSurfaceState::Ready(*frame));
                        result.changed = true;
                    }
                    result.url_changes.extend(url_change);
                }
                WebSurfaceRuntimeFrame::Failed { tab_id, message } => {
                    let had_ready = matches!(
                        self.surfaces.get(&tab_id).and_then(|surface| surface.state.as_ref()),
                        Some(WebSurfaceState::Ready(_))
                    );
                    if had_ready {
                        tracing::warn!(
                            target: "ely::web_surface",
                            tab_id = %tab_id,
                            message = %message,
                            "transient surface error; keeping last frame",
                        );
                        continue;
                    }
                    self.surface_mut(&tab_id).state = Some(WebSurfaceState::Failed { message });
                    result.changed = true;
                }
            }
        }

        result
    }

    pub(super) fn next_tick_delay(&self, visible_tab_ids: &[TabId]) -> Duration {
        self.runtime
            .next_poll_delay(visible_tab_ids, Instant::now())
            .unwrap_or(IDLE_POLL_INTERVAL)
            .min(IDLE_POLL_INTERVAL)
    }

    pub(super) fn retain_tabs(&mut self, open_tab_ids: &[TabId]) {
        let stale_tab_ids = self
            .surfaces
            .keys()
            .filter(|tab_id| !open_tab_ids.contains(*tab_id))
            .cloned()
            .collect::<Vec<_>>();
        for tab_id in stale_tab_ids {
            self.close_surface(&tab_id);
        }
    }

    pub(super) fn record_scroll_delta(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        delta: Point<Pixels>,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let Some(delta) = WebSurfaceScrollDelta::from_point(delta, scale_factor) else {
            return WebSurfaceInputOutcome::DroppedZeroDelta;
        };
        let Some(bounds) = self.surfaces.get(tab_id).and_then(|surface| surface.viewport_bounds)
        else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let Some(point) =
            WebSurfaceClickPoint::from_window_position(bounds, position, scale_factor)
        else {
            return WebSurfaceInputOutcome::DroppedOutOfBounds;
        };

        let surface = self.surface_mut(tab_id);
        let flush_throttled = surface.input_flush_is_throttled(Instant::now());
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
        surface.pending_scroll_point = Some(point);
        surface.mark_pending_input_started();
        surface.click_point = None;
        if flush_throttled {
            WebSurfaceInputOutcome::Buffered
        } else {
            WebSurfaceInputOutcome::Applied
        }
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
            return WebSurfaceInputOutcome::Applied;
        };

        if current_size == size {
            return WebSurfaceInputOutcome::NoChange;
        }

        surface.viewport_size = Some(size);
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_hover_point(
        &mut self,
        tab_id: &TabId,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        self.record_hover_point_at(tab_id, position, scale_factor, Instant::now())
    }

    fn record_hover_point_at(
        &mut self,
        tab_id: &TabId,
        position: Point<Pixels>,
        scale_factor: f32,
        now: Instant,
    ) -> WebSurfaceInputOutcome {
        let Some(surface) = self.surfaces.get_mut(tab_id) else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let Some(bounds) = surface.viewport_bounds else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let Some(point) =
            WebSurfaceClickPoint::from_window_position(bounds, position, scale_factor)
        else {
            return WebSurfaceInputOutcome::DroppedOutOfBounds;
        };
        if surface.hover_point == Some(point) {
            return WebSurfaceInputOutcome::NoChange;
        }
        if surface.hover_is_throttled(now) {
            return WebSurfaceInputOutcome::NoChange;
        }
        surface.hover_point = Some(point);
        surface.mark_hover_enqueued(now);
        WebSurfaceInputOutcome::Applied
    }

    pub(super) fn record_click_point(
        &mut self,
        tab_id: &TabId,
        requested_url: &str,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        let Some(bounds) = self.surfaces.get(tab_id).and_then(|surface| surface.viewport_bounds)
        else {
            return WebSurfaceInputOutcome::DroppedNoViewportBounds;
        };
        let Some(point) =
            WebSurfaceClickPoint::from_window_position(bounds, position, scale_factor)
        else {
            return WebSurfaceInputOutcome::DroppedOutOfBounds;
        };

        let scroll_offset = self
            .surfaces
            .get(tab_id)
            .map(|surface| surface.scroll_offset_for(requested_url))
            .unwrap_or_default();

        let state =
            WebSurfaceClickState { requested_url: requested_url.to_string(), scroll_offset, point };
        self.keyboard_focus = Some(WebSurfaceKeyboardFocusState {
            tab_id: tab_id.clone(),
            requested_url: requested_url.to_string(),
            scroll_offset: state.scroll_offset,
            click_point: state.point,
        });
        let surface = self.surface_mut(tab_id);
        surface.typed_text = None;
        surface.click_point = Some(state);
        surface.mark_pending_input_started();
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
        surface.mark_pending_input_started();
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
        let scroll_point = surface.pending_scroll_point.take();
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
        let enqueued_at = surface.pending_input_started_at.take();
        if scroll_delta.is_some()
            || click_point.is_some()
            || hover_point.is_some()
            || typed_text.is_some()
        {
            surface.mark_input_flushed(Instant::now());
        }

        WebSurfacePendingInput {
            enqueued_at,
            scroll_offset,
            scroll_delta,
            scroll_point,
            click_point,
            hover_point,
            typed_text,
        }
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

    fn should_hold_initial_frame(
        &self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
        has_previous_frame: bool,
    ) -> bool {
        !has_previous_frame
            && self
                .previous_ready_frame(tab_id, frame.requested_url.as_str(), frame.zoom_percent())
                .is_none()
            && matches!(frame.has_visible_content_for_initial_display(), Ok(false))
    }

    fn initial_display_gate_message(
        &self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
        has_previous_frame: bool,
    ) -> Result<(), String> {
        if has_previous_frame
            || self
                .previous_ready_frame(tab_id, frame.requested_url.as_str(), frame.zoom_percent())
                .is_some()
        {
            return Ok(());
        }
        frame.has_visible_content_for_initial_display().map(|_| ()).map_err(|error| {
            format!(
                "Servo hardware surface initial content check failed for {}: {error}",
                frame.requested_url
            )
        })
    }

    fn surface_mut(&mut self, tab_id: &TabId) -> &mut PerTabSurface {
        self.surfaces.entry(tab_id.clone()).or_insert_with(PerTabSurface::new)
    }

    pub(super) fn close_surface(&mut self, tab_id: &TabId) {
        self.runtime.close_tab(tab_id);
        self.surfaces.remove(tab_id);
        if self.keyboard_focus.as_ref().is_some_and(|focus| focus.tab_id == *tab_id) {
            self.keyboard_focus = None;
        }
    }

    #[cfg(test)]
    pub(super) fn surface_for_test(&self, tab_id: &TabId) -> Option<&PerTabSurface> {
        self.surfaces.get(tab_id)
    }

    #[cfg(test)]
    pub(super) fn flush_runtime_for_test(&self) {
        self.runtime.flush_for_test();
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
