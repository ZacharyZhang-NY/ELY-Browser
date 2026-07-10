use gpui::{App, Context, FocusHandle, Focusable, Window};

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn sync_address_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    /// Close the command overlay (Escape or a backdrop click): drop the
    /// `>` query that drives its visibility, reset the selection, and put
    /// the active tab's address back in the omnibar.
    pub(super) fn exit_command_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_command_query("");
        }
        self.command_selected_index = 0;
        self.sync_address_input(window, cx);
        cx.notify();
    }
}

impl Focusable for ElyShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
