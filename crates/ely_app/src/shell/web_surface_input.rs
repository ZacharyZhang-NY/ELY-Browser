use std::time::Instant;

use ely_domain::TabId;
use gpui::{Bounds, Pixels, Point};

use super::{
    web_surface::WebSurfaceStore,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceSize},
    web_surface_state::{
        WebSurfaceClickState, WebSurfaceInputOutcome, WebSurfaceKeyboardFocusState,
        WebSurfaceScrollState, WebSurfaceTextInputState,
    },
};

impl WebSurfaceStore {
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
        let now = Instant::now();
        let surface = self.surface_mut(tab_id);
        surface.viewport_bounds = Some(bounds);

        let Some(current_size) = surface.viewport_size else {
            // First measurement: hand it to the runtime immediately so the
            // page can start loading. Don't mark a transition timestamp —
            // there's nothing to debounce yet.
            surface.viewport_size = Some(size);
            return WebSurfaceInputOutcome::Applied;
        };

        if current_size == size {
            return WebSurfaceInputOutcome::NoChange;
        }

        // Genuine size transition. Stamp the moment regardless of
        // outcome so `viewport_size_is_settling` keeps stretching the
        // debounce as the animation emits more frames; the latest size
        // always wins in the cadence-driven retry.
        let was_settling = surface.viewport_size_is_settling(now);
        surface.viewport_size = Some(size);
        surface.mark_viewport_size_changed(now);

        if was_settling {
            // Inside the debounce window — drop the synchronous flush
            // so the GPUI paint that emitted this bounds doesn't pay a
            // Servo resize. The poll cadence already scheduled by the
            // first frame of this animation re-runs `ensure_surface`,
            // which will pick up the trailing size once the gesture
            // settles past [`VIEWPORT_RESIZE_DEBOUNCE`].
            WebSurfaceInputOutcome::Buffered
        } else {
            WebSurfaceInputOutcome::Applied
        }
    }

    pub(super) fn record_hover_point(
        &mut self,
        tab_id: &TabId,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> WebSurfaceInputOutcome {
        self.record_hover_point_at(tab_id, position, scale_factor, Instant::now())
    }

    pub(super) fn record_hover_point_at(
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
}
