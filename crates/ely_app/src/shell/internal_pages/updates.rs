use std::env;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::UpdatePolicy;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_REVISION: &str = env!("ELY_BUILD_REVISION");
const RELEASE_MANIFEST_PATH: &str = "/api/releases/manifest";
const RELEASE_SIGNATURE_PATH: &str = "/api/releases/signature";
const RELEASE_MANIFEST_CACHE: &str = "release_manifest_cache";
const RELEASE_INTEGRITY: &str = "SHA-256 + Ed25519";

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
                .child(render_updates_header(snapshot))
                .child(render_updates_summary(snapshot.update_policy, cx))
                .child(render_update_policy_rows(snapshot.update_policy, cx))
                .child(render_update_contract_rows()),
        )
    }
}

fn render_updates_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Updates"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::muted()))
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
                .text_color(rgb(colors::muted()))
                .child(IconName::LoaderCircle)
                .child(format!("Build {BUILD_REVISION}")),
        )
        .into_any_element()
}

fn render_updates_summary(
    update_policy: UpdatePolicy,
    cx: &mut gpui::Context<ElyShell>,
) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::hairline()))
        .bg(rgb(colors::canvas_soft()))
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
                .child(div().text_color(rgb(colors::primary())).child(IconName::LoaderCircle))
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
                                .text_color(rgb(colors::ink()))
                                .child("Release Manifest Contract"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(update_policy.detail()),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::success()))
                        .child(update_policy.name()),
                )
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
                ),
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

fn render_update_contract_rows() -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .child(update_row(IconName::Info, "Current Version", APP_VERSION, "Cargo package version"))
        .child(update_row(IconName::GitHub, "Build Revision", BUILD_REVISION, "Git revision"))
        .child(update_row(
            IconName::Globe,
            "Release Target",
            release_target(),
            "Platform and architecture",
        ))
        .child(update_row(
            IconName::File,
            "Manifest API",
            RELEASE_MANIFEST_PATH,
            format!("KV namespace: {RELEASE_MANIFEST_CACHE}"),
        ))
        .child(update_row(
            IconName::File,
            "Signature API",
            signature_query_path(),
            "Targeted release signature document",
        ))
        .child(update_row(
            IconName::CircleCheck,
            "Artifact Integrity",
            RELEASE_INTEGRITY,
            "Release manifest requires package hash and signature",
        ))
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

fn update_row(
    icon: IconName,
    label: &'static str,
    value: impl Into<String>,
    detail: impl Into<String>,
) -> AnyElement {
    let value = value.into();
    let detail = detail.into();

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
                .child(div().text_color(rgb(colors::muted_soft())).child(icon))
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
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(detail),
                        ),
                ),
        )
        .child(
            div()
                .max_w(px(360.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child(value),
        )
        .into_any_element()
}

fn release_target() -> String {
    format!("{} / {}", env::consts::OS, env::consts::ARCH)
}

fn signature_query_path() -> String {
    format!(
        "{RELEASE_SIGNATURE_PATH}?platform={}&architecture={}&version={APP_VERSION}",
        env::consts::OS,
        env::consts::ARCH
    )
}
