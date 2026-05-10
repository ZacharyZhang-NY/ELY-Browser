use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    COLLAPSED_SIDEBAR_WIDTH_PX, DEFAULT_SIDEBAR_WIDTH_PX, HIDDEN_SIDEBAR_WIDTH_PX,
};
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
    let mode = LayoutMode::from_width(active_width);

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
                .grid_cols(3)
                .gap(px(10.0))
                .child(render_layout_card(
                    "Single column",
                    "Default · workspace + tabs.",
                    mode == LayoutMode::Single,
                    LayoutMode::Single,
                    cx,
                ))
                .child(render_layout_card(
                    "Compact",
                    "Icons-only with launcher rail.",
                    mode == LayoutMode::Compact,
                    LayoutMode::Compact,
                    cx,
                ))
                .child(render_layout_card(
                    "Hidden on hover",
                    "Slide in on cursor reach.",
                    mode == LayoutMode::Hidden,
                    LayoutMode::Hidden,
                    cx,
                )),
        )
        .into_any_element()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LayoutMode {
    Single,
    Compact,
    Hidden,
}

impl LayoutMode {
    fn from_width(width: u16) -> Self {
        if width <= HIDDEN_SIDEBAR_WIDTH_PX {
            Self::Hidden
        } else if width <= COLLAPSED_SIDEBAR_WIDTH_PX {
            Self::Compact
        } else {
            Self::Single
        }
    }

    fn width(self) -> u16 {
        match self {
            Self::Single => DEFAULT_SIDEBAR_WIDTH_PX,
            Self::Compact => COLLAPSED_SIDEBAR_WIDTH_PX,
            Self::Hidden => HIDDEN_SIDEBAR_WIDTH_PX,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Single => "layout-single",
            Self::Compact => "layout-compact",
            Self::Hidden => "layout-hidden",
        }
    }

    fn preview_sidebar(self) -> f32 {
        match self {
            Self::Single => 56.0,
            Self::Compact => 18.0,
            Self::Hidden => 6.0,
        }
    }
}

fn render_layout_card(
    title: &'static str,
    detail: &'static str,
    selected: bool,
    mode: LayoutMode,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .id(SharedString::from(mode.id()))
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
            shell.set_active_sidebar_width(mode.width(), cx);
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
    let sidebar_width = mode.preview_sidebar();

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
