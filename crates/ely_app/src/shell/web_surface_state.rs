use ely_domain::TabId;
use gpui::{Bounds, Pixels};

use super::{
    web_surface_frame::WebSurfaceFrame,
    web_surface_geometry::{
        WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceScrollOffset, WebSurfaceSize,
    },
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
    pub(super) scroll_offset: WebSurfaceScrollOffset,
    pub(super) scroll_delta: Option<WebSurfaceScrollDelta>,
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
    /// Same value as currently recorded — nothing to flush downstream.
    NoChange,
    /// First sighting of a new value; held back until a second
    /// matching measurement confirms it (viewport-resize debounce).
    Buffered,
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
    pub(super) pending_viewport_size: Option<WebSurfaceSize>,
    pub(super) hover_point: Option<WebSurfaceClickPoint>,
    pub(super) click_point: Option<WebSurfaceClickState>,
    pub(super) pending_scroll_delta: Option<WebSurfaceScrollDelta>,
    pub(super) scroll_offset: Option<WebSurfaceScrollState>,
    pub(super) typed_text: Option<WebSurfaceTextInputState>,
    pub(super) state: Option<WebSurfaceState>,
}

impl PerTabSurface {
    pub(super) fn new() -> Self {
        Self {
            viewport_bounds: None,
            viewport_size: None,
            pending_viewport_size: None,
            hover_point: None,
            click_point: None,
            pending_scroll_delta: None,
            scroll_offset: None,
            typed_text: None,
            state: None,
        }
    }

    pub(super) fn scroll_offset_for(&self, requested_url: &str) -> WebSurfaceScrollOffset {
        self.scroll_offset
            .as_ref()
            .filter(|state| state.requested_url == requested_url)
            .map(|state| state.offset)
            .unwrap_or_default()
    }
}
