use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::Space;
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::animations::fade_in;

// ----- Sidebar header layout constants -----
// Single source of truth for the spacing that render_sidebar_header
// applies to its outer column and inner picker row. The disclosure
// anchor solver reads from these so when the layout changes, the
// popover follows automatically.

/// Top padding of the sidebar header column.
const HEADER_PT: f32 = 8.0;
/// Horizontal padding of the sidebar header column.
const HEADER_PX: f32 = 10.0;
/// Vertical padding inside the picker row (top/bottom).
const PICKER_ROW_PY: f32 = 4.0;
/// Horizontal padding inside the picker row (left/right).
const PICKER_ROW_PX: f32 = 2.0;
/// Effective square height of the picker pill row content (matches
/// the prior tile/add-button size so vertical rhythm is preserved
/// after the tile and add button were folded into a single pill).
const PICKER_BUTTON_SIZE: f32 = 32.0;
/// Visual lift applied to the popover so it nestles just below the
/// picker pill instead of sitting flush on its bottom edge.
const DISCLOSURE_LIFT: f32 = 6.0;
/// Width reserved at the start of the sidebar's picker row for the
/// macOS traffic lights so the picker pill never collides with the
/// close / minimize / maximize circles.
///
/// Derivation: traffic-light origin is `SHELL_INSET + 34 = 50 px` from
/// the window edge (see `main.rs::TRAFFIC_LIGHT_ORIGIN_X`). The macOS
/// system traffic-light group renders ~62 px wide (three ~14 px
/// circles with ~10 px gaps), so the rightmost circle reaches
/// `50 + 62 = 112 px`. The pill's outer left edge sits at
/// `SHELL_INSET + 1 px border + HEADER_PX + PICKER_ROW_PX + RESERVE`
/// = `29 + RESERVE`. Setting RESERVE to 100 px puts the pill at
/// `129 px` — 17 px of breathing room past the green button, enough
/// that the pill's drop-shadow doesn't visually bleed into the
/// circles even on Retina sub-pixel positioning.
pub(crate) const TRAFFIC_LIGHT_RESERVE: f32 = 100.0;

pub(crate) fn render_sidebar_header(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_space = snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id);
    let picker_open = shell.workspace_picker_open;

    div()
        .flex()
        .flex_col()
        .pt(px(HEADER_PT))
        .px(px(HEADER_PX))
        .pb(px(HEADER_PX))
        .flex_shrink_0()
        .child(render_workspace_picker(snapshot, active_space, picker_open, cx))
        .into_any_element()
}

fn render_workspace_picker(
    _snapshot: &BrowserSnapshot,
    active_space: Option<&Space>,
    picker_open: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    // The disclosure renders at the window root (see render.rs) so it
    // can sit above a fullscreen click-out backdrop and extend past
    // the sidebar's overflow_hidden clip. After folding the dashboard
    // tile and the "+" button into a single workspace identity, the
    // pill is the sole occupant of the picker row — it just reserves
    // space at the start for the macOS traffic lights that float in
    // this same horizontal band.
    div()
        .flex()
        .items_center()
        .py(px(PICKER_ROW_PY))
        .px(px(PICKER_ROW_PX))
        .pl(px(TRAFFIC_LIGHT_RESERVE))
        .child(render_picker_pill(active_space, picker_open, cx))
        .into_any_element()
}

fn render_picker_pill(
    active_space: Option<&Space>,
    picker_open: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_name = active_space.map(|space| space.name().to_string()).unwrap_or_default();
    let space_glyph = active_space.map(|space| space.icon().to_string()).unwrap_or_default();
    let chevron = if picker_open { IconName::ChevronUp } else { IconName::ChevronDown };
    let bg = if picker_open { picker_bg_hover() } else { picker_bg() };

    div()
        .id(SharedString::from("workspace-picker"))
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(9.0))
        .bg(rgba(bg))
        .shadow(soft_shadow())
        .cursor_pointer()
        .hover(|style| style.bg(rgba(picker_bg_hover())))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, _, cx| {
            shell.toggle_workspace_picker(cx);
        }))
        .child(render_workspace_glyph(space_glyph))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child(space_name),
        )
        .child(div().text_color(rgb(colors::ink_3())).child(chevron))
        .into_any_element()
}

/// Window-relative position and width for the disclosure popover.
/// Computed at paint time from the sidebar's live layout so the
/// popover always tracks the picker pill — sidebar resizes, design-
/// system inset tweaks, and header-row adjustments flow through
/// without touching this module.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorkspaceDisclosureAnchor {
    pub top_px: f32,
    pub left_px: f32,
    pub width_px: f32,
}

impl WorkspaceDisclosureAnchor {
    /// Derive the popover anchor from the live sidebar width. This is
    /// the geometric inverse of `render_sidebar_header` — every term
    /// here corresponds to a real CSS-equivalent property applied
    /// upstream, so changing one of those constants moves the popover
    /// with it.
    pub(crate) fn solve(sidebar_width_px: f32) -> Self {
        let left_px = spacing::SHELL_INSET + HEADER_PX + PICKER_ROW_PX + TRAFFIC_LIGHT_RESERVE;

        let top_px = spacing::SHELL_INSET
            + HEADER_PT
            + PICKER_ROW_PY
            + PICKER_BUTTON_SIZE
            + PICKER_ROW_PY
            + DISCLOSURE_LIFT;

        // Pill now owns the picker row alone, so:
        //   pill_w = sidebar_w
        //          - 2 * (HEADER_PX + PICKER_ROW_PX)   (row insets)
        //          - TRAFFIC_LIGHT_RESERVE             (left reserve)
        let chrome = 2.0 * (HEADER_PX + PICKER_ROW_PX) + TRAFFIC_LIGHT_RESERVE;
        let width_px = (sidebar_width_px - chrome).max(0.0);

        Self { top_px, left_px, width_px }
    }
}

/// Window-root popover surface for the workspace picker. Rendered by
/// render_browser when `workspace_picker_open` is true. Pinned to the
/// solved anchor for the live sidebar width so the surface lines up
/// with the trigger without needing to capture bounds at paint time.
pub(crate) fn render_workspace_disclosure(
    snapshot: &BrowserSnapshot,
    anchor: WorkspaceDisclosureAnchor,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_id = snapshot.active_space_id.clone();

    let body = div()
        .absolute()
        .top(px(anchor.top_px))
        .left(px(anchor.left_px))
        .w(px(anchor.width_px))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(4.0))
        .rounded(px(10.0))
        .bg(rgba(disclosure_bg()))
        .border_1()
        .border_color(rgba(disclosure_border()))
        .shadow(soft_shadow())
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
            }),
        )
        .children(
            snapshot
                .spaces
                .iter()
                .enumerate()
                .map(|(index, space)| render_disclosure_row(index, space, &active_id, cx)),
        )
        .child(render_new_workspace_row(cx))
        .child(render_disclosure_footer(cx));

    fade_in("workspace-disclosure", 140, body).into_any_element()
}

/// Transparent fullscreen backdrop layered between the layout grid
/// and the disclosure. Press on it closes the picker AND consumes
/// the event so the dismiss-click doesn't ricochet into a button
/// underneath the cursor.
pub(crate) fn render_workspace_disclosure_backdrop(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("workspace-picker-backdrop"))
        .absolute()
        .inset_0()
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|shell, _: &gpui::MouseDownEvent, _, cx| {
                shell.close_workspace_picker(cx);
                cx.stop_propagation();
            }),
        )
        .into_any_element()
}

fn render_disclosure_row(
    index: usize,
    space: &Space,
    active_id: &ely_domain::SpaceId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_id = space.id().clone();
    let active = space.id() == active_id;
    let bg = if active { disclosure_row_active_bg() } else { 0x00000000 };

    div()
        .id(SharedString::from(format!("workspace-row-{index}")))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(8.0))
        .bg(rgba(bg))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(disclosure_row_hover_bg())))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_space_from_picker(&space_id, window, cx);
        }))
        .child(render_workspace_glyph(space.icon().to_string()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child(space.name().to_string()),
        )
        .when(active, |el| el.child(div().text_color(rgb(colors::accent())).child(IconName::Check)))
        .into_any_element()
}

fn render_disclosure_footer(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("workspace-manage"))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .mt(px(2.0))
        .rounded(px(8.0))
        .border_t_1()
        .border_color(rgba(colors::divider()))
        .text_size(px(12.0))
        .text_color(rgb(colors::ink_3()))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(disclosure_row_hover_bg())).text_color(rgb(colors::ink())))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.close_workspace_picker(cx);
            shell.open_internal_tab("ely://settings/spaces", window, cx);
        }))
        .child(div().text_color(rgb(colors::ink_4())).child(IconName::Settings))
        .child("Manage spaces")
        .into_any_element()
}

fn render_workspace_glyph(emoji: String) -> AnyElement {
    div()
        .size(px(18.0))
        .rounded(px(5.0))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(341.0 / 360.0, 0.78, 0.67, 1.0), 0.0),
            linear_color_stop(hsla(15.0 / 360.0, 0.55, 0.53, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(rgb(0xffffff))
        .child(emoji)
        .into_any_element()
}

fn render_new_workspace_row(cx: &mut Context<ElyShell>) -> AnyElement {
    // The "+" now lives inside the disclosure as a peer row to the
    // existing spaces, keeping a single source of "workspace identity"
    // visible at rest. Click semantics mirror the old top-of-sidebar
    // button so existing flows keep working.
    div()
        .id(SharedString::from("workspace-add"))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(8.0))
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight(500.0))
        .text_color(rgb(colors::ink_2()))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(disclosure_row_hover_bg())).text_color(rgb(colors::ink())))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.create_workspace_from_picker(window, cx);
        }))
        .child(div().text_color(rgb(colors::ink_3())).child(IconName::Plus))
        .child(div().flex_1().min_w_0().truncate().child("New workspace"))
        .into_any_element()
}

fn picker_bg() -> u32 {
    colors::pick(0xffffff99, 0x1f1d1b99)
}
fn picker_bg_hover() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
fn disclosure_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
fn disclosure_border() -> u32 {
    colors::pick(0xffffff80, 0x1f1d1b80)
}
fn disclosure_row_active_bg() -> u32 {
    colors::pick(0xffffffeb, 0x1f1d1beb)
}
fn disclosure_row_hover_bg() -> u32 {
    colors::pick(0xffffffb3, 0x1f1d1bb3)
}

fn soft_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(0.0, 0.0, 1.0, 0.7),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
        },
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.08),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_lines_up_with_default_sidebar_picker_pill() {
        // Default sidebar (280 px) should land the popover at the
        // window-relative position the visual design was tuned for.
        // The picker pill now owns the row alone (tile + add-button
        // folded into the disclosure), so the anchor sits flush to
        // the traffic-light reserve with no extra tile offset.
        let anchor = WorkspaceDisclosureAnchor::solve(280.0);
        assert_eq!(anchor.left_px, 128.0);
        assert_eq!(anchor.top_px, 70.0);
        assert_eq!(anchor.width_px, 156.0);
    }

    #[test]
    fn anchor_width_tracks_resized_sidebar() {
        // Wider sidebar -> wider pill -> wider popover. Top/left
        // stay locked to the trigger row.
        let narrow = WorkspaceDisclosureAnchor::solve(240.0);
        let wide = WorkspaceDisclosureAnchor::solve(360.0);
        assert_eq!(narrow.left_px, wide.left_px);
        assert_eq!(narrow.top_px, wide.top_px);
        assert_eq!(narrow.width_px, 116.0);
        assert_eq!(wide.width_px, 236.0);
    }

    #[test]
    fn anchor_width_clamps_at_zero_for_pathologically_small_sidebar() {
        // Solver must never report negative geometry, even if the
        // sidebar is somehow narrower than the chrome it contains.
        let anchor = WorkspaceDisclosureAnchor::solve(0.0);
        assert_eq!(anchor.width_px, 0.0);
    }
}
