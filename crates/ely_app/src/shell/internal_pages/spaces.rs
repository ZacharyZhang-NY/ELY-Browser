use ely_browser_core::{BrowserSnapshot, TrashedSpace};
use ely_design_system::colors;
use ely_domain::{ArchivePolicy, Profile, Space, SpaceId};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_spaces_page(
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
                .child(render_spaces_header(snapshot))
                .child(render_active_space_summary(snapshot))
                .child(render_spaces_list(snapshot, self.pending_space_trash.as_ref(), cx))
                .child(render_trashed_spaces_list(snapshot, cx)),
        )
    }
}

fn render_spaces_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Spaces"))
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
                .child(IconName::GalleryVerticalEnd)
                .child(format!("{} spaces", snapshot.spaces.len())),
        )
        .into_any_element()
}

fn render_active_space_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    let Some(active_space) =
        snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
    else {
        return div()
            .rounded_md()
            .border_1()
            .border_color(rgb(colors::ERROR))
            .px_4()
            .py_3()
            .text_sm()
            .text_color(rgb(colors::ERROR))
            .child("Active Space is unavailable.")
            .into_any_element();
    };

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
            div().min_w_0().flex().items_center().gap_3().child(space_avatar(active_space)).child(
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
                            .child(active_space.name().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(colors::MUTED))
                            .child(space_detail_label(active_space, &snapshot.profiles)),
                    ),
            ),
        )
        .child(div().text_xs().font_semibold().text_color(rgb(colors::SUCCESS)).child("Active"))
        .into_any_element()
}

fn render_spaces_list(
    snapshot: &BrowserSnapshot,
    pending_space_trash: Option<&SpaceId>,
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
        .children(snapshot.spaces.iter().enumerate().map(|(index, space)| {
            render_space_row(
                index,
                snapshot.spaces.len(),
                space,
                snapshot,
                space.id() == &snapshot.active_space_id,
                pending_space_trash == Some(space.id()),
                cx,
            )
        }))
        .into_any_element()
}

fn render_space_row(
    index: usize,
    space_count: usize,
    space: &Space,
    snapshot: &BrowserSnapshot,
    active: bool,
    confirming_trash: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_id = space.id().clone();

    div()
        .id(SharedString::from(format!("settings-space-{}", space.id().as_str())))
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div().min_w_0().flex().items_center().gap_3().child(space_avatar(space)).child(
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
                            .child(space.name().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(colors::MUTED))
                            .child(space_detail_label(space, &snapshot.profiles)),
                    ),
            ),
        )
        .child(render_space_actions(index, space_count, space_id, active, confirming_trash, cx))
        .into_any_element()
}

fn render_space_actions(
    index: usize,
    space_count: usize,
    space_id: SpaceId,
    active: bool,
    confirming_trash: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if confirming_trash {
        return render_trash_confirmation(cx);
    }

    let can_move_up = index > 0;
    let can_move_down = index + 1 < space_count;

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(render_space_order_button(
            ("move-space-up", index),
            space_id.clone(),
            IconName::ArrowUp,
            "Move Space Up",
            can_move_up,
            true,
            cx,
        ))
        .child(render_space_order_button(
            ("move-space-down", index),
            space_id.clone(),
            IconName::ArrowDown,
            "Move Space Down",
            can_move_down,
            false,
            cx,
        ))
        .child(render_space_switch_action(index, space_id.clone(), active, cx))
        .child(render_request_trash_button(index, space_id, space_count, cx))
        .into_any_element()
}

fn render_space_order_button(
    id: (&'static str, usize),
    space_id: SpaceId,
    icon: IconName,
    tooltip: &'static str,
    enabled: bool,
    moves_up: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(id)
        .small()
        .ghost()
        .icon(icon)
        .tooltip(tooltip)
        .disabled(!enabled)
        .on_click(cx.listener(move |shell, _, _, cx| {
            if moves_up {
                shell.move_space_up(&space_id, cx);
            } else {
                shell.move_space_down(&space_id, cx);
            }
        }))
        .into_any_element()
}

fn render_request_trash_button(
    index: usize,
    space_id: SpaceId,
    space_count: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("trash-space", index))
        .small()
        .ghost()
        .icon(IconName::Delete)
        .tooltip("Move Space to Trash")
        .disabled(space_count <= 1)
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.request_space_trash(space_id.clone(), cx);
        }))
        .into_any_element()
}

fn render_trash_confirmation(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(Button::new("cancel-space-trash").small().ghost().label("Cancel").on_click(
            cx.listener(|shell, _, _, cx| {
                shell.cancel_space_trash(cx);
            }),
        ))
        .child(
            Button::new("confirm-space-trash")
                .small()
                .danger()
                .icon(IconName::Delete)
                .label("Trash")
                .tooltip("Move Space to Trash")
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.trash_pending_space(window, cx);
                })),
        )
        .into_any_element()
}

fn render_space_switch_action(
    index: usize,
    space_id: SpaceId,
    active: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if active {
        return div()
            .text_xs()
            .font_semibold()
            .text_color(rgb(colors::SUCCESS))
            .child("Active")
            .into_any_element();
    }

    Button::new(("switch-space", index))
        .small()
        .primary()
        .icon(IconName::Check)
        .label("Switch")
        .tooltip("Switch Space")
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_space(&space_id, window, cx);
        }))
        .into_any_element()
}

fn render_trashed_spaces_list(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.trashed_spaces.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_2()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .pt_3()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child("Recently Trashed"),
        )
        .children(
            snapshot
                .trashed_spaces
                .iter()
                .enumerate()
                .map(|(index, trashed_space)| render_trashed_space_row(index, trashed_space, cx)),
        )
        .into_any_element()
}

fn render_trashed_space_row(
    index: usize,
    trashed_space: &TrashedSpace,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let space_id = trashed_space.space().id().clone();
    div()
        .py_2()
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
                .child(space_avatar(trashed_space.space()))
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
                                .child(trashed_space.space().name().to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(trashed_space_detail_label(trashed_space)),
                        ),
                ),
        )
        .child(
            Button::new(("restore-space", index))
                .small()
                .primary()
                .icon(IconName::Undo)
                .label("Restore")
                .tooltip("Restore Space")
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.restore_trashed_space(&space_id, window, cx);
                })),
        )
        .into_any_element()
}

fn space_avatar(space: &Space) -> AnyElement {
    div()
        .w(px(28.0))
        .h(px(28.0))
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE_STRONG))
        .bg(rgb(space.accent_hex()))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_semibold()
        .text_color(rgb(colors::CANVAS))
        .child(space.icon().to_string())
        .into_any_element()
}

fn trashed_space_detail_label(trashed_space: &TrashedSpace) -> String {
    format!(
        "{} open tabs - {} archived tabs - retained 30 days",
        trashed_space.tabs().len(),
        trashed_space.archived_tabs().len()
    )
}

fn space_detail_label(space: &Space, profiles: &[Profile]) -> String {
    format!(
        "{} - {} - {}",
        accent_label(space.accent_hex()),
        archive_policy_label(space.archive_policy()),
        default_profile_label(space, profiles)
    )
}

fn default_profile_label(space: &Space, profiles: &[Profile]) -> String {
    let Some(profile) = profiles.iter().find(|profile| profile.id() == space.default_profile_id())
    else {
        return "Default profile unavailable".to_string();
    };

    format!("Default profile: {}", profile.name())
}

fn accent_label(accent_hex: u32) -> String {
    format!("#{accent_hex:06X}")
}

fn archive_policy_label(policy: &ArchivePolicy) -> &'static str {
    match policy {
        ArchivePolicy::Manual => "Manual archive",
        ArchivePolicy::IdleDays(0) => "Archive today",
        ArchivePolicy::IdleDays(1) => "Archive after 1 day",
        ArchivePolicy::IdleDays(7) => "Archive after 7 days",
        ArchivePolicy::IdleDays(30) => "Archive after 30 days",
        ArchivePolicy::IdleDays(_) => "Custom archive policy",
    }
}
