use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ArchivePolicy, FavoriteLimit, Space};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

struct ArchivePolicyOption {
    label: &'static str,
    detail: &'static str,
    policy: ArchivePolicy,
}

const ARCHIVE_POLICY_OPTIONS: &[ArchivePolicyOption] = &[
    ArchivePolicyOption {
        label: "Manual",
        detail: "Keep unpinned tabs open until you close them.",
        policy: ArchivePolicy::Manual,
    },
    ArchivePolicyOption {
        label: "Today",
        detail: "Archive idle unpinned tabs during daily cleanup.",
        policy: ArchivePolicy::IdleDays(0),
    },
    ArchivePolicyOption {
        label: "7 days",
        detail: "Archive unpinned tabs after a week away.",
        policy: ArchivePolicy::IdleDays(7),
    },
    ArchivePolicyOption {
        label: "30 days",
        detail: "Archive unpinned tabs after a month away.",
        policy: ArchivePolicy::IdleDays(30),
    },
];

impl ElyShell {
    pub(super) fn render_sidebar_tabs_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(active_space) =
            snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
        else {
            return render_canvas_surface(
                div()
                    .size_full()
                    .p_8()
                    .text_color(rgb(colors::ERROR))
                    .child("Active Space is unavailable."),
            );
        };

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_sidebar_tabs_header(snapshot, active_space))
                .child(render_sidebar_tabs_settings(snapshot, active_space, cx)),
        )
    }
}

fn render_sidebar_tabs_header(snapshot: &BrowserSnapshot, active_space: &Space) -> AnyElement {
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
                    div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Sidebar & Tabs"),
                )
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Space: {}", snapshot.active_space_name)),
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
                .child(IconName::LayoutDashboard)
                .child(format!(
                    "{} / {}",
                    archive_policy_label(active_space.archive_policy()),
                    snapshot.favorite_limit.label()
                )),
        )
        .into_any_element()
}

fn render_sidebar_tabs_settings(
    snapshot: &BrowserSnapshot,
    active_space: &Space,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .pt_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(render_archive_policy_section(active_space, cx))
        .child(render_favorite_limit_section(snapshot.favorite_limit, cx))
        .into_any_element()
}

fn render_archive_policy_section(active_space: &Space, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
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
                                .child("Auto Archive"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child("Current Space policy for idle unpinned tabs."),
                        ),
                )
                .child(
                    Button::new("archive-idle-tabs-now")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Inbox)
                        .label("Run Now")
                        .tooltip("Archive idle tabs now")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.archive_idle_tabs_now(cx);
                        })),
                ),
        )
        .children(
            ARCHIVE_POLICY_OPTIONS.iter().enumerate().map(|(index, option)| {
                render_archive_policy_option(index, option, active_space, cx)
            }),
        )
        .into_any_element()
}

fn render_favorite_limit_section(
    favorite_limit: FavoriteLimit,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .pt_4()
        .child(
            div()
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
                                .child("Favorite Limit"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child("Maximum cross-space Favorites visible in the sidebar."),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::MUTED))
                        .child(favorite_limit.label()),
                ),
        )
        .children(
            FavoriteLimit::ALL.iter().copied().enumerate().map(|(index, limit)| {
                render_favorite_limit_option(index, limit, favorite_limit, cx)
            }),
        )
        .into_any_element()
}

fn render_archive_policy_option(
    index: usize,
    option: &'static ArchivePolicyOption,
    active_space: &Space,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = active_space.archive_policy() == &option.policy;
    let policy = option.policy.clone();
    let border = if selected { colors::PRIMARY } else { colors::HAIRLINE };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
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
                                .text_color(rgb(colors::INK))
                                .child(option.label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(option.detail),
                        ),
                ),
        )
        .child(
            Button::new(("archive-policy-option", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label("Select")
                .tooltip(option.label)
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_active_space_archive_policy(policy.clone(), cx);
                })),
        )
        .into_any_element()
}

fn render_favorite_limit_option(
    index: usize,
    limit: FavoriteLimit,
    active_limit: FavoriteLimit,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = limit == active_limit;
    let border = if selected { colors::PRIMARY } else { colors::HAIRLINE };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
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
                                .text_color(rgb(colors::INK))
                                .child(limit.label()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(limit.detail()),
                        ),
                ),
        )
        .child(
            Button::new(("favorite-limit-option", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label("Select")
                .tooltip(limit.label())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_favorite_limit(limit, cx);
                })),
        )
        .into_any_element()
}

fn archive_policy_label(policy: &ArchivePolicy) -> &'static str {
    match policy {
        ArchivePolicy::Manual => "Manual",
        ArchivePolicy::IdleDays(0) => "Today",
        ArchivePolicy::IdleDays(7) => "7 days",
        ArchivePolicy::IdleDays(30) => "30 days",
        ArchivePolicy::IdleDays(_) => "Custom",
    }
}

fn policy_icon(selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { IconName::Inbox }
}

fn policy_icon_color(selected: bool) -> u32 {
    if selected { colors::PRIMARY } else { colors::MUTED }
}
