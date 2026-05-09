use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    SyncConnectionState, SyncObjectKind, SyncObjectPolicy, SyncObjectState, SyncObjectStatus,
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    px, rgb,
};
use gpui::{StatefulInteractiveElement, prelude::FluentBuilder};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_sync_page(
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
                .child(render_sync_header(snapshot))
                .child(render_sync_queue(snapshot, cx))
                .child(render_sync_objects(snapshot, cx)),
        )
    }
}

fn render_sync_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Sync"))
                .child(
                    div()
                        .text_sm()
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
                .child(IconName::Globe)
                .child(connection_label(snapshot.sync_status.connection())),
        )
        .into_any_element()
}

fn render_sync_queue(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
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
                .flex()
                .items_center()
                .gap_4()
                .child(metric_block(
                    "Pending objects",
                    snapshot.sync_status.pending_objects(),
                    colors::INK,
                ))
                .child(metric_block(
                    "Failed objects",
                    snapshot.sync_status.failed_objects(),
                    colors::ERROR,
                )),
        )
        .child(
            Button::new("reset-sync-settings")
                .ghost()
                .xsmall()
                .icon(IconName::Undo2)
                .label("Reset")
                .tooltip("Restore Sync Defaults")
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.reset_sync_settings(cx);
                })),
        )
        .into_any_element()
}

fn metric_block(label: &'static str, value: usize, color: u32) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div()
                .text_size(px(18.0))
                .font_semibold()
                .text_color(rgb(color))
                .child(value.to_string()),
        )
        .into_any_element()
}

fn render_sync_objects(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
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
                .sync_status
                .objects()
                .iter()
                .enumerate()
                .map(|(index, status)| render_sync_object_row(index, status, cx)),
        )
        .into_any_element()
}

fn render_sync_object_row(
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
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
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .truncate()
                        .text_color(rgb(colors::INK))
                        .child(sync_object_kind_label(status.kind())),
                )
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("{} local objects", status.local_count())),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(sync_object_state_color(status.state())))
                        .child(sync_object_state_label(status.state())),
                )
                .child(render_sync_policy_toggle(index, status, cx)),
        )
        .into_any_element()
}

fn render_sync_policy_toggle(
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let enabled = status.policy() == SyncObjectPolicy::Enabled;
    let next_policy = if enabled { SyncObjectPolicy::Paused } else { SyncObjectPolicy::Enabled };
    let kind = status.kind();

    div()
        .id(SharedString::from(format!("sync-policy-toggle-{index}")))
        .w(px(38.0))
        .h(px(22.0))
        .rounded_full()
        .border_1()
        .border_color(rgb(sync_policy_border_color(enabled)))
        .bg(rgb(sync_policy_track_color(enabled)))
        .p(px(2.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.9))
        .active(|style| style.opacity(0.78))
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .bg(rgb(colors::SURFACE_CARD))
                .shadow_sm()
                .when(enabled, |this| this.ml(px(16.0))),
        )
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_sync_object_policy(kind, next_policy, cx);
        }))
        .into_any_element()
}

fn connection_label(connection: &SyncConnectionState) -> &'static str {
    match connection {
        SyncConnectionState::SignedOut => "Signed out",
    }
}

fn sync_object_kind_label(kind: SyncObjectKind) -> &'static str {
    match kind {
        SyncObjectKind::Spaces => "Spaces",
        SyncObjectKind::Tabs => "Tabs",
        SyncObjectKind::Bookmarks => "Bookmarks",
        SyncObjectKind::Notes => "Notes",
        SyncObjectKind::ReadingList => "Reading List",
        SyncObjectKind::Profiles => "Profiles",
        SyncObjectKind::SitePermissions => "Site permissions",
        SyncObjectKind::History => "History",
        SyncObjectKind::PluginSettings => "Plugin settings",
    }
}

fn sync_object_state_label(state: SyncObjectState) -> &'static str {
    match state {
        SyncObjectState::LocalOnly => "Local only",
        SyncObjectState::Paused => "Paused",
        SyncObjectState::PrivacyControlled => "Privacy controlled",
    }
}

fn sync_object_state_color(state: SyncObjectState) -> u32 {
    match state {
        SyncObjectState::LocalOnly => colors::MUTED,
        SyncObjectState::Paused => colors::PRIMARY,
        SyncObjectState::PrivacyControlled => colors::PRIMARY,
    }
}

fn sync_policy_track_color(enabled: bool) -> u32 {
    if enabled { colors::SUCCESS } else { colors::CANVAS_SOFT }
}

fn sync_policy_border_color(enabled: bool) -> u32 {
    if enabled { colors::SUCCESS } else { colors::HAIRLINE_STRONG }
}
