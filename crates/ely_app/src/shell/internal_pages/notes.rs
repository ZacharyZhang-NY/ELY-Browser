use std::time::{Duration, SystemTime};

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::NoteEntry;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_notes_page(
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
                .child(render_notes_header(snapshot))
                .child(self.render_note_entries(snapshot, cx)),
        )
    }

    fn render_note_entries(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.notes.is_empty() {
            return div()
                .flex_1()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child("Notes are empty for this Profile.")
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
                    .notes
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, note)| self.render_note_row(index, snapshot, note, cx)),
            )
            .into_any_element()
    }

    fn render_note_row(
        &mut self,
        index: usize,
        snapshot: &BrowserSnapshot,
        note: &NoteEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = note.source_url().clone();
        let open_url = note.source_url().clone();
        let space_name = note_space_name(snapshot, note);

        div()
            .id(SharedString::from(format!("note-{}", note.id().as_str())))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .id(SharedString::from(format!("note-open-{}", note.id().as_str())))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
                    .active(|style| style.opacity(0.82))
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.open_url(url.clone(), window, cx);
                    }))
                    .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::File))
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
                                    .child(note.title().to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::MUTED))
                                    .child(note.display_url()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::MUTED_SOFT))
                                    .child(markdown_preview(note.body())),
                            ),
                    ),
            )
            .child(
                div()
                    .max_w(px(260.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .text_xs()
                    .font_semibold()
                    .text_color(rgb(colors::MUTED))
                    .when_some(space_name, |this, space_name| {
                        this.child(div().max_w(px(110.0)).truncate().child(space_name))
                    })
                    .child(note.target_label())
                    .child(updated_at_label(note.updated_at())),
            )
            .child(
                Button::new(("open-note-source", index))
                    .ghost()
                    .xsmall()
                    .label("Open")
                    .tooltip("Open Note Source")
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.open_url(open_url.clone(), window, cx);
                    })),
            )
            .into_any_element()
    }
}

fn render_notes_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Notes"))
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
                .child(notes_count_label(snapshot.notes.len())),
        )
        .into_any_element()
}

fn note_space_name(snapshot: &BrowserSnapshot, note: &NoteEntry) -> Option<String> {
    snapshot
        .spaces
        .iter()
        .find(|space| space.id() == note.space_id())
        .map(|space| space.name().to_string())
}

fn notes_count_label(count: usize) -> String {
    match count {
        1 => "1 note".to_string(),
        count => format!("{count} notes"),
    }
}

fn markdown_preview(body: &str) -> String {
    let preview = body
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or(body.trim());

    truncate_chars(preview, 120)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() { format!("{truncated}...") } else { truncated }
}

fn updated_at_label(updated_at: SystemTime) -> String {
    updated_at_label_for(updated_at, SystemTime::now())
}

fn updated_at_label_for(updated_at: SystemTime, now: SystemTime) -> String {
    let age = now.duration_since(updated_at).unwrap_or(Duration::ZERO);
    if age < Duration::from_secs(60) {
        return "Updated just now".to_string();
    }
    if age < Duration::from_secs(3_600) {
        return format!("Updated {} mins ago", age.as_secs() / 60);
    }
    if age < Duration::from_secs(86_400) {
        return format!("Updated {} hrs ago", age.as_secs() / 3_600);
    }
    if age < Duration::from_secs(604_800) {
        return format!("Updated {} days ago", age.as_secs() / 86_400);
    }
    "Updated earlier".to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{markdown_preview, updated_at_label_for};

    #[test]
    fn markdown_preview_uses_first_non_empty_line() {
        assert_eq!(markdown_preview("\n  # Heading\n- detail"), "# Heading");
    }

    #[test]
    fn markdown_preview_truncates_long_lines() {
        let preview = markdown_preview(&"a".repeat(130));

        assert_eq!(preview.len(), 123);
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn updated_at_label_formats_recent_entries() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);

        assert_eq!(updated_at_label_for(now - Duration::from_secs(20), now), "Updated just now");
    }

    #[test]
    fn updated_at_label_formats_minutes_hours_days_and_older_entries() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);

        assert_eq!(updated_at_label_for(now - Duration::from_secs(120), now), "Updated 2 mins ago");
        assert_eq!(
            updated_at_label_for(now - Duration::from_secs(10_800), now),
            "Updated 3 hrs ago"
        );
        assert_eq!(
            updated_at_label_for(now - Duration::from_secs(345_600), now),
            "Updated 4 days ago"
        );
        assert_eq!(
            updated_at_label_for(now - Duration::from_secs(691_200), now),
            "Updated earlier"
        );
    }
}
