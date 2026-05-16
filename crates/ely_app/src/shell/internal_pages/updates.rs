use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::UpdatePolicy;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_updates_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_updates_header(cx))
                .child(render_update_policy_rows(snapshot.update_policy, cx)),
        )
    }
}

fn render_updates_header(cx: &mut gpui::Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Updates"))
        .child(
            Button::new("reset-update-settings")
                .ghost()
                .xsmall()
                .icon(IconName::Undo2)
                .label("Reset")
                .tooltip("Restore Update Defaults")
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.reset_update_settings(cx);
                })),
        )
        .into_any_element()
}

fn render_update_policy_rows(
    active_policy: UpdatePolicy,
    cx: &mut gpui::Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .children(
            UpdatePolicy::ALL
                .iter()
                .copied()
                .enumerate()
                .map(|(index, policy)| render_update_policy_row(index, policy, active_policy, cx)),
        )
        .into_any_element()
}

fn render_update_policy_row(
    index: usize,
    policy: UpdatePolicy,
    active_policy: UpdatePolicy,
    cx: &mut gpui::Context<ElyShell>,
) -> AnyElement {
    let selected = policy == active_policy;

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::hairline()))
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
                    div().text_color(rgb(policy_icon_color(selected))).child(policy_icon(selected)),
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
                                .text_color(rgb(colors::ink()))
                                .child(policy.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(policy.detail()),
                        ),
                ),
        )
        .child(
            Button::new(("update-policy", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label(policy_button_label(selected))
                .tooltip(policy.name())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_update_policy(policy, cx);
                })),
        )
        .into_any_element()
}

fn policy_icon(selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { IconName::LoaderCircle }
}

fn policy_icon_color(selected: bool) -> u32 {
    if selected { colors::primary() } else { colors::muted_soft() }
}

fn policy_button_label(selected: bool) -> &'static str {
    if selected { "Active" } else { "Select" }
}
