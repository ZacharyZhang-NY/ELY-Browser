use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    SyncConnectionState, SyncObjectKind, SyncObjectPolicy, SyncObjectState, SyncObjectStatus,
};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, scroll::ScrollableElement};

use crate::brand::SYNC_SERVICE_NAME;

use super::{ElyShell, render_canvas_surface};
use crate::shell::chrome::SERIF_FAMILY;

impl ElyShell {
    pub(super) fn render_sync_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div().size_full().pt(px(40.0)).px(px(56.0)).pb(px(32.0)).flex().justify_center().child(
                div()
                    .max_w(px(960.0))
                    .grid()
                    .grid_cols(2)
                    .gap(px(32.0))
                    .child(render_left_column(snapshot, cx))
                    .child(render_right_column(snapshot, cx)),
            ),
        )
    }
}

fn render_left_column(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(20.0))
        .child(render_status_pill(snapshot))
        .child(render_serif_headline())
        .child(render_intro_paragraph())
        .child(render_metrics_card(snapshot, cx))
        .into_any_element()
}

fn render_status_pill(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(5.0))
        .rounded(px(999.0))
        .bg(rgba(PILL_BG))
        .text_size(px(11.0))
        .text_color(rgb(colors::ink_3()))
        .child(div().text_color(rgb(colors::accent())).child(IconName::Globe))
        .child(format!(
            "{SYNC_SERVICE_NAME} · {}",
            connection_label(snapshot.sync_status.connection())
        ))
        .into_any_element()
}

fn render_serif_headline() -> AnyElement {
    div()
        .font_family(SERIF_FAMILY)
        .text_size(px(46.0))
        .font_weight(FontWeight(400.0))
        .text_color(rgb(colors::ink()))
        .child("Your tabs, on every device.")
        .into_any_element()
}

fn render_intro_paragraph() -> AnyElement {
    div()
        .max_w(px(440.0))
        .text_size(px(14.0))
        .text_color(rgb(colors::ink_2()))
        .child(
            "ELY keeps tabs, workspaces, pinned items, and history mirrored across your \
             devices — encrypted in your hands and replayed at the edge.",
        )
        .into_any_element()
}

fn render_metrics_card(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .max_w(px(380.0))
        .p(px(20.0))
        .rounded(px(16.0))
        .bg(rgba(CARD_BG))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(div().text_size(px(12.5)).text_color(rgb(colors::ink_3())).child("Local queue"))
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(px(12.0))
                .child(render_metric(
                    "Pending",
                    snapshot.sync_status.pending_objects(),
                    colors::ink(),
                ))
                .child(render_metric(
                    "Failed",
                    snapshot.sync_status.failed_objects(),
                    colors::error(),
                )),
        )
        .child(render_reset_button(cx))
        .into_any_element()
}

fn render_metric(label: &'static str, value: usize, color: u32) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(10.5)).text_color(rgb(colors::ink_4())).child(label))
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(color))
                .child(value.to_string()),
        )
        .into_any_element()
}

fn render_reset_button(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .gap(px(8.0))
        .child(
            div()
                .id(SharedString::from("sync-upload"))
                .px(px(12.0))
                .py(px(7.0))
                .rounded(px(8.0))
                .bg(rgba(colors::accent()))
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(0xfff5e6))
                .cursor_pointer()
                .hover(|style| style.opacity(0.92))
                .active(|style| style.opacity(0.78))
                .on_click(cx.listener(|shell, _, _, cx| shell.trigger_cloud_sync_upload(cx)))
                .child("Sync now"),
        )
        .child(
            div()
                .id(SharedString::from("sync-reset"))
                .px(px(12.0))
                .py(px(7.0))
                .rounded(px(8.0))
                .bg(rgba(BUTTON_BG))
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink_2()))
                .cursor_pointer()
                .hover(|style| style.bg(rgba(BUTTON_BG_HOVER)))
                .active(|style| style.opacity(0.85))
                .on_click(cx.listener(|shell, _, _, cx| shell.reset_sync_settings(cx)))
                .child("Reset to defaults"),
        )
        .into_any_element()
}

fn render_right_column(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(render_what_syncs_card(snapshot, cx))
        .into_any_element()
}

fn render_what_syncs_card(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .p(px(18.0))
        .rounded(px(16.0))
        .bg(rgba(CARD_BG))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .max_h(px(640.0))
        .overflow_y_scrollbar()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child("What syncs"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::ink_3()))
                        .child(format!("{} kinds tracked", snapshot.sync_status.objects().len())),
                ),
        )
        .child(
            div().flex().flex_col().gap(px(2.0)).children(
                snapshot
                    .sync_status
                    .objects()
                    .iter()
                    .enumerate()
                    .map(|(index, status)| render_sync_object_row(index, status, cx)),
            ),
        )
        .into_any_element()
}

fn render_sync_object_row(
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .child(render_state_dot(status.state()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child(sync_object_kind_label(status.kind())),
                )
                .child(div().text_size(px(11.0)).text_color(rgb(colors::ink_4())).child(format!(
                    "{} local · {}",
                    status.local_count(),
                    sync_object_state_label(status.state())
                ))),
        )
        .child(render_policy_toggle(index, status, cx))
        .into_any_element()
}

fn render_state_dot(state: SyncObjectState) -> AnyElement {
    let color = match state {
        SyncObjectState::LocalOnly => colors::ink_4(),
        SyncObjectState::Paused => colors::ink_5(),
        SyncObjectState::PrivacyControlled => colors::accent(),
    };

    div().size(px(8.0)).rounded_full().bg(rgb(color)).into_any_element()
}

fn render_policy_toggle(
    index: usize,
    status: &SyncObjectStatus,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let enabled = status.policy() == SyncObjectPolicy::Enabled;
    let next_policy = if enabled { SyncObjectPolicy::Paused } else { SyncObjectPolicy::Enabled };
    let kind = status.kind();
    let track_color = if enabled { colors::accent() } else { 0x281e1426 };

    div()
        .id(SharedString::from(format!("sync-policy-{index}")))
        .w(px(34.0))
        .h(px(20.0))
        .rounded_full()
        .bg(rgba(track_color))
        .p(px(2.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.9))
        .active(|style| style.opacity(0.78))
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_sync_object_policy(kind, next_policy, cx);
        }))
        .child(
            div()
                .size(px(16.0))
                .rounded_full()
                .bg(rgb(0xffffff))
                .when(enabled, |this| this.ml(px(14.0))),
        )
        .into_any_element()
}

fn connection_label(connection: &SyncConnectionState) -> &'static str {
    match connection {
        SyncConnectionState::SignedOut => "Local-only · sign-in coming soon",
    }
}

fn sync_object_kind_label(kind: SyncObjectKind) -> &'static str {
    match kind {
        SyncObjectKind::Spaces => "Spaces",
        SyncObjectKind::Tabs => "Tabs & pinned",
        SyncObjectKind::Bookmarks => "Bookmarks",
        SyncObjectKind::Notes => "Notes",
        SyncObjectKind::ReadingList => "Reading list",
        SyncObjectKind::Profiles => "Profiles",
        SyncObjectKind::SitePermissions => "Site permissions",
        SyncObjectKind::History => "History",
        SyncObjectKind::PluginSettings => "Plugins",
    }
}

fn sync_object_state_label(state: SyncObjectState) -> &'static str {
    match state {
        SyncObjectState::LocalOnly => "Local only",
        SyncObjectState::Paused => "Paused",
        SyncObjectState::PrivacyControlled => "Privacy controlled",
    }
}

const PILL_BG: u32 = 0xffffffb3;
const CARD_BG: u32 = 0xffffffd9;
const BUTTON_BG: u32 = 0xffffffd9;
const BUTTON_BG_HOVER: u32 = 0xffffffeb;
