use std::time::SystemTime;

use ely_domain::{ProfileId, SpaceId};
use gpui::{Context, Window};

use super::{ElyShell, ShellState};
use crate::{SelectNextSpace, SelectPreviousSpace};

impl ElyShell {
    pub(super) fn set_active_space_default_profile(
        &mut self,
        profile_id: &ProfileId,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_active_space_default_profile(profile_id).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn move_space_up(&mut self, space_id: &SpaceId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.move_space_up(space_id).is_ok_and(|moved| moved)
        {
            cx.notify();
        }
    }

    pub(super) fn move_space_down(&mut self, space_id: &SpaceId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.move_space_down(space_id).is_ok_and(|moved| moved)
        {
            cx.notify();
        }
    }

    pub(super) fn request_space_trash(&mut self, space_id: SpaceId, cx: &mut Context<Self>) {
        self.pending_space_trash = Some(space_id);
        cx.notify();
    }

    pub(super) fn cancel_space_trash(&mut self, cx: &mut Context<Self>) {
        self.pending_space_trash = None;
        cx.notify();
    }

    pub(super) fn trash_pending_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space_id) = self.pending_space_trash.clone() else {
            return;
        };

        if let ShellState::Ready(core) = &mut self.state
            && core.trash_space(&space_id, std::time::SystemTime::now()).is_ok()
        {
            self.pending_space_trash = None;
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    pub(super) fn restore_trashed_space(
        &mut self,
        space_id: &SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.restore_trashed_space(space_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    pub(super) fn purge_expired_trashed_spaces(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.purge_expired_trashed_spaces(SystemTime::now()) > 0
        {
            cx.notify();
        }
    }

    pub(super) fn on_select_next_space(
        &mut self,
        _: &SelectNextSpace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_to_next_space(window, cx);
    }

    pub(super) fn cycle_to_next_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
