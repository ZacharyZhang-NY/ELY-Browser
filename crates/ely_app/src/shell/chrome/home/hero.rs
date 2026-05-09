use ely_design_system::colors;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;

use super::style::{
    ARROW_CHIP_BG, PILL_BG, PILL_BG_HOVER, SEARCH_BG, card_shadow, soft_shadow,
};

pub(crate) fn render_hero(greeting: String, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(14.0))
        .child(render_greeting_row(greeting))
        .child(render_serif_headline())
        .child(render_search_bar(cx))
        .child(render_suggestion_pills(cx))
        .into_any_element()
}

fn render_greeting_row(text: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(rgb(colors::INK_3))
        .child(
            div()
                .text_color(rgb(colors::ACCENT_LIGHT))
                .child(IconName::Sun),
        )
        .child(text)
        .into_any_element()
}

fn render_serif_headline() -> AnyElement {
    div()
        .text_size(px(64.0))
        .font_weight(FontWeight(400.0))
        .text_color(rgb(colors::INK))
        .child("Where focus finds flow.")
        .into_any_element()
}

fn render_search_bar(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("home-search"))
        .w(px(640.0))
        .h(px(54.0))
        .rounded(px(14.0))
        .bg(rgba(SEARCH_BG))
        .shadow(card_shadow())
        .px(px(16.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.94))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.focus_address_bar(window, cx);
        }))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(IconName::Search),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(14.0))
                .text_color(rgb(colors::INK_4))
                .child("Search the web or ELY"),
        )
        .child(
            div()
                .size(px(28.0))
                .rounded(px(8.0))
                .bg(rgba(ARROW_CHIP_BG))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::INK_3))
                .child(IconName::ArrowRight),
        )
        .into_any_element()
}

fn render_suggestion_pills(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(render_pill(
            "pill-search-tabs",
            IconName::Search,
            "Search Tabs",
            cx,
            |shell, window, cx| shell.focus_address_bar(window, cx),
        ))
        .child(render_pill(
            "pill-switch-workspace",
            IconName::LayoutDashboard,
            "Switch Workspace",
            cx,
            |shell, window, cx| shell.cycle_to_next_space(window, cx),
        ))
        .child(render_pill(
            "pill-open-history",
            IconName::Undo2,
            "Open History",
            cx,
            |shell, window, cx| shell.open_internal_tab("ely://history", window, cx),
        ))
        .into_any_element()
}

fn render_pill<F>(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from(id))
        .rounded(px(999.0))
        .bg(rgba(PILL_BG))
        .shadow(soft_shadow())
        .px(px(12.0))
        .py(px(6.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(PILL_BG_HOVER)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .text_size(px(12.0))
                .child(icon),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::INK_2))
                .child(label),
        )
        .into_any_element()
}
