use std::time::{Duration, Instant};

use ely_domain::TabId;
use gpui::{Bounds, NativeSurfaceHandle, Pixels};

use super::{
    web_surface_cadence::ACTIVE_POLL_INTERVAL,
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{
        WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceScrollOffset, WebSurfaceSize,
    },
    web_surface_metadata::{WebSurfaceMetadataTracker, WebSurfacePageMetadata},
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_runtime::WebSurfaceUrlChange,
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
    pub(super) enqueued_at: Option<Instant>,
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) scroll_delta: Option<WebSurfaceScrollDelta>,
    pub(super) scroll_point: Option<WebSurfaceClickPoint>,
    pub(super) click_point: Option<WebSurfaceClickPoint>,
    pub(super) hover_point: Option<WebSurfaceClickPoint>,
    pub(super) typed_text: Option<String>,
}

/// Outcome of a `WebSurfaceStore::record_*` call.
///
/// Replaces the previous `-> bool` return so silent rejections name
/// themselves at the call site. Callers only branch on `Applied`
/// (every other variant means "do nothing, don't notify"), but each
/// `Dropped*` / non-`Applied` variant pins down *why* a coordinate
/// or keystroke never reached the runtime — so the next regression
/// shows up as a specific variant in tests instead of a missing
/// repaint. `#[must_use]` keeps a future caller from dropping the
/// outcome on the floor and reintroducing the silent-fail pattern.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WebSurfaceInputOutcome {
    /// State changed and the renderer should re-notify.
    Applied,
    /// State changed and the active cadence timer will flush it.
    Buffered,
    /// Same value as currently recorded — nothing to flush downstream.
    NoChange,
    /// Geometry constructor rejected the input (zero/NaN/negative
    /// bounds). The viewport never measured cleanly.
    DroppedInvalidBounds,
    /// Viewport bounds have not been recorded yet, so window-relative
    /// coordinates can't be translated into the page coordinate space.
    DroppedNoViewportBounds,
    /// Window position falls outside the viewport rect after scaling.
    DroppedOutOfBounds,
    /// Wheel delta rounded to zero device pixels in both axes.
    DroppedZeroDelta,
    /// Empty string passed to `record_typed_text` — nothing to buffer.
    DroppedEmptyText,
    /// `record_typed_text` ran before any click established
    /// keyboard focus on this surface.
    DroppedNoKeyboardFocus,
    /// Keyboard focus belongs to a different tab or the URL drifted
    /// (redirect / trailing-slash mismatch) since the focusing click.
    DroppedFocusMismatch,
}

pub(super) enum WebSurfaceState {
    Loading { requested_url: String, previous_frame: Option<WebSurfaceFrame> },
    Ready(WebSurfaceFrame),
    Failed { message: String },
}

#[derive(Default)]
pub(super) struct WebSurfaceTickResult {
    pub(super) changed: bool,
    pub(super) url_changes: Vec<WebSurfaceUrlChange>,
    pub(super) page_metadata: Vec<WebSurfacePageMetadata>,
}

/// All per-tab surface invariants in one owner.
///
/// Replaces the previous 11 parallel `BTreeMap<TabId, _>` fields on
/// `WebSurfaceStore`. Every per-tab field lives here so a single
/// lookup yields the full input/render context for that tab; the
/// remaining cross-tab state (the runtime backend and the singular
/// `keyboard_focus` pointer to the active tab) stays on the store.
///
/// All inputs that depend on a measured viewport take their
/// pre-conditions from this struct (`viewport_bounds`,
/// `viewport_size`), so "viewport not ready" turns into a guard at
/// the call site instead of a `get` on a parallel map that may or
/// may not be populated.
pub(super) struct PerTabSurface {
    pub(super) viewport_bounds: Option<Bounds<Pixels>>,
    pub(super) viewport_size: Option<WebSurfaceSize>,
    /// When [`viewport_size`] last *transitioned* to a new value. The
    /// initial measurement does not update this; only subsequent
    /// genuine size changes do. Drives [`viewport_size_is_settling`]
    /// so a sidebar / window animation that emits a viewport bounds on
    /// every GPUI paint does not translate into a Servo framebuffer
    /// destroy/recreate every frame (the "page flashing" symptom).
    viewport_size_changed_at: Option<Instant>,
    pub(super) native_surface: Option<NativeSurfaceHandle>,
    pub(super) last_ensure_key: Option<WebSurfaceEnsureKey>,
    pub(super) hover_point: Option<WebSurfaceClickPoint>,
    last_hover_enqueued_at: Option<Instant>,
    pub(super) click_point: Option<WebSurfaceClickState>,
    pub(super) pending_scroll_delta: Option<WebSurfaceScrollDelta>,
    pub(super) pending_scroll_point: Option<WebSurfaceClickPoint>,
    pub(super) pending_input_started_at: Option<Instant>,
    pub(super) scroll_offset: Option<WebSurfaceScrollState>,
    pub(super) typed_text: Option<WebSurfaceTextInputState>,
    pub(super) state: Option<WebSurfaceState>,
    last_input_flushed_at: Option<Instant>,
    metadata_tracker: WebSurfaceMetadataTracker,
}

impl PerTabSurface {
    pub(super) fn new() -> Self {
        Self {
            viewport_bounds: None,
            viewport_size: None,
            viewport_size_changed_at: None,
            native_surface: None,
            last_ensure_key: None,
            hover_point: None,
            last_hover_enqueued_at: None,
            click_point: None,
            pending_scroll_delta: None,
            pending_scroll_point: None,
            pending_input_started_at: None,
            scroll_offset: None,
            typed_text: None,
            state: None,
            last_input_flushed_at: None,
            metadata_tracker: WebSurfaceMetadataTracker::default(),
        }
    }

    pub(super) fn mark_pending_input_started(&mut self) {
        self.pending_input_started_at.get_or_insert_with(Instant::now);
    }

    pub(super) fn hover_is_throttled(&self, now: Instant) -> bool {
        self.last_hover_enqueued_at
            .is_some_and(|last| now.duration_since(last) < HOVER_INPUT_MIN_INTERVAL)
    }

    pub(super) fn mark_hover_enqueued(&mut self, now: Instant) {
        self.last_hover_enqueued_at = Some(now);
    }

    pub(super) fn input_flush_is_throttled(&self, now: Instant) -> bool {
        self.last_input_flushed_at
            .is_some_and(|last| now.duration_since(last) < ACTIVE_POLL_INTERVAL)
    }

    pub(super) fn mark_input_flushed(&mut self, now: Instant) {
        self.last_input_flushed_at = Some(now);
    }

    pub(super) fn should_ensure(&self, key: &WebSurfaceEnsureKey) -> bool {
        self.last_ensure_key.as_ref() != Some(key) || self.has_pending_input()
    }

    /// True when the viewport bounds have changed within the last
    /// [`VIEWPORT_RESIZE_DEBOUNCE`] window. The renderer treats this
    /// as "the user is mid-animation" and holds off telling Servo to
    /// resize its rendering context until the gesture settles —
    /// Servo's surfman backend recreates the framebuffer on every
    /// `rendering_context.resize`, which would otherwise flash a
    /// blank surface for every animation frame.
    pub(super) fn viewport_size_is_settling(&self, now: Instant) -> bool {
        self.viewport_size_changed_at
            .is_some_and(|last| now.duration_since(last) < VIEWPORT_RESIZE_DEBOUNCE)
    }

    /// Stamp the moment a genuine viewport size transition happened.
    /// Only called when [`viewport_size`] is actually moving to a new
    /// value — the very first measurement does *not* invoke this so
    /// `viewport_size_is_settling` stays false at page-load time and
    /// the initial `ensure_surface` is allowed to fire immediately.
    pub(super) fn mark_viewport_size_changed(&mut self, now: Instant) {
        self.viewport_size_changed_at = Some(now);
    }

    /// Test-only escape hatch: simulate the trailing edge of a resize
    /// animation by clearing the debounce timestamp, so a synchronous
    /// `record + ensure` sequence in tests behaves as if the gesture
    /// has fully settled. Production code drives this via the poll
    /// cadence and time elapsing — tests can't advance an `Instant`.
    #[cfg(test)]
    pub(super) fn clear_viewport_resize_debounce_for_test(&mut self) {
        self.viewport_size_changed_at = None;
    }

    pub(super) fn mark_ensured(&mut self, key: WebSurfaceEnsureKey) {
        self.last_ensure_key = Some(key);
    }

    pub(super) fn matches_ready(&self, frame: &WebSurfaceFrame) -> bool {
        match self.state.as_ref() {
            Some(WebSurfaceState::Ready(current)) => current.has_same_software_render_as(frame),
            _ => false,
        }
    }

    pub(super) fn changed_page_metadata(
        &mut self,
        tab_id: &TabId,
        frame: &WebSurfaceFrame,
    ) -> Option<WebSurfacePageMetadata> {
        self.metadata_tracker.changed_metadata_for(tab_id, frame)
    }

    fn has_pending_input(&self) -> bool {
        self.hover_point.is_some()
            || self.click_point.is_some()
            || self.pending_scroll_delta.is_some()
            || self.pending_scroll_point.is_some()
            || self.typed_text.is_some()
    }

    pub(super) fn scroll_offset_for(&self, requested_url: &str) -> WebSurfaceScrollOffset {
        self.scroll_offset
            .as_ref()
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.offset)
            .unwrap_or_default()
    }
}

const HOVER_INPUT_MIN_INTERVAL: Duration = Duration::from_millis(32);

/// How long the viewport must hold its new size before we propagate
/// the resize down to Servo. 80 ms is short enough that a user-driven
/// drag still feels live, and long enough to collapse a 60 Hz sidebar
/// animation (~17 ms per frame) into a single trailing-edge resize.
const VIEWPORT_RESIZE_DEBOUNCE: Duration = Duration::from_millis(80);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceEnsureKey {
    requested_url: String,
    size: WebSurfaceSize,
    native_surface_id: Option<usize>,
    zoom_percent: u16,
    permissions: Vec<WebSurfaceSitePermission>,
}

impl WebSurfaceEnsureKey {
    pub(super) fn new(
        requested_url: String,
        size: WebSurfaceSize,
        native_surface: Option<&NativeSurfaceHandle>,
        zoom_percent: u16,
        permissions: &[WebSurfaceSitePermission],
    ) -> Self {
        Self {
            requested_url,
            size,
            native_surface_id: native_surface.map(NativeSurfaceHandle::identity),
            zoom_percent,
            permissions: permissions.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::*;

    #[test]
    fn unchanged_surface_without_input_skips_ensure() {
        let key = ensure_key("https://example.com/", 800, 600);
        let mut surface = PerTabSurface::new();

        assert!(surface.should_ensure(&key));

        surface.mark_ensured(key.clone());

        assert!(!surface.should_ensure(&key));
    }

    #[test]
    fn pending_input_forces_ensure_even_when_key_matches() {
        let key = ensure_key("https://example.com/", 800, 600);
        let mut surface = PerTabSurface::new();
        surface.mark_ensured(key.clone());
        surface.pending_scroll_delta =
            WebSurfaceScrollDelta::from_point(point(px(0.0), px(120.0)), 1.0);

        assert!(surface.should_ensure(&key));
    }

    #[test]
    fn viewport_change_forces_ensure() {
        let old_key = ensure_key("https://example.com/", 800, 600);
        let new_key = ensure_key("https://example.com/", 1024, 768);
        let mut surface = PerTabSurface::new();
        surface.mark_ensured(old_key);

        assert!(surface.should_ensure(&new_key));
    }

    #[test]
    fn recent_input_flush_throttles_immediate_flush() {
        let start = Instant::now();
        let mut surface = PerTabSurface::new();

        surface.mark_input_flushed(start);

        assert!(surface.input_flush_is_throttled(start + Duration::from_millis(7)));
        assert!(!surface.input_flush_is_throttled(start + Duration::from_millis(8)));
    }

    fn ensure_key(url: &str, width: u32, height: u32) -> WebSurfaceEnsureKey {
        WebSurfaceEnsureKey::new(
            url.to_string(),
            WebSurfaceSize { width, height, device_pixel_ratio_percent: 100 },
            None,
            100,
            &[],
        )
    }
}
