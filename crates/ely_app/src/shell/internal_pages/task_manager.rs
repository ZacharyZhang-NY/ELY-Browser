use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use ely_domain::{BrowserTab, DownloadEntry, DownloadState, TabId, TabState};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{
    ElyShell,
    download_labels::{download_size_label, download_state_label},
    render_canvas_surface,
};

impl ElyShell {
    pub(super) fn render_task_manager_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_task_manager_header(snapshot))
                .child(render_task_summary(snapshot))
                .child(render_task_table(snapshot)),
        )
    }
}

fn render_task_manager_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Task Manager"))
                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(format!(
                    "{} / {}",
                    snapshot.active_profile_name, snapshot.active_space_name
                ))),
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
                .child(format!("{} local tasks", task_count(snapshot))),
        )
        .into_any_element()
}

fn render_task_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .border_t_1()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .children([
            task_metric("Tabs", snapshot.tabs.len()),
            task_metric("Downloads", snapshot.download_entries.len()),
            task_metric("Plugins", snapshot.installed_plugins.len()),
            task_metric("Spaces", snapshot.spaces.len()),
        ])
        .into_any_element()
}

fn task_metric(label: &'static str, value: usize) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div().text_sm().font_semibold().text_color(rgb(colors::INK)).child(value.to_string()),
        )
        .into_any_element()
}

fn render_task_table(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .child(render_section_header("Tabs"))
        .children(
            snapshot
                .tabs
                .iter()
                .enumerate()
                .map(|(index, tab)| render_tab_task_row(index, tab, &snapshot.active_tab_id)),
        )
        .when(!snapshot.download_entries.is_empty(), |this| {
            this.child(render_section_header("Downloads")).children(
                snapshot
                    .download_entries
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, entry)| render_download_task_row(index, entry)),
            )
        })
        .when(!snapshot.installed_plugins.is_empty(), |this| {
            this.child(render_section_header("Plugins")).children(
                snapshot.installed_plugins.iter().enumerate().map(|(index, plugin)| {
                    render_plugin_task_row(index, plugin, &snapshot.active_profile_kind)
                }),
            )
        })
        .into_any_element()
}

fn render_section_header(label: &'static str) -> AnyElement {
    div()
        .pt_4()
        .pb_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(colors::MUTED))
        .child(label)
        .into_any_element()
}

fn render_tab_task_row(index: usize, tab: &BrowserTab, active_tab_id: &TabId) -> AnyElement {
    let status = tab_state_label(tab.state());
    let status_color = tab_state_color(tab.state());
    let scope = if tab.id() == active_tab_id { "Active tab" } else { "Tab" };
    let icon = if tab.id() == active_tab_id { IconName::CircleCheck } else { IconName::Globe };

    render_task_row(
        format!("tab-task-{index}"),
        icon,
        tab.title().to_string(),
        tab.display_url(),
        scope,
        status,
        status_color,
    )
}

fn render_download_task_row(index: usize, entry: &DownloadEntry) -> AnyElement {
    render_task_row(
        format!("download-task-{index}"),
        IconName::File,
        entry.file_name().to_string(),
        download_size_label(entry),
        "Download",
        download_state_label(entry.state()),
        download_state_color(entry.state()),
    )
}

fn render_plugin_task_row(
    index: usize,
    plugin: &InstalledPlugin,
    profile_kind: &ely_domain::ProfileKind,
) -> AnyElement {
    let status = if plugin.enabled_for_profile(profile_kind) { "Enabled" } else { "Disabled" };
    let status_color =
        if plugin.enabled_for_profile(profile_kind) { colors::SUCCESS } else { colors::MUTED };
    let detail = format!(
        "{} permissions - {} contributions",
        plugin.manifest().permissions().len(),
        plugin.manifest().contributes().len()
    );

    render_task_row(
        format!("plugin-task-{index}"),
        IconName::Asterisk,
        plugin.manifest().name().to_string(),
        detail,
        "Plugin",
        status,
        status_color,
    )
}

fn render_task_row(
    id: String,
    icon: IconName,
    title: String,
    detail: String,
    scope: &'static str,
    status: &'static str,
    status_color: u32,
) -> AnyElement {
    div()
        .id(SharedString::from(id))
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
                .child(div().text_color(rgb(colors::MUTED_SOFT)).child(icon))
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
                                .child(title),
                        )
                        .child(
                            div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(detail),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_3()
                .text_xs()
                .child(div().min_w(px(86.0)).text_color(rgb(colors::MUTED)).child(scope))
                .child(
                    div()
                        .min_w(px(76.0))
                        .font_semibold()
                        .text_color(rgb(status_color))
                        .child(status),
                ),
        )
        .into_any_element()
}

fn task_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot.tabs.len() + snapshot.download_entries.len() + snapshot.installed_plugins.len()
}

fn tab_state_label(state: &TabState) -> &'static str {
    match state {
        TabState::Loading => "Loading",
        TabState::Ready => "Ready",
        TabState::Crashed => "Crashed",
        TabState::Discarded => "Discarded",
        TabState::Archived => "Archived",
    }
}

fn tab_state_color(state: &TabState) -> u32 {
    match state {
        TabState::Loading => colors::PRIMARY,
        TabState::Ready => colors::SUCCESS,
        TabState::Crashed => colors::ERROR,
        TabState::Discarded | TabState::Archived => colors::MUTED,
    }
}

fn download_state_color(state: &DownloadState) -> u32 {
    match state {
        DownloadState::InProgress => colors::PRIMARY,
        DownloadState::Completed => colors::SUCCESS,
        DownloadState::Paused => colors::MUTED,
        DownloadState::Cancelled | DownloadState::Failed => colors::ERROR,
    }
}
