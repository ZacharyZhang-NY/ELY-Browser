use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, IntoElement, ParentElement, Styled, div, hsla,
    point, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::animations::{blink, fade_in};
use crate::shell::chrome::command_footer::{render_command_footer, render_kbd};
use crate::shell::chrome::command_match::{
    matching_actions, matching_bookmarks, matching_history, matching_tabs,
};
use crate::shell::chrome::command_rows::{
    render_action_rows, render_bookmark_rows, render_history_rows, render_tab_rows,
};

const COMMAND_PREFIX: &str = ">";

pub(crate) fn render_command_overlay(
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> Option<AnyElement> {
    let query = snapshot.command_query.as_str();
    if !query.starts_with(COMMAND_PREFIX) {
        return None;
    }
    let needle = query[COMMAND_PREFIX.len()..].trim().to_lowercase();
    let selected_index = shell.command_selected_index;

    Some(render_overlay(snapshot, &needle, selected_index, cx))
}

fn render_overlay(
    snapshot: &BrowserSnapshot,
    needle: &str,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    fade_in(
        "command-overlay-backdrop",
        160,
        div()
            .absolute()
            .inset_0()
            .bg(rgba(BACKDROP_BG))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.0))
            .child(render_panel(snapshot, needle, selected_index, cx)),
    )
    .into_any_element()
}

fn render_panel(
    snapshot: &BrowserSnapshot,
    needle: &str,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let query_label =
        if needle.is_empty() { "Type to search…".to_string() } else { needle.to_string() };

    div()
        .w(px(640.0))
        .rounded(px(16.0))
        .bg(rgba(PANEL_BG))
        .border_1()
        .border_color(rgba(PANEL_BORDER))
        .shadow(panel_shadow())
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(render_header(query_label.clone(), needle.is_empty()))
        .child(render_results(snapshot, needle, selected_index, cx))
        .child(render_command_footer())
        .into_any_element()
}

fn render_header(query_label: String, is_empty: bool) -> AnyElement {
    let label_color = if is_empty { colors::ink_4() } else { colors::ink() };

    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(18.0))
        .py(px(16.0))
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .child(div().text_color(rgb(colors::ink_3())).child(IconName::Search))
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(2.0))
                .text_size(px(16.0))
                .text_color(rgb(label_color))
                .child(query_label)
                .child(blink(
                    "command-overlay-caret",
                    div().w(px(1.0)).h(px(18.0)).bg(rgb(colors::accent())),
                )),
        )
        .child(
            div()
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(6.0))
                .bg(rgba(BADGE_BG))
                .text_size(px(10.5))
                .text_color(rgb(colors::ink_3()))
                .child("Switcher"),
        )
        .child(render_kbd("esc"))
        .into_any_element()
}

fn render_results(
    snapshot: &BrowserSnapshot,
    needle: &str,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tabs = matching_tabs(snapshot, needle);
    let history = matching_history(snapshot, needle);
    let bookmarks = matching_bookmarks(snapshot, needle);
    let actions = matching_actions(needle);

    let mut offset = 0usize;
    let mut sections: Vec<AnyElement> = Vec::new();
    if !tabs.is_empty() {
        let count = tabs.len();
        sections
            .push(render_section("Open tabs", render_tab_rows(tabs, offset, selected_index, cx)));
        offset += count;
    }
    if !history.is_empty() {
        let count = history.len();
        sections.push(render_section(
            "History",
            render_history_rows(history, offset, selected_index, cx),
        ));
        offset += count;
    }
    if !bookmarks.is_empty() {
        let count = bookmarks.len();
        sections.push(render_section(
            "Bookmarks",
            render_bookmark_rows(bookmarks, offset, selected_index, cx),
        ));
        offset += count;
    }
    if !actions.is_empty() {
        sections.push(render_section(
            "Actions",
            render_action_rows(actions, offset, selected_index, cx),
        ));
    }

    if sections.is_empty() {
        sections.push(render_empty_state());
    }

    div().max_h(px(440.0)).py(px(8.0)).flex().flex_col().children(sections).into_any_element()
}

fn render_section(label: &'static str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .pb(px(4.0))
        .pt(px(8.0))
        .child(
            div()
                .px(px(16.0))
                .pb(px(4.0))
                .pt(px(6.0))
                .text_size(px(10.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink_4()))
                .child(label),
        )
        .child(body)
        .into_any_element()
}

fn render_empty_state() -> AnyElement {
    div()
        .px(px(18.0))
        .py(px(20.0))
        .text_size(px(12.5))
        .text_color(rgb(colors::ink_4()))
        .child("No matches yet — keep typing.")
        .into_any_element()
}

const PANEL_BG: u32 = 0xfffffff5;
const PANEL_BORDER: u32 = 0xffffff80;
const BACKDROP_BG: u32 = 0x140f0a3d;
const BADGE_BG: u32 = 0x281e140f;

fn panel_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(25.0 / 360.0, 0.33, 0.12, 0.45),
        offset: point(px(0.0), px(30.0)),
        blur_radius: px(80.0),
        spread_radius: px(-20.0),
    }]
}
