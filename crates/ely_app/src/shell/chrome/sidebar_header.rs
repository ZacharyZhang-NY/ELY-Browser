use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::Space;
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient,
    prelude::FluentBuilder, point, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::animations::fade_in;

pub(crate) fn render_sidebar_header(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_space = snapshot
        .spaces
        .iter()
        .find(|space| space.id() == &snapshot.active_space_id);
    let picker_open = shell.workspace_picker_open;

    div()
        .flex()
        .flex_col()
        .pt(px(8.0))
        .px(px(10.0))
        .pb(px(10.0))
        .gap(px(8.0))
        .flex_shrink_0()
        .child(render_title_row())
        .child(render_workspace_picker(snapshot, active_space, picker_open, cx))
        .into_any_element()
}

fn render_title_row() -> AnyElement {
    div()
        .h(px(20.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        // macOS draws the traffic lights at window (12, 14) — roughly the
        // first 70 px of any sidebar that touches the window's left edge.
        // Pad the title past that span so "ELY Browser ⌄" sits inline with
        // the dots instead of stranded on a row of its own.
        .pl(px(TRAFFIC_LIGHT_RESERVE))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(gpui::FontWeight(500.0))
                .text_color(rgb(colors::INK_2))
                .child("ELY Browser"),
        )
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .opacity(0.6)
                .child(IconName::ChevronDown),
        )
        .into_any_element()
}

/// Width reserved at the start of the sidebar's top row for the macOS
/// traffic lights. Three 12 px dots with 8 px gaps, plus 12 px from the
/// window left edge minus our 16 px shell inset, lands at ~62; we add
/// breathing room and call it 68.
const TRAFFIC_LIGHT_RESERVE: f32 = 68.0;

fn render_workspace_picker(
    _snapshot: &BrowserSnapshot,
    active_space: Option<&Space>,
    picker_open: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    // The disclosure renders at the window root (see render.rs) so it can
    // sit above a fullscreen click-out backdrop and extend past the
    // sidebar's overflow_hidden clip. Picker row keeps just the tile, the
    // pill, and the add button.
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .py(px(4.0))
        .px(px(2.0))
        .child(render_workspaces_tile(cx))
        .child(render_picker_pill(active_space, picker_open, cx))
        .child(render_add_workspace_button(cx))
        .into_any_element()
}

fn render_workspaces_tile(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("workspace-tile"))
        .size(px(32.0))
        .rounded(px(9.0))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(0.0, 0.0, 1.0, 1.0), 0.0),
            linear_color_stop(hsla(20.0 / 360.0, 0.6, 0.94, 1.0), 1.0),
        ))
        .shadow(soft_shadow())
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::ACCENT))
        .cursor_pointer()
        .hover(|style| style.opacity(0.92))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://settings/spaces", window, cx);
        }))
        .child(IconName::LayoutDashboard)
        .into_any_element()
}

fn render_picker_pill(
    active_space: Option<&Space>,
    picker_open: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_name = active_space
        .map(|space| space.name().to_string())
        .unwrap_or_default();
    let space_glyph = active_space
        .map(|space| space.icon().to_string())
        .unwrap_or_default();
    let chevron = if picker_open {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    };
    let bg = if picker_open { PICKER_BG_HOVER } else { PICKER_BG };

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
        .hover(|style| style.bg(rgba(PICKER_BG_HOVER)))
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
                .text_color(rgb(colors::INK))
                .child(space_name),
        )
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(chevron),
        )
        .into_any_element()
}

/// Window-root popover surface for the workspace picker. Rendered by
/// render_browser when `workspace_picker_open` is true. Pinned to a
/// fixed window-relative origin matched to the picker pill's row in
/// the sidebar header, so the surface lines up with the trigger
/// without needing to capture bounds at paint time.
pub(crate) fn render_workspace_disclosure(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_id = snapshot.active_space_id.clone();

    let body = div()
        .absolute()
        .top(px(DISCLOSURE_TOP_PX))
        .left(px(DISCLOSURE_LEFT_PX))
        .w(px(DISCLOSURE_WIDTH_PX))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(4.0))
        .rounded(px(10.0))
        .bg(rgba(DISCLOSURE_BG))
        .border_1()
        .border_color(rgba(DISCLOSURE_BORDER))
        .shadow(soft_shadow())
        .children(
            snapshot
                .spaces
                .iter()
                .enumerate()
                .map(|(index, space)| {
                    render_disclosure_row(index, space, &active_id, cx)
                }),
        )
        .child(render_disclosure_footer(cx));

    fade_in("workspace-disclosure", 140, body).into_any_element()
}

/// Transparent fullscreen backdrop layered between the layout grid
/// and the disclosure. Press on it closes the picker AND consumes
/// the event so the dismiss-click doesn't ricochet into a button
/// underneath the cursor.
pub(crate) fn render_workspace_disclosure_backdrop(
    cx: &mut Context<ElyShell>,
) -> AnyElement {
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

/// Anchor of the disclosure popover relative to the window's top-left.
/// Walks the sidebar layout: SHELL_INSET (16) + sidebar header pt (8)
/// + title row h (20) + header gap (8) + picker row py-top (4) + tile
/// height (32) + picker row py-bot (4) + popover lift (6) = 98.
const DISCLOSURE_TOP_PX: f32 = 98.0;

/// Left edge of the disclosure: SHELL_INSET (16) + header px-left (10)
/// + picker row px-left (2) + tile width (32) + picker gap (6) = 66.
/// That lines the disclosure up with the picker pill, not the tile.
const DISCLOSURE_LEFT_PX: f32 = 66.0;

/// Default sidebar (280 px) → pill width 180 px. The disclosure inherits
/// that width so the popover and trigger frame to the same column. If
/// the sidebar is resized the disclosure stays a fixed width — picker
/// is closed before the user can drag, so the misalignment is
/// transient at worst.
const DISCLOSURE_WIDTH_PX: f32 = 180.0;

fn render_disclosure_row(
    index: usize,
    space: &Space,
    active_id: &ely_domain::SpaceId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_id = space.id().clone();
    let active = space.id() == active_id;
    let bg = if active { DISCLOSURE_ROW_ACTIVE_BG } else { 0x00000000 };

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
        .hover(|style| style.bg(rgba(DISCLOSURE_ROW_HOVER_BG)))
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
                .text_color(rgb(colors::INK))
                .child(space.name().to_string()),
        )
        .when(active, |el| {
            el.child(
                div()
                    .text_color(rgb(colors::ACCENT))
                    .child(IconName::Check),
            )
        })
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
        .border_color(rgba(colors::DIVIDER))
        .text_size(px(12.0))
        .text_color(rgb(colors::INK_3))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(DISCLOSURE_ROW_HOVER_BG)).text_color(rgb(colors::INK)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.close_workspace_picker(cx);
            shell.open_internal_tab("ely://settings/spaces", window, cx);
        }))
        .child(
            div()
                .text_color(rgb(colors::INK_4))
                .child(IconName::Settings),
        )
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

fn render_add_workspace_button(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("workspace-add"))
        .size(px(32.0))
        .rounded(px(9.0))
        .bg(rgba(ADD_BUTTON_BG))
        .shadow(soft_shadow())
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::INK_3))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(PICKER_BG)).text_color(rgb(colors::INK)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://settings/spaces", window, cx);
        }))
        .child(IconName::Plus)
        .into_any_element()
}

const PICKER_BG: u32 = 0xffffff99;
const PICKER_BG_HOVER: u32 = 0xffffffd9;
const ADD_BUTTON_BG: u32 = 0xffffff66;
const DISCLOSURE_BG: u32 = 0xffffffd9;
const DISCLOSURE_BORDER: u32 = 0xffffff80;
const DISCLOSURE_ROW_ACTIVE_BG: u32 = 0xffffffeb;
const DISCLOSURE_ROW_HOVER_BG: u32 = 0xffffffb3;

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
