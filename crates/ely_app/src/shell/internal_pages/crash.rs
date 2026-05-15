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
    pub(super) fn render_crash_page(
        &mut self,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(render_crash_content(tab, snapshot, cx))
    }

    pub(super) fn render_crash_route(
        &mut self,
        snapshot: &BrowserSnapshot,
        url: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(tab_id) = crash_route_tab_id(url) else {
            return render_missing_crash_route("Crash route is invalid.");
        };

        let Some(tab) = snapshot.tabs.iter().find(|tab| tab.id().as_str() == tab_id) else {
            return render_missing_crash_route("Tab could not be found.");
        };

        render_canvas_surface(render_crash_content(tab, snapshot, cx))
    }
}

fn render_crash_content(
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
        .child(render_crash_header(tab, cx))
        .child(
            div()
                .border_t_1()
                .border_b_1()
                .border_color(rgb(colors::hairline()))
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
                    "Form restore prompt",
                    "Session data remains attached to this tab.".to_string(),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_sm()
                .text_color(rgb(colors::muted()))
                .child(IconName::TriangleAlert)
                .child("The renderer stopped for this tab. Restore reloads the saved tab state."),
        )
        .child(
            div().flex().child(
                Button::new("restore-crashed-tab")
                    .primary()
                    .small()
                    .icon(IconName::Undo2)
                    .label("Restore")
                    .tooltip("Restore tab")
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.recover_crashed_tab(&tab_id, window, cx);
                    })),
            ),
        )
        .into_any_element()
}

fn render_crash_header(tab: &BrowserTab, cx: &mut Context<ElyShell>) -> AnyElement {
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
                        .text_color(rgb(colors::ink()))
                        .child(IconName::TriangleAlert)
                        .child("Tab Recovery"),
                )
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::muted()))
                        .child(format!("Recovering {}", tab.display_url())),
                ),
        )
        .child(
            Button::new("restore-crashed-tab-header")
                .ghost()
                .small()
                .icon(IconName::Undo2)
                .label("Restore")
                .tooltip("Restore tab")
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.recover_crashed_tab(&tab_id, window, cx);
                })),
        )
        .into_any_element()
}

fn render_missing_crash_route(message: &'static str) -> AnyElement {
    render_canvas_surface(
        div()
            .size_full()
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_size(px(26.0)).text_color(rgb(colors::ink())).child("Tab Recovery"))
            .child(div().text_sm().text_color(rgb(colors::muted())).child(message)),
    )
}

fn crash_route_tab_id(url: &str) -> Option<&str> {
    let tab_id = url.strip_prefix("ely://crash/")?;
    (!tab_id.is_empty() && !tab_id.contains('/')).then_some(tab_id)
}
