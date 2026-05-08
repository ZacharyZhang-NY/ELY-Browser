use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{BrowserTab, TabId};
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

struct NewTabAction {
    label: &'static str,
    route: &'static str,
    icon: IconName,
}

const NEW_TAB_ACTIONS: &[NewTabAction] = &[
    NewTabAction { label: "History", route: "ely://history", icon: IconName::Undo2 },
    NewTabAction { label: "Bookmarks", route: "ely://bookmarks", icon: IconName::BookOpen },
    NewTabAction { label: "Downloads", route: "ely://downloads", icon: IconName::Folder },
    NewTabAction { label: "Plugins", route: "ely://plugins", icon: IconName::Asterisk },
    NewTabAction { label: "Settings", route: "ely://settings", icon: IconName::Settings2 },
];

impl ElyShell {
    pub(super) fn render_new_tab_page(
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
                .gap_6()
                .child(render_new_tab_header(snapshot))
                .child(render_new_tab_actions(cx))
                .child(render_new_tab_metrics(snapshot))
                .child(render_new_tab_tab_list(snapshot, cx)),
        )
    }
}

fn render_new_tab_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap_5()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(30.0)).text_color(rgb(colors::INK)).child("New Tab"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("{} space", snapshot.active_space_name)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .px_3()
                .py_2()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(IconName::CircleUser)
                .child(snapshot.active_profile_name.clone()),
        )
        .into_any_element()
}

fn render_new_tab_actions(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .grid()
        .grid_cols(5)
        .gap_3()
        .children(
            NEW_TAB_ACTIONS
                .iter()
                .enumerate()
                .map(|(index, action)| render_new_tab_action(index, action, cx)),
        )
        .into_any_element()
}

fn render_new_tab_action(
    index: usize,
    action: &'static NewTabAction,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    Button::new(("new-tab-action", index))
        .ghost()
        .small()
        .icon(action.icon.clone())
        .label(action.label)
        .tooltip(action.label)
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(action.route, window, cx);
        }))
        .into_any_element()
}

fn render_new_tab_metrics(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .border_t_1()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .py_3()
        .grid()
        .grid_cols(4)
        .gap_4()
        .child(new_tab_metric("Open Tabs", snapshot.tabs.len()))
        .child(new_tab_metric("Favorites", snapshot.favorites.len()))
        .child(new_tab_metric("History", snapshot.active_profile_history_entry_count))
        .child(new_tab_metric("Downloads", snapshot.download_entries.len()))
        .into_any_element()
}

fn new_tab_metric(label: &'static str, value: usize) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div().text_sm().font_semibold().text_color(rgb(colors::INK)).child(value.to_string()),
        )
        .into_any_element()
}

fn render_new_tab_tab_list(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().font_semibold().text_color(rgb(colors::INK)).child("Tabs"))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("{} visible", snapshot.tabs.len())),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .children(snapshot.tabs.iter().enumerate().map(|(index, tab)| {
                    render_new_tab_tab_row(index, tab, &snapshot.active_tab_id, cx)
                })),
        )
        .into_any_element()
}

fn render_new_tab_tab_row(
    index: usize,
    tab: &BrowserTab,
    active_tab_id: &TabId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();
    let active = tab.id() == active_tab_id;
    let icon = if active { IconName::CircleCheck } else { IconName::Globe };
    let icon_color = if active { colors::PRIMARY } else { colors::MUTED_SOFT };

    div()
        .id(SharedString::from(format!("new-tab-row-{index}")))
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
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(icon_color)).child(icon))
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
                                .child(tab.title().to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(tab.display_url()),
                        ),
                ),
        )
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(if active {
            "Active"
        } else {
            "Open"
        }))
        .into_any_element()
}
