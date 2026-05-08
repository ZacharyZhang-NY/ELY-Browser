use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    DownloadDestination, DownloadPolicy, Profile, ProfileId, ProfileKind, ProfileSyncPolicy,
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_profiles_page(
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
                .child(render_profiles_header(snapshot))
                .child(render_profile_list(snapshot, cx)),
        )
    }
}

fn render_profiles_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Profiles"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Active Space: {}", snapshot.active_space_name)),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} profiles", snapshot.profiles.len())),
        )
        .into_any_element()
}

fn render_profile_list(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(snapshot.profiles.iter().enumerate().map(|(index, profile)| {
            render_profile_row(index, profile, profile.id() == &snapshot.active_profile_id, cx)
        }))
        .into_any_element()
}

fn render_profile_row(
    index: usize,
    profile: &Profile,
    active: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let profile_id = profile.id().clone();
    let sync_profile_id = profile.id().clone();
    let sync_policy = profile.sync_policy();

    div()
        .id(SharedString::from(format!("profile-{}", profile.id().as_str())))
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
                .child(profile_color_swatch(profile.color_hex()))
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
                                .child(profile.name().to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(profile_detail_label(profile)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(render_profile_sync_action(index, sync_profile_id, sync_policy, cx))
                .child(render_profile_action(index, profile_id, active, cx)),
        )
        .into_any_element()
}

fn profile_color_swatch(color_hex: u32) -> AnyElement {
    div()
        .w(px(14.0))
        .h(px(14.0))
        .rounded_full()
        .border_1()
        .border_color(rgb(colors::HAIRLINE_STRONG))
        .bg(rgb(color_hex))
        .into_any_element()
}

fn render_profile_action(
    index: usize,
    profile_id: ProfileId,
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

    Button::new(("switch-profile", index))
        .small()
        .primary()
        .icon(IconName::Check)
        .label("Switch")
        .tooltip("Switch Profile")
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_profile(&profile_id, window, cx);
        }))
        .into_any_element()
}

fn render_profile_sync_action(
    index: usize,
    profile_id: ProfileId,
    sync_policy: ProfileSyncPolicy,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let next_policy = sync_policy.toggled();
    let paused = sync_policy == ProfileSyncPolicy::Paused;

    Button::new(("profile-sync-policy", index))
        .ghost()
        .xsmall()
        .selected(paused)
        .label(sync_policy.action_label())
        .tooltip(sync_policy.label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_profile_sync_policy(&profile_id, next_policy, cx);
        }))
        .into_any_element()
}

fn profile_detail_label(profile: &Profile) -> String {
    format!(
        "{} - {} - {}",
        profile_kind_label(profile.kind()),
        download_policy_label(profile.download_policy()),
        profile.sync_policy().label()
    )
}

fn profile_kind_label(kind: &ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Standard => "Standard",
        ProfileKind::Private => "Private",
    }
}

fn download_policy_label(policy: &DownloadPolicy) -> String {
    match policy.destination() {
        DownloadDestination::AskEveryTime => "Ask every time".to_string(),
        DownloadDestination::FixedDirectory(path) => path.display().to_string(),
    }
}
