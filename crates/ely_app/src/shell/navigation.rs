use ely_domain::UrlText;
use gpui::{Context, Window};
use gpui_component::input::SelectAll;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn active_tab_matches_url(&self, url: &str) -> bool {
        match &self.state {
            ShellState::Ready(core) => core.active_tab().is_ok_and(|tab| tab.url().as_str() == url),
            ShellState::StartupError(_) => false,
        }
    }

    pub(super) fn open_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.open_new_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    pub(super) fn open_downloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://downloads", window, cx);
    }

    pub(super) fn open_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://history", window, cx);
    }

    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://settings", window, cx);
    }

    pub(super) fn open_task_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://task-manager", window, cx);
    }

    pub(super) fn open_internal_tab(
        &mut self,
        url_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Ok(url) = UrlText::parse(url_text) {
            self.open_url(url, window, cx);
        }
    }

    pub(crate) fn open_url(&mut self, url: UrlText, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.open_tab(url);
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    pub(super) fn focus_address_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        window.dispatch_action(Box::new(SelectAll), cx);
    }
}
