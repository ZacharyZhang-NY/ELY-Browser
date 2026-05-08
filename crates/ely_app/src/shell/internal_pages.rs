mod about;
mod bookmarks;
mod download_actions;
mod download_labels;
mod downloads;
mod plugin_catalog;
mod plugin_details;
mod plugins;
mod profiles;
mod reading_list;
mod settings;
mod shortcuts;
mod sidebar_tabs;
mod site_settings;
mod spaces;
mod sync;
mod task_manager;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{ArchiveSource, ArchivedTab, BrowserTab, HistoryEntry};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::ElyShell;

impl ElyShell {
    pub(super) fn render_web_canvas(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match tab.url().as_str() {
            "ely://bookmarks" => self.render_bookmarks_page(snapshot, cx),
            "ely://reading-list" => self.render_reading_list_page(snapshot, cx),
            "ely://downloads" => self.render_downloads_page(snapshot, cx),
            "ely://history" => self.render_history_page(snapshot, cx),
            "ely://archive" => self.render_archive_page(snapshot, cx),
            "ely://task-manager" => self.render_task_manager_page(snapshot),
            "ely://plugins" => self.render_plugin_catalog_page(snapshot, cx),
            url if url.starts_with("ely://plugin/") => {
                self.render_plugin_detail_page(snapshot, url, cx)
            }
            url if url.starts_with("ely://site/") => {
                self.render_site_settings_page(snapshot, url, cx)
            }
            "ely://about" => self.render_about_page(snapshot),
            "ely://settings" => self.render_settings_page(snapshot, cx),
            "ely://settings/sidebar-tabs" => self.render_sidebar_tabs_page(snapshot, cx),
            "ely://settings/spaces" => self.render_spaces_page(snapshot, cx),
            "ely://settings/shortcuts" => self.render_shortcuts_page(snapshot),
            "ely://settings/plugins" => self.render_plugins_page(snapshot, cx),
            "ely://settings/profiles" => self.render_profiles_page(snapshot, cx),
            "ely://settings/sync" => self.render_sync_page(snapshot),
            "ely://sync/status" => self.render_sync_page(snapshot),
            _ => render_default_page(tab),
        }
    }

    fn render_history_page(
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
                                        .child("History"),
                                )
                                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(
                                    format!(
                                        "{} / {}",
                                        snapshot.active_profile_name, snapshot.active_space_name
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(format!("{} entries", snapshot.history_entries.len())),
                        ),
                )
                .child(self.render_history_list(snapshot, cx)),
        )
    }

    fn render_history_list(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.history_entries.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("History is empty for this Space and Profile.")
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
                    .history_entries
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, entry)| self.render_history_row(index, entry, cx)),
            )
            .into_any_element()
    }

    fn render_history_row(
        &mut self,
        index: usize,
        entry: &HistoryEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = entry.url().clone();

        div()
            .id(SharedString::from(format!("history-{index}")))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.open_url(url.clone(), window, cx);
            }))
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
                            .child(entry.title().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(colors::MUTED))
                            .child(entry.url().display_url()),
                    ),
            )
            .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::ExternalLink))
            .into_any_element()
    }

    fn render_archive_page(
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
                                        .child("Archived Tabs"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(colors::MUTED))
                                        .child("All Spaces"),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(format!("{} archived", snapshot.archived_tabs.len())),
                        ),
                )
                .child(self.render_archive_list(snapshot, cx)),
        )
    }

    fn render_archive_list(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.archived_tabs.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("Archive is empty.")
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
                    .archived_tabs
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, archived_tab)| self.render_archive_row(index, archived_tab, cx)),
            )
            .into_any_element()
    }

    fn render_archive_row(
        &mut self,
        index: usize,
        archived_tab: &ArchivedTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = archived_tab.tab();
        let tab_id = tab.id().clone();

        div()
            .id(SharedString::from(format!("archive-{index}")))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.restore_archived_tab(&tab_id, window, cx);
            }))
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
                            .child(tab.title().to_string()),
                    )
                    .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(
                        format!(
                            "{} - {}",
                            tab.display_url(),
                            archive_source_label(archived_tab.source())
                        ),
                    )),
            )
            .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::Undo2))
            .into_any_element()
    }
}

fn render_default_page(tab: &BrowserTab) -> AnyElement {
    render_canvas_surface(
        div()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(26.0))
                    .text_color(rgb(colors::INK))
                    .child(tab.title().to_string()),
            )
            .child(div().text_sm().text_color(rgb(colors::MUTED)).child(render_tab_status(tab))),
    )
}

fn render_canvas_surface(content: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .p(px(spacing::LG))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .size_full()
                .rounded_lg()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .bg(rgb(colors::SURFACE_CARD))
                .child(content),
        )
        .into_any_element()
}

fn render_tab_status(tab: &BrowserTab) -> String {
    match tab.url().as_str() {
        "ely://new-tab" => "Ready".to_string(),
        url => url.to_string(),
    }
}

fn archive_source_label(source: &ArchiveSource) -> &'static str {
    match source {
        ArchiveSource::ManualClose => "Closed",
        ArchiveSource::AutoArchive => "Auto archived",
    }
}
