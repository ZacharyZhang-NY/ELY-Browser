use std::time::{Duration, SystemTime};

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ReadingListEntry, ReadingListId, ReadingProgress};
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
                .border_color(rgb(colors::hairline()))
                .pt_5()
                .text_sm()
                .text_color(rgb(colors::muted()))
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
            .border_color(rgb(colors::hairline()))
            .children(
                snapshot
                    .reading_list
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, entry)| self.render_reading_list_row(index, snapshot, entry, cx)),
            )
            .into_any_element()
    }

    fn render_reading_list_row(
        &mut self,
        index: usize,
        snapshot: &BrowserSnapshot,
        entry: &ReadingListEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let url = entry.source_url().clone();
        let space_name = reading_list_space_name(snapshot, entry);
        let entry_id = entry.id().clone();
        let progress = *entry.progress();

        div()
            .id(SharedString::from(format!("reading-{}", entry.id().as_str())))
            .py_3()
            .border_b_1()
            .border_color(rgb(colors::hairline()))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .id(SharedString::from(format!("reading-open-{}", entry.id().as_str())))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(colors::canvas_soft())))
                    .active(|style| style.opacity(0.82))
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.open_url(url.clone(), window, cx);
                    }))
                    .child(div().text_color(rgb(colors::muted_soft())).child(IconName::Inbox))
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
                                    .child(entry.title().to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::muted()))
                                    .child(entry.display_url()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(rgb(colors::muted_soft()))
                                    .child(added_at_label(entry.added_at())),
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
                    .text_color(rgb(colors::muted()))
                    .when_some(space_name, |this, space_name| {
                        this.child(div().max_w(px(140.0)).truncate().child(space_name))
                    })
                    .child(progress_label(entry.progress())),
            )
            .child(render_progress_action(index, entry_id, progress, cx))
            .child(render_remove_action(index, entry.id().clone(), cx))
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
                .child(
                    div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Reading List"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::muted()))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(colors::muted()))
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

fn progress_label(progress: &ReadingProgress) -> String {
    progress.label()
}

fn render_progress_action(
    index: usize,
    entry_id: ReadingListId,
    progress: ReadingProgress,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let next_progress = progress.toggled();
    let icon = match progress {
        ReadingProgress::Unread | ReadingProgress::InProgress(_) => IconName::CircleCheck,
        ReadingProgress::Finished => IconName::Undo2,
    };

    Button::new(("reading-progress", index))
        .ghost()
        .xsmall()
        .icon(icon)
        .label(progress.action_label())
        .tooltip(progress.action_label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_reading_list_progress(&entry_id, next_progress, cx);
        }))
        .into_any_element()
}

fn render_remove_action(
    index: usize,
    entry_id: ReadingListId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("remove-reading-list", index))
        .danger()
        .xsmall()
        .icon(IconName::Delete)
        .label("Remove")
        .tooltip("Remove From Reading List")
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.remove_reading_list_entry(&entry_id, cx);
        }))
        .into_any_element()
}

fn reading_list_count_label(count: usize) -> String {
    match count {
        1 => "1 item".to_string(),
        count => format!("{count} items"),
    }
}

fn added_at_label(added_at: SystemTime) -> String {
    added_at_label_for(added_at, SystemTime::now())
}

fn added_at_label_for(added_at: SystemTime, now: SystemTime) -> String {
    let age = now.duration_since(added_at).unwrap_or(Duration::ZERO);
    if age < Duration::from_secs(60) {
        return "Added just now".to_string();
    }
    if age < Duration::from_secs(3_600) {
        return format!("Added {} mins ago", age.as_secs() / 60);
    }
    if age < Duration::from_secs(86_400) {
        return format!("Added {} hrs ago", age.as_secs() / 3_600);
    }
    if age < Duration::from_secs(604_800) {
        return format!("Added {} days ago", age.as_secs() / 86_400);
    }
    "Added earlier".to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::added_at_label_for;

    #[test]
    fn added_at_label_formats_recent_entries() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);

        assert_eq!(added_at_label_for(now - Duration::from_secs(20), now), "Added just now");
    }

    #[test]
    fn added_at_label_formats_minutes_hours_days_and_older_entries() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);

        assert_eq!(added_at_label_for(now - Duration::from_secs(120), now), "Added 2 mins ago");
        assert_eq!(added_at_label_for(now - Duration::from_secs(10_800), now), "Added 3 hrs ago");
        assert_eq!(added_at_label_for(now - Duration::from_secs(345_600), now), "Added 4 days ago");
        assert_eq!(added_at_label_for(now - Duration::from_secs(691_200), now), "Added earlier");
    }
}
