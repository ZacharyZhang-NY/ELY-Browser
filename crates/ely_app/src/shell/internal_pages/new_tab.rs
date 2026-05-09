use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::ElyShell;

struct QuickAction {
    label: &'static str,
    route: &'static str,
    icon: IconName,
}

const QUICK_ACTIONS: &[QuickAction] = &[
    QuickAction { label: "History", route: "ely://history", icon: IconName::Undo2 },
    QuickAction { label: "Bookmarks", route: "ely://bookmarks", icon: IconName::BookOpen },
    QuickAction { label: "Downloads", route: "ely://downloads", icon: IconName::Folder },
    QuickAction { label: "Plugins", route: "ely://plugins", icon: IconName::Asterisk },
    QuickAction { label: "Settings", route: "ely://settings", icon: IconName::Settings2 },
];

impl ElyShell {
    pub(super) fn render_new_tab_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_1()
            .h_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .size_full()
                    .p(px(48.0))
                    .flex()
                    .flex_col()
                    .gap(px(24.0))
                    .child(render_hero_section())
                    .child(render_quick_actions(cx))
                    .child(render_favorites_grid(snapshot, cx))
                    .child(render_tabs_section(snapshot, cx)),
            )
            .into_any_element()
    }
}

fn render_hero_section() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(20.0))
        .pt(px(24.0))
        .pb(px(12.0))
        .child(
            div()
                .text_size(px(48.0))
                .font_weight(gpui::FontWeight(400.0))
                .text_color(rgb(colors::INK))
                .child("Where focus finds flow."),
        )
        .child(
            div()
                .w(px(420.0))
                .h(px(44.0))
                .rounded(px(999.0))
                .bg(rgba(colors::GLASS_2))
                .px(px(16.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(div().text_color(rgb(colors::INK_4)).child(IconName::Search))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(colors::INK_4))
                        .child("Search the web or ELY"),
                ),
        )
        .into_any_element()
}

fn render_quick_actions(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .children(QUICK_ACTIONS.iter().enumerate().map(|(index, action)| {
            let route = action.route;
            div()
                .id(SharedString::from(format!("quick-action-{index}")))
                .rounded(px(999.0))
                .bg(rgba(colors::GLASS_2))
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .hover(|style| style.bg(rgba(colors::GLASS_3)))
                .active(|style| style.opacity(0.82))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.open_internal_tab(route, window, cx);
                }))
                .child(
                    div()
                        .text_color(rgb(colors::INK_3))
                        .text_size(px(12.0))
                        .child(action.icon.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight(500.0))
                        .text_color(rgb(colors::INK_2))
                        .child(action.label),
                )
        }))
        .into_any_element()
}

fn render_favorites_grid(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.favorites.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(gpui::FontWeight(500.0))
                .text_color(rgb(colors::INK_4))
                .child("FAVORITES"),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(10.0))
                .children(snapshot.favorites.iter().enumerate().map(|(index, tab)| {
                    render_favorite_card(index, tab, cx)
                })),
        )
        .into_any_element()
}

fn render_favorite_card(
    index: usize,
    tab: &BrowserTab,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();
    let initial = tab
        .title()
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    div()
        .id(SharedString::from(format!("fav-card-{index}")))
        .w(px(96.0))
        .h(px(96.0))
        .rounded(px(14.0))
        .bg(rgba(colors::GLASS_2))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(colors::GLASS_3)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(
            div()
                .size(px(36.0))
                .rounded(px(10.0))
                .bg(rgb(colors::ACCENT))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_semibold()
                        .text_color(rgb(colors::SURFACE_CARD))
                        .child(initial),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_2))
                .max_w(px(80.0))
                .truncate()
                .child(tab.title().to_string()),
        )
        .into_any_element()
}

fn render_tabs_section(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.tabs.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_weight(gpui::FontWeight(500.0))
                        .text_color(rgb(colors::INK_4))
                        .child("OPEN TABS"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::INK_4))
                        .child(format!("{}", snapshot.tabs.len())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .children(snapshot.tabs.iter().enumerate().map(|(index, tab)| {
                    render_tab_row(index, tab, &snapshot.active_tab_id, cx)
                })),
        )
        .into_any_element()
}

fn render_tab_row(
    index: usize,
    tab: &BrowserTab,
    active_tab_id: &TabId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();
    let active = tab.id() == active_tab_id;
    let bg = if active { colors::GLASS_3 } else { 0x00000000 };

    div()
        .id(SharedString::from(format!("tab-row-{index}")))
        .rounded(px(8.0))
        .px(px(10.0))
        .py(px(8.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .bg(rgba(bg))
        .hover(|style| style.bg(rgba(colors::GLASS_3)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(
            div()
                .text_color(if active { rgb(colors::ACCENT) } else { rgb(colors::INK_4) })
                .child(IconName::Globe),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight(500.0))
                        .text_color(rgb(colors::INK))
                        .truncate()
                        .child(tab.title().to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::INK_4))
                        .truncate()
                        .child(tab.display_url()),
                ),
        )
        .into_any_element()
}
