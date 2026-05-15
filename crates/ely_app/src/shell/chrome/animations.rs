use std::time::Duration;

use gpui::{Animation, AnimationExt, ElementId, IntoElement, Styled};

/// One-second blink that toggles between fully visible and invisible at the
/// half-period mark, matching the design's
/// `animation: blink 1s infinite`.
///
/// We deliberately use `with_animation` rather than rolling a Timer / cx.spawn
/// loop — GPUI requeues the layout pass on each frame the animation owns, so
/// CPU stays idle when the caret is offscreen and the visible state is
/// stable inside one render. The animation re-enters the closure with `t` in
/// `[0, 1]`; we snap to a square wave so the caret pops cleanly instead of
/// fading in and out.
pub(crate) fn blink<E>(id: impl Into<ElementId>, element: E) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    element.with_animation(id, Animation::new(Duration::from_secs(1)).repeat(), |element, t| {
        element.opacity(if t < 0.5 { 1.0 } else { 0.0 })
    })
}

/// Soft fade-in over `panel_transition_ms` once the element first mounts.
///
/// Drives the design's `panel transition` motion token (180 ms productive)
/// for surfaces like the command overlay and workspace disclosure that
/// appear / disappear in response to user input.
pub(crate) fn fade_in<E>(id: impl Into<ElementId>, duration_ms: u64, element: E) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    let animation = Animation::new(Duration::from_millis(duration_ms));
    element.with_animation(id, animation, |element, t| element.opacity(t))
}
