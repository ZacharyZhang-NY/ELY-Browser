use ely_domain::TabId;
use gpui::{Context, Window};

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn recover_crashed_tab(
        &mut self,
        tab_id: &TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.recover_crashed_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    pub(super) fn wake_discarded_tab(
        &mut self,
        tab_id: &TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.wake_discarded_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }
}
