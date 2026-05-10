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

    /// Close a specific tab by id. Used by the per-row close (×) on
    /// vertical tabs and launcher rows: closing the tab the user
    /// targeted directly, instead of the prior select-then-close
    /// dance which depended on `close_active_tab` doing the right
    /// thing after a fresh selection (and which silently no-op'd if
    /// the active tab routed into split-view close logic).
    pub(super) fn close_tab_by_id(
        &mut self,
        tab_id: &TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.close_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }
}
