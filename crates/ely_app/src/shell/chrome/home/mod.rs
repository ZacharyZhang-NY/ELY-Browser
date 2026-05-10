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
    shell: &ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let now = SystemTime::now();
    let greeting = time::greeting_for_now(now, &snapshot.active_profile_name);
    let phase = time::day_phase_for(now);

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
                .child(hero::render_hero(shell, greeting, phase, cx))
                .child(div().mt(px(48.0)).child(favorites::render_favorites_grid(snapshot, cx)))
                .child(div().mt(px(16.0)).child(recap::render_recap(snapshot, cx))),
        )
        .into_any_element()
}
