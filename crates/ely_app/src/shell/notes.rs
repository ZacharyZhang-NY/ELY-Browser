use ely_domain::NoteId;
use gpui::Context;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn remove_note_entry(&mut self, note_id: &NoteId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.remove_note_entry(note_id).is_ok()
        {
            cx.notify();
        }
    }
}
