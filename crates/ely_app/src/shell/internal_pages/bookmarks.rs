use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BookmarkEntry;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_bookmarks_page(
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
                .child(render_bookmarks_header(snapshot))
                .child(self.render_bookmark_list(snapshot, cx)),
        )
    }

    fn render_bookmark_list(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.bookmarks.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("No bookmarks in this Profile.")
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
                    .bookmarks
                    .iter()
                    .rev()
                    .map(|bookmark| self.render_bookmark_row(bookmark, cx)),
            )
            .into_any_element()
    }

    fn render_bookmark_row(
        &mut self,
        bookmark: &BookmarkEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = bookmark.url().clone();

        div()
            .id(SharedString::from(format!("bookmark-{}", bookmark.id().as_str())))
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
                    .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::BookOpen))
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
                                    .child(bookmark.title().to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::MUTED))
                                    .child(bookmark.display_url()),
                            ),
                    ),
            )
            .child(
                div()
                    .max_w(px(180.0))
                    .truncate()
                    .text_xs()
                    .font_semibold()
                    .text_color(rgb(colors::MUTED))
                    .child(bookmark.collection_name().to_string()),
            )
            .into_any_element()
    }
}

fn render_bookmarks_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Bookmarks"))
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
                .child(bookmark_count_label(snapshot.bookmarks.len())),
        )
        .into_any_element()
}

fn bookmark_count_label(count: usize) -> String {
    match count {
        1 => "1 bookmark".to_string(),
        count => format!("{count} bookmarks"),
    }
}
