use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::Space;
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, point, px,
    rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;

pub(crate) fn render_sidebar_header(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_space = snapshot
        .spaces
        .iter()
        .find(|space| space.id() == &snapshot.active_space_id);

    div()
        .flex()
        .flex_col()
        .pt(px(36.0))
        .px(px(10.0))
        .pb(px(10.0))
        .gap(px(8.0))
        .flex_shrink_0()
        .child(render_title_row())
        .child(render_workspace_picker(active_space, cx))
        .into_any_element()
}

fn render_title_row() -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .pl(px(6.0))
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

fn render_workspace_picker(
    active_space: Option<&Space>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .py(px(4.0))
        .px(px(2.0))
        .child(render_workspaces_tile(cx))
        .child(render_picker_pill(active_space, cx))
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

fn render_picker_pill(active_space: Option<&Space>, cx: &mut Context<ElyShell>) -> AnyElement {
    let space_name = active_space
        .map(|space| space.name().to_string())
        .unwrap_or_default();
    let space_glyph = active_space
        .map(|space| space.icon().to_string())
        .unwrap_or_default();

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
        .bg(rgba(PICKER_BG))
        .shadow(soft_shadow())
        .cursor_pointer()
        .hover(|style| style.bg(rgba(PICKER_BG_HOVER)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.cycle_to_next_space(window, cx);
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
                .child(IconName::ChevronDown),
        )
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
