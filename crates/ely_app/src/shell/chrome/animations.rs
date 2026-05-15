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

/// Cubic ease-out: `1 - (1 - t)^3`. Decelerates as it approaches `1.0`,
/// matching the productive register's "settles fast" feel used by every
/// reveal / pop / scale animation in the shell. Inlined so callers can
/// shape their `t` value before applying it to opacity or scale without
/// adding a per-frame function-call cost.
pub(crate) fn ease_out_cubic(t: f32) -> f32 {
    let inv = (1.0 - t).clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// Soft fade-in over `duration_ms` once the element first mounts, eased
/// out so the trail end settles to fully opaque without an abrupt cut.
///
/// Drives the design's `panel transition` motion token (180 ms productive)
/// for surfaces like the command overlay and workspace disclosure that
/// appear / disappear in response to user input.
pub(crate) fn fade_in<E>(id: impl Into<ElementId>, duration_ms: u64, element: E) -> impl IntoElement
where
    E: IntoElement + Styled + 'static,
{
    let animation = Animation::new(Duration::from_millis(duration_ms));
    element.with_animation(id, animation, |element, t| element.opacity(ease_out_cubic(t)))
}

#[cfg(test)]
mod tests {
    use super::ease_out_cubic;

    #[test]
    fn ease_out_cubic_anchors_at_endpoints() {
        assert!((ease_out_cubic(0.0)).abs() < f32::EPSILON);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn ease_out_cubic_decelerates() {
        // First half should travel less than the second half — that's
        // the visual signature of ease-out.
        let first_half = ease_out_cubic(0.5);
        let second_half = 1.0 - first_half;
        assert!(first_half > 0.5, "ease-out spends more time near 1.0 ({first_half})");
        assert!(second_half < 0.5);
    }
}
