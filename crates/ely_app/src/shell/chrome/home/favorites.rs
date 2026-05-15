use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::render_glyph_for;

use super::section::render_section_chevron_label;
use super::style::{ADD_TILE_BG, CARD_BG, CARD_BG_HOVER, card_shadow};

pub(crate) fn render_favorites_grid(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.favorites.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_section_chevron_label("Favorites"))
        .child(
            div()
                .grid()
                .grid_cols(7)
                .gap(px(10.0))
                .children(
                    snapshot
                        .favorites
                        .iter()
                        .enumerate()
                        .map(|(index, tab)| render_favorite_tile(index, tab, cx)),
                )
                .child(render_add_favorite_tile(cx)),
        )
        .into_any_element()
}

fn render_favorite_tile(index: usize, tab: &BrowserTab, cx: &mut Context<ElyShell>) -> AnyElement {
    let tab_id = tab.id().clone();
    let host = tab.url().host().map(|host| host.to_string());
    let title = tab.title().to_string();
    let initial = title.chars().next().unwrap_or('?').to_string();

    div()
        .id(SharedString::from(format!("fav-tile-{index}")))
        .h(px(96.0))
        .rounded(px(16.0))
        .bg(rgba(CARD_BG))
        .shadow(card_shadow())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CARD_BG_HOVER)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_tab(&tab_id, window, cx);
        }))
        .child(render_glyph_for(host.as_deref(), &initial, 28.0))
        .child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(colors::ink_3()))
                .max_w(px(80.0))
                .truncate()
                .child(title),
        )
        .into_any_element()
}

fn render_add_favorite_tile(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("fav-tile-add"))
        .h(px(96.0))
        .rounded(px(16.0))
        .bg(rgba(ADD_TILE_BG))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::ink_4()))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CARD_BG)))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.open_internal_tab("ely://bookmarks", window, cx);
        }))
        .child(IconName::Plus)
        .into_any_element()
}
