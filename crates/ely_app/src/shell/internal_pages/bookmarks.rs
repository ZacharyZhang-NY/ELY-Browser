use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BookmarkEntry;
use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb,
};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    scroll::ScrollableElement,
};

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
                    .enumerate()
                    .map(|(index, bookmark)| self.render_bookmark_row(index, bookmark, cx)),
            )
            .into_any_element()
    }

    fn render_bookmark_row(
        &mut self,
        index: usize,
        bookmark: &BookmarkEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = bookmark.url().clone();
        let open_url = bookmark.url().clone();
        let edit_bookmark = bookmark.clone();
        let pending_edit = self
            .pending_bookmark_edit
            .clone()
            .filter(|pending_edit| pending_edit.matches(bookmark.id()));
        let editing = pending_edit.is_some();
        let edit_error = self.bookmark_edit_error.clone();

        div()
            .id(SharedString::from(format!("bookmark-{}", bookmark.id().as_str())))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .min_w_0()
                            .id(SharedString::from(format!(
                                "bookmark-open-{}",
                                bookmark.id().as_str()
                            )))
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_3()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                shell.open_url(url.clone(), window, cx);
                            }))
                            .child(
                                div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::BookOpen),
                            )
                            .child(render_bookmark_summary(bookmark)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new(("open-bookmark", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ExternalLink)
                                    .tooltip("Open Bookmark")
                                    .on_click(cx.listener(move |shell, _, window, cx| {
                                        shell.open_url(open_url.clone(), window, cx);
                                    })),
                            )
                            .child(
                                Button::new(("edit-bookmark", index))
                                    .ghost()
                                    .xsmall()
                                    .selected(editing)
                                    .icon(IconName::Replace)
                                    .tooltip("Edit Bookmark")
                                    .on_click(cx.listener(move |shell, _, window, cx| {
                                        shell.start_bookmark_edit(&edit_bookmark, window, cx);
                                    })),
                            ),
                    ),
            )
            .when_some(pending_edit, |this, pending_edit| {
                this.child(render_bookmark_editor(index, &pending_edit, edit_error.as_deref(), cx))
            })
            .into_any_element()
    }
}

fn render_bookmark_summary(bookmark: &BookmarkEntry) -> AnyElement {
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
            div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(bookmark.display_url()),
        )
        .child(
            div()
                .text_xs()
                .truncate()
                .text_color(rgb(colors::MUTED_SOFT))
                .child(bookmark_metadata_label(bookmark)),
        )
        .into_any_element()
}

fn render_bookmark_editor(
    index: usize,
    pending_edit: &crate::shell::bookmarks::PendingBookmarkEdit,
    edit_error: Option<&str>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE_STRONG))
        .bg(rgb(colors::CANVAS_SOFT))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_3()
                .child(render_bookmark_edit_field("Collection", pending_edit.collection_input()))
                .child(render_bookmark_edit_field("Tags", pending_edit.tags_input())),
        )
        .child(render_bookmark_edit_field("Note", pending_edit.note_input()))
        .when_some(edit_error.map(ToOwned::to_owned), |this, message| {
            this.child(div().text_xs().text_color(rgb(colors::ERROR)).child(message))
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    Button::new(("cancel-bookmark-edit", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .label("Cancel")
                        .tooltip("Cancel Bookmark Edit")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.cancel_bookmark_edit(cx);
                        })),
                )
                .child(
                    Button::new(("save-bookmark-edit", index))
                        .primary()
                        .xsmall()
                        .icon(IconName::Check)
                        .label("Save")
                        .tooltip("Save Bookmark")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.save_bookmark_edit(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_bookmark_edit_field(label: &'static str, input: &Entity<InputState>) -> AnyElement {
    div()
        .min_w_0()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child(label))
        .child(Input::new(input).small())
        .into_any_element()
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

fn bookmark_metadata_label(bookmark: &BookmarkEntry) -> String {
    let mut parts = vec![bookmark.collection_name().to_string()];
    if !bookmark.tags().is_empty() {
        parts.push(format!("Tags: {}", bookmark.tags().join(", ")));
    }
    if let Some(note) = bookmark.note() {
        parts.push(note.to_string());
    }
    parts.join(" - ")
}

fn bookmark_count_label(count: usize) -> String {
    match count {
        1 => "1 bookmark".to_string(),
        count => format!("{count} bookmarks"),
    }
}
