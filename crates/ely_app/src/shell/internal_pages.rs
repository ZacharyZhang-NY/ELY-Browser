use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{BrowserTab, HistoryEntry};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::ElyShell;

impl ElyShell {
    pub(super) fn render_web_canvas(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match tab.url().as_str() {
            "ely://history" => self.render_history_page(snapshot, cx),
            _ => render_default_page(tab),
        }
    }

    fn render_history_page(
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
                .child(
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(26.0))
                                        .text_color(rgb(colors::INK))
                                        .child("History"),
                                )
                                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(
                                    format!(
                                        "{} / {}",
                                        snapshot.active_profile_name, snapshot.active_space_name
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(format!("{} entries", snapshot.history_entries.len())),
                        ),
                )
                .child(self.render_history_list(snapshot, cx)),
        )
    }

    fn render_history_list(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.history_entries.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("No history entries for this Space and Profile.")
                .into_any_element();
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .children(
                snapshot
                    .history_entries
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, entry)| self.render_history_row(index, entry, cx)),
            )
            .into_any_element()
    }

    fn render_history_row(
        &mut self,
        index: usize,
        entry: &HistoryEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = entry.url().clone();

        div()
            .id(SharedString::from(format!("history-{index}")))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.open_url(url.clone(), window, cx);
            }))
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
                            .child(entry.title().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(colors::MUTED))
                            .child(entry.url().display_url()),
                    ),
            )
            .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::ExternalLink))
            .into_any_element()
    }
}

fn render_default_page(tab: &BrowserTab) -> AnyElement {
    render_canvas_surface(
        div()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(26.0))
                    .text_color(rgb(colors::INK))
                    .child(tab.title().to_string()),
            )
            .child(div().text_sm().text_color(rgb(colors::MUTED)).child(render_tab_status(tab))),
    )
}

fn render_canvas_surface(content: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .p(px(spacing::LG))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .size_full()
                .rounded_lg()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .bg(rgb(colors::SURFACE_CARD))
                .child(content),
        )
        .into_any_element()
}

fn render_tab_status(tab: &BrowserTab) -> String {
    match tab.url().as_str() {
        "ely://new-tab" => "Ready".to_string(),
        url => url.to_string(),
    }
}
