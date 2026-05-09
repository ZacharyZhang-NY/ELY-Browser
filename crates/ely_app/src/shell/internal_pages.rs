mod about;
mod appearance;
mod bookmarks;
mod crash;
mod download_actions;
mod download_labels;
mod download_settings;
mod downloads;
mod general;
mod history;
mod new_tab;
mod notes;
mod notes_markdown;
mod plugin_catalog;
mod plugin_details;
mod plugins;
mod privacy_security;
mod profiles;
mod reading_list;
mod search;
mod settings;
mod shortcuts;
mod sidebar_tabs;
mod site_permissions_settings;
mod site_settings;
mod sleep;
mod spaces;
mod sync;
mod tab_context;
mod task_manager;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{ArchivedTab, BrowserTab, TabState};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::ElyShell;
use super::archive_labels::archive_detail_label;

impl ElyShell {
    pub(super) fn render_web_canvas(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if tab.state() == &TabState::Crashed {
            return self.render_crash_page(tab, snapshot, cx);
        }
        if tab.state() == &TabState::Discarded {
            return self.render_sleep_page(tab, snapshot, cx);
        }

        match tab.url().as_str() {
            "ely://new-tab" => self.render_new_tab_page(snapshot, cx),
            "ely://bookmarks" => self.render_bookmarks_page(snapshot, cx),
            "ely://notes" => self.render_notes_page(snapshot, cx),
            "ely://reading-list" => self.render_reading_list_page(snapshot, cx),
            "ely://downloads" => self.render_downloads_page(snapshot, cx),
            "ely://history" => self.render_history_page(snapshot, cx),
            "ely://archive" => self.render_archive_page(snapshot, cx),
            "ely://task-manager" => self.render_task_manager_page(snapshot),
            "ely://plugins" => self.render_plugin_catalog_page(snapshot, cx),
            url if url.starts_with("ely://crash/") => self.render_crash_route(snapshot, url, cx),
            url if url.starts_with("ely://plugin/") => {
                self.render_plugin_detail_page(snapshot, url, cx)
            }
            url if url.starts_with("ely://site/") => {
                self.render_site_settings_page(snapshot, url, cx)
            }
            "ely://about" => self.render_about_page(snapshot),
            "ely://settings" => self.render_settings_page(snapshot, cx),
            "ely://settings/appearance" => self.render_appearance_page(snapshot),
            "ely://settings/general" => self.render_general_page(snapshot, cx),
            "ely://settings/sidebar-tabs" => self.render_sidebar_tabs_page(snapshot, cx),
            "ely://settings/search" => self.render_search_page(snapshot, cx),
            "ely://settings/privacy-security" => self.render_privacy_security_page(snapshot, cx),
            "ely://settings/downloads" => self.render_download_settings_page(snapshot, cx),
            "ely://settings/spaces" => self.render_spaces_page(snapshot, cx),
            "ely://settings/site-permissions" => {
                self.render_site_permissions_settings_page(snapshot, cx)
            }
            "ely://settings/shortcuts" => self.render_shortcuts_page(snapshot),
            "ely://settings/plugins" => self.render_plugins_page(snapshot, cx),
            "ely://settings/profiles" => self.render_profiles_page(snapshot, cx),
            "ely://settings/sync" => self.render_sync_page(snapshot, cx),
            "ely://sync/status" => self.render_sync_page(snapshot, cx),
            url if super::web_surface::is_external_web_url(url) => {
                self.render_external_web_canvas(tab, cx)
            }
            _ => render_default_page(tab),
        }
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
            .children(snapshot.archived_tabs.iter().rev().enumerate().map(
                |(index, archived_tab)| self.render_archive_row(index, archived_tab, snapshot, cx),
            ))
            .into_any_element()
    }

    fn render_archive_row(
        &mut self,
        index: usize,
        archived_tab: &ArchivedTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = archived_tab.tab();
        let tab_id = tab.id().clone();
        let detail = archive_detail_label(archived_tab, &snapshot.spaces, &snapshot.profiles);

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
                    .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(detail)),
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
