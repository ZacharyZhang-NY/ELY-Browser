use gpui::Context;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn request_clear_history_confirmation(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
        {
            self.history_clear_confirmation = Some(snapshot.active_profile_id);
            cx.notify();
        }
    }

    pub(super) fn cancel_clear_history_confirmation(&mut self, cx: &mut Context<Self>) {
        self.history_clear_confirmation = None;
        cx.notify();
    }

    pub(super) fn clear_active_profile_history(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
            && self.history_clear_confirmation.as_ref() == Some(&snapshot.active_profile_id)
            && core.clear_active_profile_history().is_ok()
        {
            self.history_clear_confirmation = None;
            cx.notify();
        }
    }
}
