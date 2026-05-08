use ely_domain::{ReadingListId, ReadingProgress};
use gpui::Context;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn set_reading_list_progress(
        &mut self,
        entry_id: &ReadingListId,
        progress: ReadingProgress,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_reading_list_progress(entry_id, progress).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn remove_reading_list_entry(
        &mut self,
        entry_id: &ReadingListId,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.remove_reading_list_entry(entry_id).is_ok()
        {
            cx.notify();
        }
    }
}
