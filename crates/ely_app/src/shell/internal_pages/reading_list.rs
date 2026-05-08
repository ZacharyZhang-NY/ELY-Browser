use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ReadingListEntry, ReadingProgress};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_reading_list_page(
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
                .child(render_reading_list_header(snapshot))
                .child(self.render_reading_list_entries(snapshot, cx)),
        )
    }

    fn render_reading_list_entries(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.reading_list.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("Reading List is empty for this Profile.")
                .into_any_element();
        }

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
                    .reading_list
                    .iter()
                    .rev()
                    .map(|entry| self.render_reading_list_row(snapshot, entry, cx)),
            )
            .into_any_element()
    }

    fn render_reading_list_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        entry: &ReadingListEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = entry.source_url().clone();
        let space_name = reading_list_space_name(snapshot, entry);

        div()
            .id(SharedString::from(format!("reading-{}", entry.id().as_str())))
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
                    .items_center()
                    .gap_3()
                    .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::Inbox))
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
                                    .child(entry.display_url()),
                            ),
                    ),
            )
            .child(
                div()
                    .max_w(px(220.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .text_xs()
                    .font_semibold()
                    .text_color(rgb(colors::MUTED))
                    .when_some(space_name, |this, space_name| {
                        this.child(div().max_w(px(140.0)).truncate().child(space_name))
                    })
                    .child(progress_label(entry.progress())),
            )
            .into_any_element()
    }
}

fn render_reading_list_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Reading List"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(reading_list_count_label(snapshot.reading_list.len())),
        )
        .into_any_element()
}

fn reading_list_space_name(snapshot: &BrowserSnapshot, entry: &ReadingListEntry) -> Option<String> {
    snapshot
        .spaces
        .iter()
        .find(|space| space.id() == entry.space_id())
        .map(|space| space.name().to_string())
}

fn progress_label(progress: &ReadingProgress) -> &'static str {
    match progress {
        ReadingProgress::Unread => "Unread",
    }
}

fn reading_list_count_label(count: usize) -> String {
    match count {
        1 => "1 item".to_string(),
        count => format!("{count} items"),
    }
}
