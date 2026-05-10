use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgba};

/// Inner highlight rings on every glass panel.
///
/// CSS designs reach for `box-shadow: inset 0 0 0 1px rgba(255,255,255,0.5)`
/// to draw a 1 px highlight inside a translucent panel. GPUI 0.2.2's
/// `BoxShadow` has no inset flag, so this module composites the same effect
/// from real GPUI primitives: an absolutely-positioned overlay div that
/// owns the inner border and sits above the panel content, scoped to the
/// panel's rounded clip.
///
/// The highlight color matches the design's `--ely-shadow-window` stack:
/// 50% white border + 70% white top-edge highlight.
const HIGHLIGHT_BORDER: u32 = 0xffffff80;
const HIGHLIGHT_TOP_EDGE: u32 = 0xffffffb3;

/// Draw a single absolute overlay that paints the inset highlight ring into
/// the parent panel. Caller is responsible for `.relative()` on the parent
/// and matching `.rounded(...)` so the overlay's clip lines up.
pub(crate) fn render_inner_highlight(radius_px: f32) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius_px))
        .border_1()
        .border_color(rgba(HIGHLIGHT_BORDER))
        // The 1 px top-edge highlight from the design is drawn via a child
        // div pinned to the top edge — GPUI doesn't have asymmetric border
        // colors, so a 1-pixel-tall sliver is the cleanest substitute.
        .child(
            div()
                .absolute()
                .top_0()
                .left(px(1.0))
                .right(px(1.0))
                .h(px(1.0))
                .bg(rgba(HIGHLIGHT_TOP_EDGE)),
        )
        .into_any_element()
}
