use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{
    DownloadDestination, DownloadEntry, DownloadPolicy, DownloadSecurity, DownloadState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_downloads_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(26.0))
                                        .text_color(rgb(colors::INK))
                                        .child("Downloads"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(colors::MUTED))
                                        .child(snapshot.active_profile_name.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap_1()
                                .child(div().text_xs().text_color(rgb(colors::MUTED)).child(
                                    format!("{} downloads", snapshot.download_entries.len()),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(rgb(colors::MUTED))
                                        .child(IconName::Folder)
                                        .child(div().max_w(px(360.0)).truncate().child(
                                            download_policy_label(&snapshot.active_download_policy),
                                        )),
                                ),
                        ),
                )
                .child(render_downloads_list(snapshot)),
        )
    }
}

fn render_downloads_list(snapshot: &BrowserSnapshot) -> AnyElement {
    if snapshot.download_entries.is_empty() {
        return div()
            .flex_1()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .pt_5()
            .text_sm()
            .text_color(rgb(colors::MUTED))
            .child("Downloads are empty for this Profile.")
            .into_any_element();
    }

    div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(
            snapshot
                .download_entries
                .iter()
                .rev()
                .enumerate()
                .map(|(index, entry)| render_download_row(index, entry)),
        )
        .into_any_element()
}

fn render_download_row(index: usize, entry: &DownloadEntry) -> AnyElement {
    div()
        .id(SharedString::from(format!("download-{index}")))
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
                .child(div().text_color(rgb(colors::MUTED)).child(IconName::File))
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
                                .child(entry.file_name().to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(entry.source_url().display_url()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(download_destination_label(entry.destination())),
                                )
                                .when(entry.security().requires_prompt(), |this| {
                                    this.child(render_security_prompt(entry.security()))
                                }),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::BODY))
                        .child(download_state_label(entry.state())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child(download_size_label(entry)),
                ),
        )
        .into_any_element()
}

fn download_state_label(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::InProgress => "In progress",
        DownloadState::Paused => "Paused",
        DownloadState::Completed => "Complete",
        DownloadState::Cancelled => "Cancelled",
        DownloadState::Failed => "Failed",
    }
}

fn download_size_label(entry: &DownloadEntry) -> String {
    match entry.total_bytes() {
        Some(total_bytes) => {
            format!("{} of {}", format_bytes(entry.received_bytes()), format_bytes(total_bytes))
        }
        None => format_bytes(entry.received_bytes()),
    }
}

fn download_policy_label(policy: &DownloadPolicy) -> String {
    format!("Profile path: {}", download_destination_label(policy.destination()))
}

fn download_destination_label(destination: &DownloadDestination) -> String {
    match destination {
        DownloadDestination::AskEveryTime => "Ask before saving".to_string(),
        DownloadDestination::FixedDirectory(path) => path.display().to_string(),
    }
}

fn render_security_prompt(security: &DownloadSecurity) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_color(rgb(colors::ERROR))
        .child(IconName::TriangleAlert)
        .child(download_security_label(security))
        .into_any_element()
}

fn download_security_label(security: &DownloadSecurity) -> &'static str {
    match security {
        DownloadSecurity::Standard => "Standard",
        DownloadSecurity::DangerousExtension => "Extension prompt required",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        return format!("{:.1} GB", bytes as f64 / GIB as f64);
    }
    if bytes >= MIB {
        return format!("{:.1} MB", bytes as f64 / MIB as f64);
    }
    if bytes >= KIB {
        return format!("{:.1} KB", bytes as f64 / KIB as f64);
    }
    format!("{bytes} B")
}
