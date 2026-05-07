use ely_browser_core::{BrowserCore, BrowserSnapshot, InitialBrowserConfig};
use ely_design_system::{ELY_THEME, colors, spacing};
use ely_domain::{BrowserTab, CommandIntent, TabId, UrlText};
use gpui::{
    AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px, rgb,
};
use gpui_component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};

enum ShellState {
    Ready(BrowserCore),
    StartupError(String),
}

pub struct ElyShell {
    state: ShellState,
    command_input: Entity<InputState>,
    last_intent: Option<CommandIntent>,
    _command_subscription: Subscription,
}

impl ElyShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or enter address"));

        let command_subscription =
            cx.subscribe(&command_input, |shell: &mut Self, input, event: &InputEvent, cx| {
                let ShellState::Ready(core) = &mut shell.state else {
                    return;
                };

                let value = input.read(cx).value().to_string();
                core.set_command_query(value);

                if matches!(event, InputEvent::PressEnter { .. }) {
                    shell.last_intent = core.submit_command().ok().flatten();
                }

                cx.notify();
            });

        let state = match InitialBrowserConfig::ely_defaults().and_then(|config| {
            BrowserCore::new(config).map_err(|error| match error {
                ely_browser_core::CoreError::Domain(source) => source,
                _ => ely_domain::DomainError::InvalidCommand,
            })
        }) {
            Ok(core) => ShellState::Ready(core),
            Err(error) => ShellState::StartupError(error.to_string()),
        };

        Self {
            state,
            command_input,
            last_intent: None,
            _command_subscription: command_subscription,
        }
    }

    fn open_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(url) = UrlText::parse("ely://new-tab")
        {
            core.open_tab(url);
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_tab(&mut self, tab_id: &TabId, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn sync_address_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };

        if let Ok(active_tab) = core.active_tab() {
            let value = active_tab.url().as_str().to_string();
            self.command_input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
            });
        }
    }
}

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
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors::MUTED))
                    .child(tab.url().as_str().to_string()),
            )
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
