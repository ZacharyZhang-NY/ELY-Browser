use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::HistoryRecordingPolicy;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_privacy_security_page(
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
                .child(render_privacy_header(snapshot))
                .child(render_history_summary(snapshot))
                .child(render_history_policy_rows(snapshot.history_recording_policy, cx)),
        )
    }
}

fn render_privacy_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::INK))
                        .child("Privacy & Security"),
                )
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
                .child(privacy_icon(snapshot.history_recording_policy))
                .child(snapshot.history_recording_policy.status()),
        )
        .into_any_element()
}

fn render_history_summary(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(
                    div()
                        .text_color(rgb(policy_color(snapshot.history_recording_policy)))
                        .child(privacy_icon(snapshot.history_recording_policy)),
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
                                .text_color(rgb(colors::INK))
                                .child(snapshot.history_recording_policy.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(snapshot.history_recording_policy.detail()),
                        ),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} visible entries", snapshot.history_entries.len())),
        )
        .into_any_element()
}

fn render_history_policy_rows(
    active_policy: HistoryRecordingPolicy,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(
            HistoryRecordingPolicy::ALL
                .iter()
                .copied()
                .enumerate()
                .map(|(index, policy)| render_history_policy_row(index, policy, active_policy, cx)),
        )
        .into_any_element()
}

fn render_history_policy_row(
    index: usize,
    policy: HistoryRecordingPolicy,
    active_policy: HistoryRecordingPolicy,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = policy == active_policy;

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
                        .text_color(rgb(history_policy_icon_color(policy, selected)))
                        .child(history_policy_icon(policy, selected)),
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
                                .child(policy.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(policy.detail()),
                        ),
                ),
        )
        .child(
            Button::new(("history-recording-policy", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label(history_policy_button_label(policy, selected))
                .tooltip(policy.name())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_history_recording_policy(policy, cx);
                })),
        )
        .into_any_element()
}

fn privacy_icon(policy: HistoryRecordingPolicy) -> IconName {
    match policy {
        HistoryRecordingPolicy::Record => IconName::Eye,
        HistoryRecordingPolicy::Pause => IconName::EyeOff,
    }
}

fn policy_color(policy: HistoryRecordingPolicy) -> u32 {
    match policy {
        HistoryRecordingPolicy::Record => colors::SUCCESS,
        HistoryRecordingPolicy::Pause => colors::PRIMARY,
    }
}

fn history_policy_icon(policy: HistoryRecordingPolicy, selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { privacy_icon(policy) }
}

fn history_policy_icon_color(policy: HistoryRecordingPolicy, selected: bool) -> u32 {
    if selected { policy_color(policy) } else { colors::MUTED_SOFT }
}

fn history_policy_button_label(policy: HistoryRecordingPolicy, selected: bool) -> &'static str {
    match (policy, selected) {
        (HistoryRecordingPolicy::Record, true) => "Default",
        (_, true) => "Active",
        _ => "Select",
    }
}
