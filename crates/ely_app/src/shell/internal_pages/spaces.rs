use ely_browser_core::BrowserSnapshot;
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
                .child(render_spaces_list(snapshot, cx)),
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

fn render_spaces_list(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(snapshot.spaces.iter().enumerate().map(|(index, space)| {
            render_space_row(index, space, snapshot, space.id() == &snapshot.active_space_id, cx)
        }))
        .into_any_element()
}

fn render_space_row(
    index: usize,
    space: &Space,
    snapshot: &BrowserSnapshot,
    active: bool,
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
        .child(render_space_action(index, space_id, active, cx))
        .into_any_element()
}

fn render_space_action(
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
