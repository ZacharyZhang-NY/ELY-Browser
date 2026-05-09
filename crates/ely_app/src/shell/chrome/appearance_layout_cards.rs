use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{COLLAPSED_SIDEBAR_WIDTH_PX, DEFAULT_SIDEBAR_WIDTH_PX};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb, rgba,
};

use crate::shell::ElyShell;
use crate::shell::chrome::SERIF_FAMILY;

pub(crate) fn render_sidebar_layout_section(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active_width = snapshot
        .spaces
        .iter()
        .find(|space| space.id() == &snapshot.active_space_id)
        .map(|space| space.sidebar_width_px())
        .unwrap_or(DEFAULT_SIDEBAR_WIDTH_PX);
    let compact = active_width <= COLLAPSED_SIDEBAR_WIDTH_PX;

    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .pt(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_4))
                .child("SIDEBAR"),
        )
        .child(
            div()
                .font_family(SERIF_FAMILY)
                .text_size(px(22.0))
                .font_weight(FontWeight(400.0))
                .text_color(rgb(colors::INK))
                .child("Layout"),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(px(10.0))
                .child(render_layout_card(
                    "Single column",
                    "Default · workspace + tabs.",
                    !compact,
                    LayoutMode::Single,
                    cx,
                ))
                .child(render_layout_card(
                    "Compact",
                    "Icons-only with launcher rail.",
                    compact,
                    LayoutMode::Compact,
                    cx,
                )),
        )
        .into_any_element()
}

#[derive(Clone, Copy)]
enum LayoutMode {
    Single,
    Compact,
}

fn render_layout_card(
    title: &'static str,
    detail: &'static str,
    selected: bool,
    mode: LayoutMode,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let id = SharedString::from(match mode {
        LayoutMode::Single => "layout-single",
        LayoutMode::Compact => "layout-compact",
    });

    div()
        .id(id)
        .p(px(14.0))
        .rounded(px(14.0))
        .bg(rgba(LAYOUT_CARD_BG))
        .when(selected, |el| el.border_2().border_color(rgb(colors::ACCENT)))
        .when(!selected, |el| el.border_1().border_color(rgba(colors::STROKE)))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.94))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, _, cx| {
            let width = match mode {
                LayoutMode::Single => DEFAULT_SIDEBAR_WIDTH_PX,
                LayoutMode::Compact => COLLAPSED_SIDEBAR_WIDTH_PX,
            };
            shell.set_active_sidebar_width(width, cx);
        }))
        .child(render_layout_preview(mode))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::INK))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::INK_4))
                        .child(detail),
                ),
        )
        .into_any_element()
}

fn render_layout_preview(mode: LayoutMode) -> AnyElement {
    let sidebar_width = match mode {
        LayoutMode::Single => 56.0,
        LayoutMode::Compact => 18.0,
    };

    div()
        .h(px(96.0))
        .rounded(px(8.0))
        .bg(rgb(0xffffff))
        .relative()
        .child(
            div()
                .absolute()
                .left(px(6.0))
                .top(px(6.0))
                .bottom(px(6.0))
                .w(px(sidebar_width))
                .rounded(px(5.0))
                .bg(rgba(LAYOUT_PANEL_BG)),
        )
        .child(
            div()
                .absolute()
                .left(px(sidebar_width + 12.0))
                .top(px(6.0))
                .right(px(6.0))
                .bottom(px(6.0))
                .rounded(px(5.0))
                .bg(rgba(LAYOUT_CANVAS_BG)),
        )
        .into_any_element()
}

const LAYOUT_CARD_BG: u32 = 0xffffffd9;
const LAYOUT_PANEL_BG: u32 = 0x281e1414;
const LAYOUT_CANVAS_BG: u32 = 0x281e140a;
