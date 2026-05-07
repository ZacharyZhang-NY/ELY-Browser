use ely_browser_core::BrowserSnapshot;
use ely_design_system::{ELY_THEME, colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use gpui_component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};

use super::{ElyShell, ShellState};

impl Render for ElyShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.state {
            ShellState::Ready(core) => match (core.snapshot(), core.active_tab().cloned()) {
                (Ok(snapshot), Ok(active_tab)) => self.render_browser(snapshot, active_tab, cx),
                (Err(error), _) | (_, Err(error)) => render_error(error.to_string()),
            },
            ShellState::StartupError(message) => render_error(message.clone()),
        }
    }
}

impl ElyShell {
    fn render_browser(
        &mut self,
        snapshot: BrowserSnapshot,
        active_tab: BrowserTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_close_current_tab))
            .on_action(cx.listener(Self::on_focus_address_bar))
            .on_action(cx.listener(Self::on_open_new_tab))
            .on_action(cx.listener(Self::on_select_next_tab))
            .on_action(cx.listener(Self::on_select_previous_tab))
            .bg(rgb(ELY_THEME.canvas))
            .text_color(rgb(ELY_THEME.ink))
            .flex()
            .flex_col()
            .child(self.render_command_bar(&snapshot, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(&snapshot, cx))
                    .child(render_web_canvas(&active_tab)),
            )
            .into_any_element()
    }

    fn render_command_bar(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .h(px(spacing::COMMAND_BAR_HEIGHT))
            .px_4()
            .gap_3()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .child(
                div()
                    .w(px(spacing::SIDEBAR_WIDTH - spacing::XL))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(18.0)).font_semibold().child("ELY Browser"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors::MUTED))
                            .child(snapshot.active_space_name.clone()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(40.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(colors::HAIRLINE_STRONG))
                    .bg(rgb(colors::SURFACE_CARD))
                    .px_3()
                    .child(Input::new(&self.command_input).appearance(false).cleanable(true)),
            )
            .child(
                Button::new("new-tab")
                    .primary()
                    .small()
                    .label("+")
                    .tooltip("New Tab")
                    .on_click(cx.listener(|shell, _, window, cx| shell.open_new_tab(window, cx))),
            )
            .into_any_element()
    }

    fn render_sidebar(&mut self, snapshot: &BrowserSnapshot, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(spacing::SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_r_1()
            .border_color(rgb(colors::HAIRLINE))
            .bg(rgb(colors::CANVAS))
            .child(section_label("Favorites"))
            .child(empty_line())
            .child(section_label("Space"))
            .child(
                div()
                    .rounded_md()
                    .bg(rgb(colors::SURFACE_CARD))
                    .border_1()
                    .border_color(rgb(colors::HAIRLINE))
                    .px_3()
                    .py_2()
                    .child(snapshot.active_space_name.clone()),
            )
            .child(section_label("Tabs"))
            .children(
                snapshot
                    .tabs
                    .iter()
                    .map(|tab| self.render_tab_row(tab, tab.id() == &snapshot.active_tab_id, cx)),
            )
            .child(div().flex_1())
            .child(section_label("Profile"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(colors::BODY))
                    .child(snapshot.active_profile_name.clone()),
            )
            .into_any_element()
    }

    fn render_tab_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::HAIRLINE_STRONG } else { colors::HAIRLINE };

        div()
            .id(SharedString::from(tab.id().as_str().to_string()))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_1()
            .flex()
            .flex_col()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(colors::INK))
                    .child(tab.title().to_string()),
            )
            .child(div().text_xs().text_color(rgb(colors::MUTED)).child(tab.display_url()))
            .into_any_element()
    }
}

fn render_web_canvas(tab: &BrowserTab) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .p_6()
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .size_full()
                .rounded_lg()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .bg(rgb(colors::SURFACE_CARD))
                .p_8()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::INK))
                        .child(tab.title().to_string()),
                )
                .child(
                    div().text_sm().text_color(rgb(colors::MUTED)).child(render_tab_status(tab)),
                ),
        )
        .into_any_element()
}

fn render_error(message: String) -> AnyElement {
    div()
        .size_full()
        .bg(rgb(colors::CANVAS))
        .text_color(rgb(colors::ERROR))
        .flex()
        .items_center()
        .justify_center()
        .child(message)
        .into_any_element()
}

fn section_label(label: &'static str) -> impl IntoElement {
    div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child(label)
}

fn render_tab_status(tab: &BrowserTab) -> String {
    match tab.url().as_str() {
        "ely://new-tab" => "Ready".to_string(),
        url => url.to_string(),
    }
}

fn empty_line() -> impl IntoElement {
    div().h(px(34.0)).rounded_md().border_1().border_color(rgb(colors::HAIRLINE))
}
