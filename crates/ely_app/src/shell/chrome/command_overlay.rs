use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BookmarkEntry, BrowserTab, HistoryEntry};
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, hsla, point,
    prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::command_footer::{render_command_footer, render_kbd};
use crate::shell::chrome::command_match::{
    CommandActionEntry, matching_actions, matching_bookmarks, matching_history, matching_tabs,
};
use crate::shell::chrome::render_glyph_for;

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
    div()
        .absolute()
        .inset_0()
        .bg(rgba(BACKDROP_BG))
        .flex()
        .flex_col()
        .items_center()
        .pt(px(80.0))
        .child(render_panel(snapshot, needle, selected_index, cx))
        .into_any_element()
}

fn render_panel(
    snapshot: &BrowserSnapshot,
    needle: &str,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let query_label = if needle.is_empty() {
        "Type to search…".to_string()
    } else {
        needle.to_string()
    };

    div()
        .w(px(640.0))
        .rounded(px(16.0))
        .bg(rgba(PANEL_BG))
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
    let label_color = if is_empty { colors::INK_4 } else { colors::INK };

    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(18.0))
        .py(px(16.0))
        .border_b_1()
        .border_color(rgba(colors::DIVIDER))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(IconName::Search),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(16.0))
                .text_color(rgb(label_color))
                .child(query_label),
        )
        .child(
            div()
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(6.0))
                .bg(rgba(BADGE_BG))
                .text_size(px(10.5))
                .text_color(rgb(colors::INK_3))
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
        sections.push(render_section(
            "Open tabs",
            render_tab_rows(tabs, offset, selected_index, cx),
        ));
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

    div()
        .max_h(px(440.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .children(sections)
        .into_any_element()
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
                .text_color(rgb(colors::INK_4))
                .child(label),
        )
        .child(body)
        .into_any_element()
}

fn render_tab_rows(
    tabs: Vec<&BrowserTab>,
    offset: usize,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .children(tabs.into_iter().enumerate().map(|(index, tab)| {
            let tab_id = tab.id().clone();
            let title = tab.title().to_string();
            let host = tab.url().host().map(|host| host.to_string());
            let host_label = host
                .clone()
                .unwrap_or_else(|| tab.display_url());
            let initial = title.chars().next().unwrap_or('?').to_string();
            let is_selected = offset + index == selected_index;

            render_row_with_glyph(
                CommandRowContent {
                    id: format!("cmd-tab-{index}"),
                    title,
                    hint: Some(host_label),
                    keys: None,
                    selected: is_selected,
                },
                host.as_deref(),
                &initial,
                cx,
                move |shell, window, cx| {
                    shell.select_tab(&tab_id, window, cx);
                    shell.dismiss_command_mode(window, cx);
                },
            )
        }))
        .into_any_element()
}

fn render_history_rows(
    entries: Vec<&HistoryEntry>,
    offset: usize,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .children(entries.into_iter().enumerate().map(|(index, entry)| {
            let url = entry.url().clone();
            let title = entry.title().to_string();
            let host = entry.url().host().map(|host| host.to_string());
            let display = host
                .clone()
                .unwrap_or_else(|| entry.url().as_str().to_string());
            let initial = title.chars().next().unwrap_or('?').to_string();
            let is_selected = offset + index == selected_index;

            render_row_with_glyph(
                CommandRowContent {
                    id: format!("cmd-history-{index}"),
                    title,
                    hint: Some(display),
                    keys: None,
                    selected: is_selected,
                },
                host.as_deref(),
                &initial,
                cx,
                move |shell, window, cx| {
                    shell.open_internal_tab(url.as_str(), window, cx);
                    shell.dismiss_command_mode(window, cx);
                },
            )
        }))
        .into_any_element()
}

fn render_action_rows(
    actions: Vec<&'static CommandActionEntry>,
    offset: usize,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .children(actions.into_iter().enumerate().map(|(index, action)| {
            let route = action.route;
            let keys = if action.keys.is_empty() {
                None
            } else {
                Some(action.keys.to_string())
            };
            let icon = action.icon.clone();
            let is_selected = offset + index == selected_index;

            render_row(
                CommandRowContent {
                    id: format!("cmd-action-{index}"),
                    title: action.title.to_string(),
                    hint: Some(action.hint.to_string()),
                    keys,
                    selected: is_selected,
                },
                icon,
                cx,
                move |shell, window, cx| {
                    shell.open_internal_tab(route, window, cx);
                    shell.dismiss_command_mode(window, cx);
                },
            )
        }))
        .into_any_element()
}

struct CommandRowContent {
    id: String,
    title: String,
    hint: Option<String>,
    keys: Option<String>,
    selected: bool,
}

fn render_row<F>(
    content: CommandRowContent,
    icon: IconName,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let leading = div()
        .size(px(24.0))
        .rounded(px(6.0))
        .bg(rgba(ROW_ICON_BG))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::INK_2))
        .child(icon)
        .into_any_element();
    render_row_inner(content, leading, cx, handler)
}

fn render_row_with_glyph<F>(
    content: CommandRowContent,
    host: Option<&str>,
    fallback_initial: &str,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let leading = render_glyph_for(host, fallback_initial, 24.0);
    render_row_inner(content, leading, cx, handler)
}

fn render_row_inner<F>(
    content: CommandRowContent,
    leading: AnyElement,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let CommandRowContent { id, title, hint, keys, selected } = content;
    let bg = if selected { ROW_SELECTED_BG } else { 0x00000000 };

    div()
        .id(SharedString::from(id))
        .relative()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(16.0))
        .py(px(8.0))
        .bg(rgba(bg))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(ROW_HOVER_BG)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(6.0))
                    .bottom(px(6.0))
                    .w(px(2.0))
                    .rounded(px(2.0))
                    .bg(rgb(colors::ACCENT)),
            )
        })
        .child(leading)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::INK))
                        .truncate()
                        .child(title),
                )
                .children(hint.map(|hint| {
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::INK_4))
                        .truncate()
                        .child(hint)
                })),
        )
        .children(keys.map(|key_label| {
            div()
                .text_size(px(10.5))
                .text_color(rgb(colors::INK_3))
                .child(key_label)
        }))
        .into_any_element()
}

fn render_empty_state() -> AnyElement {
    div()
        .px(px(18.0))
        .py(px(20.0))
        .text_size(px(12.5))
        .text_color(rgb(colors::INK_4))
        .child("No matches yet — keep typing.")
        .into_any_element()
}

fn render_bookmark_rows(
    entries: Vec<&BookmarkEntry>,
    offset: usize,
    selected_index: usize,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .children(entries.into_iter().enumerate().map(|(index, bookmark)| {
            let url = bookmark.url().clone();
            let title = bookmark.title().to_string();
            let host = bookmark.url().host().map(|host| host.to_string());
            let display = host
                .clone()
                .unwrap_or_else(|| bookmark.url().as_str().to_string());
            let initial = title.chars().next().unwrap_or('?').to_string();
            let is_selected = offset + index == selected_index;

            render_row_with_glyph(
                CommandRowContent {
                    id: format!("cmd-bookmark-{index}"),
                    title,
                    hint: Some(display),
                    keys: None,
                    selected: is_selected,
                },
                host.as_deref(),
                &initial,
                cx,
                move |shell, window, cx| {
                    shell.open_internal_tab(url.as_str(), window, cx);
                    shell.dismiss_command_mode(window, cx);
                },
            )
        }))
        .into_any_element()
}

const PANEL_BG: u32 = 0xfffffff5;
const BACKDROP_BG: u32 = 0x140f0a3d;
const ROW_HOVER_BG: u32 = 0xc9644214;
const ROW_SELECTED_BG: u32 = 0xc964421f;
const ROW_ICON_BG: u32 = 0xffffffd9;
const BADGE_BG: u32 = 0x281e140f;

fn panel_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(25.0 / 360.0, 0.33, 0.12, 0.45),
        offset: point(px(0.0), px(30.0)),
        blur_radius: px(80.0),
        spread_radius: px(-20.0),
    }]
}
