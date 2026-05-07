mod render;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, TabId, UrlText};
use gpui::{App, AppContext, Context, Entity, FocusHandle, Focusable, Subscription, Window};
use gpui_component::input::{InputEvent, InputState, SelectAll};

use crate::{
    CloseCurrentTab, FocusAddressBar, OpenNewTab, SelectNextTab, SelectPreviousTab,
    ToggleFavoriteTab, TogglePinnedTab,
};

enum ShellState {
    Ready(BrowserCore),
    StartupError(String),
}

pub struct ElyShell {
    state: ShellState,
    focus_handle: FocusHandle,
    command_input: Entity<InputState>,
    last_intent: Option<CommandIntent>,
    _command_subscription: Subscription,
}

impl ElyShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or enter address"));

        let command_subscription = cx.subscribe_in(
            &command_input,
            window,
            |shell: &mut Self, input, event: &InputEvent, window, cx| {
                let mut submitted_intent = None;
                let mut sync_address = false;
                let submitted = matches!(event, InputEvent::PressEnter { .. });

                let ShellState::Ready(core) = &mut shell.state else {
                    return;
                };

                let value = input.read(cx).value().to_string();
                core.set_command_query(value);

                if submitted {
                    submitted_intent = core.submit_command().ok().flatten();
                    sync_address = core.command_query().is_empty();
                }

                if submitted {
                    shell.last_intent = submitted_intent;
                }

                if sync_address {
                    shell.sync_address_input(window, cx);
                }

                cx.notify();
            },
        );

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
            focus_handle: cx.focus_handle(),
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
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    fn focus_address_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        window.dispatch_action(Box::new(SelectAll), cx);
    }

    fn select_tab(&mut self, tab_id: &TabId, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_next_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_previous_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.close_active_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn toggle_active_tab_favorite(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_favorite().is_ok()
        {
            cx.notify();
        }
    }

    fn toggle_active_tab_pinned(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_pinned().is_ok()
        {
            cx.notify();
        }
    }

    fn on_close_current_tab(
        &mut self,
        _: &CloseCurrentTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_tab(window, cx);
    }

    fn on_focus_address_bar(
        &mut self,
        _: &FocusAddressBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_address_bar(window, cx);
    }

    fn on_open_new_tab(&mut self, _: &OpenNewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.open_new_tab(window, cx);
    }

    fn on_select_next_tab(
        &mut self,
        _: &SelectNextTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_next_tab(window, cx);
    }

    fn on_select_previous_tab(
        &mut self,
        _: &SelectPreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_previous_tab(window, cx);
    }

    fn on_toggle_favorite_tab(
        &mut self,
        _: &ToggleFavoriteTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_favorite(cx);
    }

    fn on_toggle_pinned_tab(
        &mut self,
        _: &TogglePinnedTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_pinned(cx);
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

impl Focusable for ElyShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
