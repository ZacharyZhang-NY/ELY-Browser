use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::SearchEngine;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_search_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_search_header(snapshot))
                .child(render_search_summary(snapshot.search_engine))
                .child(render_search_engines(snapshot.search_engine, cx)),
        )
    }
}

fn render_search_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Search"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(IconName::Search)
                .child(snapshot.search_engine.name()),
        )
        .into_any_element()
}

fn render_search_summary(search_engine: SearchEngine) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(colors::PRIMARY)).child(IconName::Search))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(rgb(colors::INK))
                                .child(search_engine.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(search_engine.host()),
                        ),
                ),
        )
        .child(
            div().text_xs().font_semibold().text_color(rgb(colors::SUCCESS)).child("Saved locally"),
        )
        .into_any_element()
}

fn render_search_engines(active_engine: SearchEngine, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(SearchEngine::ALL.iter().copied().enumerate().map(|(index, search_engine)| {
            render_search_engine_row(index, search_engine, active_engine, cx)
        }))
        .into_any_element()
}

fn render_search_engine_row(
    index: usize,
    search_engine: SearchEngine,
    active_engine: SearchEngine,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = search_engine == active_engine;

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(rgb(search_engine_icon_color(selected)))
                        .child(search_engine_icon(selected)),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .text_color(rgb(colors::INK))
                                .child(search_engine.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(search_engine.host()),
                        ),
                ),
        )
        .child(
            Button::new(("search-engine", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label(search_engine_button_label(selected))
                .tooltip(search_engine.name())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_search_engine(search_engine, cx);
                })),
        )
        .into_any_element()
}

fn search_engine_icon(selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { IconName::Globe }
}

fn search_engine_icon_color(selected: bool) -> u32 {
    if selected { colors::PRIMARY } else { colors::MUTED_SOFT }
}

fn search_engine_button_label(selected: bool) -> &'static str {
    if selected { "Active" } else { "Select" }
}
