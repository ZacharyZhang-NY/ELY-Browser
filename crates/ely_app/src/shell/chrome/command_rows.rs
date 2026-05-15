use ely_design_system::colors;
use ely_domain::{BookmarkEntry, BrowserTab, HistoryEntry};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::command_match::CommandActionEntry;
use crate::shell::chrome::render_glyph_for;

pub(crate) struct CommandRowContent {
    pub id: String,
    pub title: String,
    pub hint: Option<String>,
    pub keys: Option<String>,
    pub selected: bool,
}

pub(crate) fn render_tab_rows(
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
            let host_label = host.clone().unwrap_or_else(|| tab.display_url());
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

pub(crate) fn render_history_rows(
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
            let display = host.clone().unwrap_or_else(|| entry.url().as_str().to_string());
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

pub(crate) fn render_bookmark_rows(
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
            let display = host.clone().unwrap_or_else(|| bookmark.url().as_str().to_string());
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

pub(crate) fn render_action_rows(
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
            let keys = if action.keys.is_empty() { None } else { Some(action.keys.to_string()) };
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
                    div().text_size(px(11.0)).text_color(rgb(colors::INK_4)).truncate().child(hint)
                })),
        )
        .children(keys.map(|key_label| {
            div().text_size(px(10.5)).text_color(rgb(colors::INK_3)).child(key_label)
        }))
        .into_any_element()
}

const ROW_HOVER_BG: u32 = 0xc9644214;
const ROW_SELECTED_BG: u32 = 0xc964421f;
const ROW_ICON_BG: u32 = 0xffffffd9;
