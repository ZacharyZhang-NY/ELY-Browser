use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::BrowserTab;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
};

use super::{
    ElyShell, render_canvas_surface,
    tab_context::{favicon_status, profile_name, space_name, tab_context_row},
};

impl ElyShell {
    pub(super) fn render_sleep_page(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(render_sleep_content(tab, snapshot, cx))
    }
}

fn render_sleep_content(
    tab: &BrowserTab,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let tab_id = tab.id().clone();

    div()
        .size_full()
        .p_8()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_sleep_header(tab, cx))
        .child(
            div()
                .border_t_1()
                .border_b_1()
                .border_color(rgb(colors::HAIRLINE))
                .py_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(tab_context_row("URL", tab.url().as_str().to_string()))
                .child(tab_context_row("Title", tab.title().to_string()))
                .child(tab_context_row("Favicon", favicon_status(tab)))
                .child(tab_context_row("Space", space_name(snapshot, tab)))
                .child(tab_context_row("Profile", profile_name(snapshot, tab)))
                .child(tab_context_row(
                    "Session",
                    "Page session remains attached to this tab.".to_string(),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_sm()
                .text_color(rgb(colors::MUTED))
                .child(IconName::CircleCheck)
                .child("This tab is sleeping. Restore wakes the tab and keeps its saved context."),
        )
        .child(
            div().flex().child(
                Button::new("wake-discarded-tab")
                    .primary()
                    .small()
                    .icon(IconName::Undo2)
                    .label("Restore")
                    .tooltip("Restore sleeping tab")
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.wake_discarded_tab(&tab_id, window, cx);
                    })),
            ),
        )
        .into_any_element()
}

fn render_sleep_header(tab: &BrowserTab, cx: &mut Context<ElyShell>) -> AnyElement {
    let tab_id = tab.id().clone();

    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::INK))
                        .child(IconName::CircleCheck)
                        .child("Sleeping Tab"),
                )
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Sleeping {}", tab.display_url())),
                ),
        )
        .child(
            Button::new("wake-discarded-tab-header")
                .ghost()
                .small()
                .icon(IconName::Undo2)
                .label("Restore")
                .tooltip("Restore sleeping tab")
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.wake_discarded_tab(&tab_id, window, cx);
                })),
        )
        .into_any_element()
}
