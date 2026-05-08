use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::HistoryEntry;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui_component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::super::{PendingHistoryDomainClear, PendingHistoryTimeClear};
use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_history_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pending_domain_clear = self.pending_history_domain_clear.clone().filter(|pending| {
            pending.matches_context(&snapshot.active_profile_id, &snapshot.active_space_id)
        });
        let pending_time_clear = self.pending_history_time_clear.clone().filter(|pending| {
            pending.matches_context(&snapshot.active_profile_id, &snapshot.active_space_id)
        });

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_history_header(snapshot, cx))
                .when_some(pending_time_clear, |this, pending| {
                    this.child(render_time_clear_confirmation(&pending, cx))
                })
                .when_some(pending_domain_clear, |this, pending| {
                    this.child(render_domain_clear_confirmation(&pending, cx))
                })
                .child(render_history_list(snapshot, cx)),
        )
    }
}

fn render_history_header(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("History"))
                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(format!(
                    "{} / {}",
                    snapshot.active_profile_name, snapshot.active_space_name
                ))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("{} entries", snapshot.history_entries.len())),
                )
                .when(!snapshot.history_entries.is_empty(), |this| {
                    this.child(
                        Button::new("request-clear-recent-history")
                            .danger()
                            .xsmall()
                            .label("Clear Last Hour")
                            .tooltip("Clear Recent History")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.request_clear_recent_history_confirmation(cx);
                            })),
                    )
                }),
        )
        .into_any_element()
}

fn render_time_clear_confirmation(
    pending: &PendingHistoryTimeClear,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
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
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(rgb(colors::INK))
                        .child(format!("Confirm clearing {}", pending.label())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child("This removes recent history in the current Space."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("cancel-clear-recent-history")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.cancel_clear_recent_history_confirmation(cx);
                        })),
                )
                .child(
                    Button::new("confirm-clear-recent-history")
                        .danger()
                        .xsmall()
                        .label("Clear")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.clear_active_space_history_for_pending_time(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_domain_clear_confirmation(
    pending: &PendingHistoryDomainClear,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
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
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(rgb(colors::INK))
                        .child(format!("Confirm clearing {}", pending.host())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child("This removes matching history in the current Space."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("cancel-clear-history-domain")
                        .ghost()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.cancel_clear_history_domain_confirmation(cx);
                        })),
                )
                .child(
                    Button::new("confirm-clear-history-domain")
                        .danger()
                        .xsmall()
                        .label("Clear")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.clear_active_space_history_for_pending_domain(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_history_list(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.history_entries.is_empty() {
        return div()
            .flex_1()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .pt_5()
            .text_sm()
            .text_color(rgb(colors::MUTED))
            .child("History is empty for this Space and Profile.")
            .into_any_element();
    }

    div()
        .flex_1()
        .min_h_0()
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
                .map(|(index, entry)| render_history_row(index, entry, cx)),
        )
        .into_any_element()
}

fn render_history_row(
    index: usize,
    entry: &HistoryEntry,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let url = entry.url().clone();
    let host = entry.url().host();

    div()
        .id(SharedString::from(format!("history-{index}")))
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
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new(("open-history-entry", index))
                        .ghost()
                        .xsmall()
                        .label("Open")
                        .tooltip("Open History Entry")
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.open_url(url.clone(), window, cx);
                        })),
                )
                .when_some(host, |this, host| {
                    this.child(
                        Button::new(("clear-history-domain", index))
                            .danger()
                            .xsmall()
                            .label("Clear Domain")
                            .tooltip("Clear Domain History")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.request_clear_history_domain_confirmation(host.clone(), cx);
                            })),
                    )
                }),
        )
        .into_any_element()
}
