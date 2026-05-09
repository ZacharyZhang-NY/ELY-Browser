use ely_design_system::colors;
use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::render_glyph_for;

pub(crate) fn render_split_pane_header(
    host: String,
    path: String,
    secure: bool,
    glyph_initial: String,
    close_tab_id: TabId,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let lock_or_globe = if secure { IconName::Search } else { IconName::Globe };

    div()
        .h(px(32.0))
        .px(px(10.0))
        .gap(px(8.0))
        .flex()
        .items_center()
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(0xf6f4ef))
        .child(render_glyph_for(Some(host.as_str()), &glyph_initial, 16.0))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .text_size(px(11.0))
                .child(lock_or_globe),
        )
        .child(
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::INK))
                .child(host),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_4))
                .child(path),
        )
        .child(render_reload_glyph(close_tab_id.clone(), cx))
        .child(render_close_glyph(close_tab_id, cx))
        .into_any_element()
}

fn render_reload_glyph(tab_id: TabId, cx: &mut Context<ElyShell>) -> AnyElement {
    let id = format!("split-pane-reload-{}", tab_id.as_str());
    div()
        .id(SharedString::from(id))
        .text_color(rgb(colors::INK_4))
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|style| style.text_color(rgb(colors::INK)))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(IconName::Redo2)
        .into_any_element()
}

fn render_close_glyph(close_tab_id: TabId, cx: &mut Context<ElyShell>) -> AnyElement {
    let id = format!("split-pane-close-{}", close_tab_id.as_str());
    div()
        .id(SharedString::from(id))
        .text_color(rgb(colors::INK_4))
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|style| style.text_color(rgb(colors::INK)))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&close_tab_id, window, cx);
            shell.close_active_tab(window, cx);
        }))
        .child(IconName::Close)
        .into_any_element()
}

pub(crate) fn pane_host_label(tab: &BrowserTab) -> String {
    tab.url()
        .host()
        .map(|host| host.to_string())
        .unwrap_or_else(|| tab.title().to_string())
}

pub(crate) fn pane_path_label(tab: &BrowserTab) -> String {
    let url = tab.url().as_str();
    let path_start = url.find("://").map(|prefix| prefix + 3).unwrap_or(0);
    let from_path = &url[path_start..];
    match from_path.find('/') {
        Some(slash) => from_path[slash..].to_string(),
        None => String::new(),
    }
}

pub(crate) fn pane_url_is_secure(tab: &BrowserTab) -> bool {
    let url = tab.url().as_str();
    url.starts_with("https://") || url.starts_with("ely://")
}

pub(crate) fn split_canvas_status(tab: &BrowserTab) -> String {
    if tab.url().as_str() == "ely://new-tab" {
        "Ready".to_string()
    } else {
        tab.display_url()
    }
}

pub(crate) fn render_compact_split_canvas(tab: &BrowserTab) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .p(px(8.0))
        .child(
            div()
                .size_full()
                .min_h_0()
                .rounded(px(10.0))
                .bg(rgba(colors::GLASS_2))
                .px(px(10.0))
                .py(px(8.0))
                .gap(px(8.0))
                .flex()
                .items_center()
                .child(div().text_color(rgb(colors::INK_4)).child(IconName::Globe))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.0))
                                .font_weight(FontWeight(500.0))
                                .child(tab.title().to_string()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.0))
                                .text_color(rgb(colors::INK_4))
                                .child(split_canvas_status(tab)),
                        ),
                ),
        )
        .into_any_element()
}
