use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BrowserTab, HistoryEntry};
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, hsla, point, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::command_footer::{render_command_footer, render_kbd};
use crate::shell::chrome::render_glyph_for;

const COMMAND_PREFIX: &str = ">";
const RESULT_LIMIT: usize = 4;

pub(crate) fn render_command_overlay(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> Option<AnyElement> {
    let query = snapshot.command_query.as_str();
    if !query.starts_with(COMMAND_PREFIX) {
        return None;
    }
    let needle = query[COMMAND_PREFIX.len()..].trim().to_lowercase();

    Some(render_overlay(snapshot, &needle, cx))
}

fn render_overlay(
    snapshot: &BrowserSnapshot,
    needle: &str,
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
        .child(render_panel(snapshot, needle, cx))
        .into_any_element()
}

fn render_panel(
    snapshot: &BrowserSnapshot,
    needle: &str,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let query_label = if needle.is_empty() {
        "Type to search…".to_string()
    } else {
        needle.to_string()
    };

    div()
        .w(px(640.0))
        .max_h(px(540.0))
        .rounded(px(16.0))
        .bg(rgba(PANEL_BG))
        .shadow(panel_shadow())
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(render_header(query_label.clone(), needle.is_empty()))
        .child(render_results(snapshot, needle, cx))
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
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tabs = matching_tabs(snapshot, needle);
    let history = matching_history(snapshot, needle);
    let actions = matching_actions(needle);

    let mut sections: Vec<AnyElement> = Vec::new();
    if !tabs.is_empty() {
        sections.push(render_section("Open tabs", render_tab_rows(tabs, cx)));
    }
    if !history.is_empty() {
        sections.push(render_section("History", render_history_rows(history, cx)));
    }
    if !actions.is_empty() {
        sections.push(render_section("Actions", render_action_rows(actions, cx)));
    }

    if sections.is_empty() {
        sections.push(render_empty_state());
    }

    div()
        .flex_1()
        .min_h_0()
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

fn render_tab_rows(tabs: Vec<&BrowserTab>, cx: &mut Context<ElyShell>) -> AnyElement {
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

            render_row_with_glyph(
                CommandRowContent {
                    id: format!("cmd-tab-{index}"),
                    title,
                    hint: Some(host_label),
                    keys: None,
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

            render_row_with_glyph(
                CommandRowContent {
                    id: format!("cmd-history-{index}"),
                    title,
                    hint: Some(display),
                    keys: None,
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

struct CommandAction {
    title: &'static str,
    hint: &'static str,
    icon: IconName,
    route: &'static str,
    keys: &'static str,
}

const COMMAND_ACTIONS: &[CommandAction] = &[
    CommandAction {
        title: "Switch workspace",
        hint: "Cycle to the next space",
        icon: IconName::LayoutDashboard,
        route: "ely://settings/spaces",
        keys: "⌘⇧W",
    },
    CommandAction {
        title: "Open Settings",
        hint: "Appearance, sync, plugins…",
        icon: IconName::Settings,
        route: "ely://settings",
        keys: "⌘,",
    },
    CommandAction {
        title: "Open Plugins",
        hint: "Marketplace and installed plugins",
        icon: IconName::Asterisk,
        route: "ely://plugins",
        keys: "",
    },
    CommandAction {
        title: "Open Bookmarks",
        hint: "Manage your saved pages",
        icon: IconName::BookOpen,
        route: "ely://bookmarks",
        keys: "",
    },
];

fn matching_actions(needle: &str) -> Vec<&'static CommandAction> {
    COMMAND_ACTIONS
        .iter()
        .filter(|action| {
            needle.is_empty()
                || action.title.to_lowercase().contains(needle)
                || action.hint.to_lowercase().contains(needle)
        })
        .collect()
}

fn render_action_rows(
    actions: Vec<&'static CommandAction>,
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

            render_row(
                CommandRowContent {
                    id: format!("cmd-action-{index}"),
                    title: action.title.to_string(),
                    hint: Some(action.hint.to_string()),
                    keys,
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
    let CommandRowContent { id, title, hint, keys } = content;

    div()
        .id(SharedString::from(id))
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(16.0))
        .py(px(8.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(ROW_HOVER_BG)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
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

fn matching_tabs<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a BrowserTab> {
    if needle.is_empty() {
        return snapshot.tabs.iter().take(RESULT_LIMIT).collect();
    }

    snapshot
        .tabs
        .iter()
        .filter(|tab| matches_tab(tab, needle))
        .take(RESULT_LIMIT)
        .collect()
}

fn matches_tab(tab: &BrowserTab, needle: &str) -> bool {
    tab.title().to_lowercase().contains(needle) || tab.url().as_str().to_lowercase().contains(needle)
}

fn matching_history<'a>(
    snapshot: &'a BrowserSnapshot,
    needle: &str,
) -> Vec<&'a HistoryEntry> {
    if needle.is_empty() {
        return snapshot.history_entries.iter().rev().take(RESULT_LIMIT).collect();
    }

    snapshot
        .history_entries
        .iter()
        .rev()
        .filter(|entry| matches_history(entry, needle))
        .take(RESULT_LIMIT)
        .collect()
}

fn matches_history(entry: &HistoryEntry, needle: &str) -> bool {
    entry.title().to_lowercase().contains(needle)
        || entry.url().as_str().to_lowercase().contains(needle)
}

const PANEL_BG: u32 = 0xfffffff5;
const BACKDROP_BG: u32 = 0x140f0a3d;
const ROW_HOVER_BG: u32 = 0xc9644214;
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
