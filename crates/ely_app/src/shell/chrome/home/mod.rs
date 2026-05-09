use std::time::SystemTime;

use ely_browser_core::BrowserSnapshot;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::scroll::ScrollableElement;

use crate::shell::ElyShell;

mod favorites;
mod hero;
mod recap;
mod section;
mod style;
mod time;

pub(crate) fn render_home_page(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let greeting = time::greeting_for_now(SystemTime::now(), &snapshot.active_profile_name);

    div()
        .flex_1()
        .h_full()
        .overflow_y_scrollbar()
        .child(
            div()
                .size_full()
                .pt(px(48.0))
                .px(px(64.0))
                .pb(px(28.0))
                .flex()
                .flex_col()
                .gap(px(40.0))
                .child(hero::render_hero(greeting, cx))
                .child(favorites::render_favorites_grid(snapshot, cx))
                .child(recap::render_recap(snapshot, cx)),
        )
        .into_any_element()
}
