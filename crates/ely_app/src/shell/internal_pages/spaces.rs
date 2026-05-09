use std::time::{Duration, SystemTime};

use ely_browser_core::{BrowserSnapshot, SpaceImportProfileMapping, TrashedSpace};
use ely_design_system::colors;
use ely_domain::{ArchivePolicy, Profile, Space, SpaceId};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface, space_actions::render_space_actions};

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
                .child(render_spaces_header(snapshot, cx))
                .child(render_space_file_message(
                    self.space_file_notice.as_deref(),
                    self.space_file_error.as_deref(),
                ))
                .child(render_active_space_summary(snapshot))
                .child(render_spaces_list(snapshot, self.pending_space_trash.as_ref(), cx))
                .child(render_trashed_spaces_list(snapshot, cx)),
        )
    }
}

fn render_spaces_header(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
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
                .child(
                    Button::new("import-space-active-profile")
                        .small()
                        .primary()
                        .icon(IconName::FolderOpen)
                        .label("Import")
                        .tooltip("Import .elyspace to Active Profile")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.choose_space_import(
                                SpaceImportProfileMapping::UseActiveProfile,
                                window,
                                cx,
                            );
                        })),
                )
                .child(
                    Button::new("import-space-preserve-profiles")
                        .small()
                        .ghost()
                        .icon(IconName::User)
                        .label("Import Profiles")
                        .tooltip("Import .elyspace with Existing Profiles")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.choose_space_import(
                                SpaceImportProfileMapping::PreserveExisting,
                                window,
                                cx,
                            );
                        })),
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
                ),
        )
        .into_any_element()
}

fn render_space_file_message(notice: Option<&str>, error: Option<&str>) -> AnyElement {
    let (message, color, icon) = if let Some(error) = error {
        (error, colors::ERROR, IconName::TriangleAlert)
    } else if let Some(notice) = notice {
        (notice, colors::SUCCESS, IconName::CircleCheck)
    } else {
        return div().into_any_element();
    };

    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(color))
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .text_color(rgb(color))
        .child(icon)
        .child(message.to_string())
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

fn render_trashed_spaces_list(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.trashed_spaces.is_empty() {
        return div().into_any_element();
    }

    let now = SystemTime::now();
    let expired_count =
        snapshot.trashed_spaces.iter().filter(|space| space.is_expired(now)).count();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .pt_3()
        .child(render_trashed_spaces_header(expired_count, cx))
        .children(
            snapshot.trashed_spaces.iter().enumerate().map(|(index, trashed_space)| {
                render_trashed_space_row(index, trashed_space, now, cx)
            }),
        )
        .into_any_element()
}

fn render_trashed_spaces_header(expired_count: usize, cx: &mut Context<ElyShell>) -> AnyElement {
    let action = if expired_count == 0 {
        div()
            .text_xs()
            .font_semibold()
            .text_color(rgb(colors::MUTED_SOFT))
            .child("30 day retention")
            .into_any_element()
    } else {
        Button::new("purge-expired-spaces")
            .small()
            .danger()
            .icon(IconName::Delete)
            .label("Purge Expired")
            .tooltip("Purge Expired Spaces")
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.purge_expired_trashed_spaces(cx);
            }))
            .into_any_element()
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child("Recently Trashed"),
        )
        .child(action)
        .into_any_element()
}

fn render_trashed_space_row(
    index: usize,
    trashed_space: &TrashedSpace,
    now: SystemTime,
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
                                .child(trashed_space_detail_label(trashed_space, now)),
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

fn trashed_space_detail_label(trashed_space: &TrashedSpace, now: SystemTime) -> String {
    format!(
        "{} open tabs - {} archived tabs - {}",
        trashed_space.tabs().len(),
        trashed_space.archived_tabs().len(),
        trash_retention_label(trashed_space, now)
    )
}

fn trash_retention_label(trashed_space: &TrashedSpace, now: SystemTime) -> String {
    if trashed_space.is_expired(now) {
        return "eligible for permanent purge".to_string();
    }

    format!("purges in {}", time_until_purge(trashed_space.purge_at(), now))
}

fn time_until_purge(purge_at: SystemTime, now: SystemTime) -> String {
    let remaining = purge_at.duration_since(now).unwrap_or(Duration::ZERO);
    if remaining < Duration::from_secs(3_600) {
        return "under 1 hour".to_string();
    }

    let days = remaining.as_secs().div_ceil(86_400);
    if days == 1 { "1 day".to_string() } else { format!("{days} days") }
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
