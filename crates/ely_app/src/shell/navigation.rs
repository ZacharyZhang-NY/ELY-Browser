use ely_domain::UrlText;
use gpui::{ClipboardItem, Context, Window};
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

    /// In-tab navigation. Used by every settings sidebar item, every
    /// home pill, and every "go to internal page" affordance — the
    /// active tab follows the link instead of spawning a fresh tab
    /// for every URL change. Use `open_url` only when the explicit
    /// intent is "spawn a new tab" (see `open_new_tab` for the
    /// keyboard shortcut path).
    pub(super) fn open_internal_tab(
        &mut self,
        url_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Ok(url) = UrlText::parse(url_text) {
            self.navigate_active_tab(url, window, cx);
        }
    }

    /// Navigate the active tab to `url` without creating a new tab.
    /// Falls back to opening a new tab only if there's no active tab
    /// to navigate (the BrowserCore returns `TabNotFound`).
    pub(crate) fn navigate_active_tab(
        &mut self,
        url: UrlText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            if core.navigate_active_tab(url.clone()).is_err() {
                core.open_tab(url);
            }
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    /// Spawn a fresh tab for `url`. Reserved for "+ New Tab" buttons
    /// and the deep-link router — anything that explicitly wants a
    /// new sibling tab rather than navigating in place.
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

    pub(super) fn copy_active_tab_url(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &self.state
            && let Ok(tab) = core.active_tab()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(tab.url().as_str().to_string()));
        }
    }

    pub(crate) fn dismiss_command_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_command_query(String::new());
        }

        self.command_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.command_selected_index = 0;
        cx.notify();
    }

    pub(crate) fn command_select_next(&mut self, total_rows: usize, cx: &mut Context<Self>) {
        if total_rows == 0 {
            self.command_selected_index = 0;
            return;
        }
        let next = (self.command_selected_index + 1) % total_rows;
        if next != self.command_selected_index {
            self.command_selected_index = next;
            cx.notify();
        }
    }

    pub(crate) fn command_select_prev(&mut self, total_rows: usize, cx: &mut Context<Self>) {
        if total_rows == 0 {
            self.command_selected_index = 0;
            return;
        }
        let prev = if self.command_selected_index == 0 {
            total_rows - 1
        } else {
            self.command_selected_index - 1
        };
        if prev != self.command_selected_index {
            self.command_selected_index = prev;
            cx.notify();
        }
    }

    pub(crate) fn activate_selected_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = match &self.state {
            ShellState::Ready(core) => match core.snapshot() {
                Ok(snapshot) => snapshot,
                Err(_) => return,
            },
            ShellState::StartupError(_) => return,
        };
        let needle_owned = snapshot.command_query.as_str();
        let Some(stripped) = needle_owned.strip_prefix('>') else {
            return;
        };
        let needle = stripped.trim().to_lowercase();
        let rows = crate::shell::chrome::command_match::visible_command_rows(
            &snapshot, &needle,
        );
        if rows.is_empty() {
            return;
        }
        let index = self.command_selected_index.min(rows.len() - 1);
        match rows[index].clone() {
            crate::shell::chrome::command_match::CommandSelection::SelectTab(tab_id) => {
                self.select_tab(&tab_id, window, cx);
            }
            crate::shell::chrome::command_match::CommandSelection::OpenUrl(url) => {
                self.open_internal_tab(url.as_str(), window, cx);
            }
            crate::shell::chrome::command_match::CommandSelection::OpenRoute(route) => {
                self.open_internal_tab(route, window, cx);
            }
        }
        self.dismiss_command_mode(window, cx);
    }
}
