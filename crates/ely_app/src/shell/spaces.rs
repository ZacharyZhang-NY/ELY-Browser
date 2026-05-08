use gpui::{Context, Window};

use super::{ElyShell, ShellState};
use crate::{SelectNextSpace, SelectPreviousSpace};

impl ElyShell {
    pub(super) fn on_select_next_space(
        &mut self,
        _: &SelectNextSpace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_next_space().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    pub(super) fn on_select_previous_space(
        &mut self,
        _: &SelectPreviousSpace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_previous_space().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }
}
