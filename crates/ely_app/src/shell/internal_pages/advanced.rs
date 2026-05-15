use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ArchivePolicy, Space};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{ElyShell, download_labels::download_policy_label, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_advanced_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_advanced_header(snapshot))
                .child(render_advanced_summary(snapshot))
                .child(render_advanced_rows(snapshot)),
        )
    }
}

fn render_advanced_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Advanced"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::muted()))
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
                .text_color(rgb(colors::muted()))
                .child(IconName::Inspector)
                .child(format!("{} policies", advanced_policy_count())),
        )
        .into_any_element()
}

fn render_advanced_summary(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_color(rgb(colors::primary())).child(IconName::Inspector))
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
                                .child("Local Runtime"),
                        )
                        .child(div().text_xs().truncate().text_color(rgb(colors::muted())).child(
                            format!(
                                "{} space / {} profile",
                                snapshot.active_space_name, snapshot.active_profile_name
                            ),
                        )),
                ),
        )
        .child(div().text_xs().font_semibold().text_color(rgb(colors::success())).child("Local"))
        .into_any_element()
}

fn render_advanced_rows(snapshot: &BrowserSnapshot) -> AnyElement {
    let active_space = snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id);

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .child(advanced_row(
            IconName::Eye,
            "History Recording",
            snapshot.history_recording_policy.status(),
            snapshot.history_recording_policy.detail(),
        ))
        .child(advanced_row(
            IconName::Star,
            "Favorite Limit",
            snapshot.favorite_limit.label(),
            snapshot.favorite_limit.detail(),
        ))
        .child(render_sidebar_width_row(active_space))
        .child(render_archive_policy_row(active_space))
        .child(advanced_row(
            IconName::Folder,
            "Download Policy",
            download_policy_label(&snapshot.active_download_policy),
            "Active Profile download destination policy",
        ))
        .child(advanced_row(
            IconName::Globe,
            "Sync Objects",
            snapshot.sync_status.objects().len().to_string(),
            "Object scopes tracked by local Sync state",
        ))
        .child(advanced_row(
            IconName::Asterisk,
            "Installed Plugins",
            snapshot.installed_plugins.len().to_string(),
            "Verified plugins registered in Browser Core",
        ))
        .child(advanced_row(
            IconName::Inspector,
            "Audit Events",
            audit_event_count(snapshot).to_string(),
            "Plugin and site permission audit records",
        ))
        .into_any_element()
}

fn render_sidebar_width_row(active_space: Option<&Space>) -> AnyElement {
    let value = active_space
        .map(|space| format!("{} px", space.sidebar_width_px()))
        .unwrap_or_else(|| "Unavailable".to_string());
    advanced_row(IconName::PanelLeft, "Sidebar Width", value, "Current Space sidebar width")
}

fn render_archive_policy_row(active_space: Option<&Space>) -> AnyElement {
    let value = active_space
        .map(|space| archive_policy_label(space.archive_policy()).to_string())
        .unwrap_or_else(|| "Unavailable".to_string());
    advanced_row(IconName::Inbox, "Auto Archive", value, "Current Space idle unpinned tab policy")
}

fn advanced_row(
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
                .max_w(px(280.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child(value),
        )
        .into_any_element()
}

fn archive_policy_label(policy: &ArchivePolicy) -> &'static str {
    match policy {
        ArchivePolicy::Manual => "Manual",
        ArchivePolicy::IdleDays(0) => "Today",
        ArchivePolicy::IdleDays(1) => "1 day",
        ArchivePolicy::IdleDays(7) => "7 days",
        ArchivePolicy::IdleDays(30) => "30 days",
        ArchivePolicy::IdleDays(_) => "Custom",
    }
}

fn audit_event_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot.plugin_audit_events.len() + snapshot.site_permission_audit_events.len()
}

fn advanced_policy_count() -> usize {
    8
}
