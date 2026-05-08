use std::time::{Duration, SystemTime};

use ely_domain::{ProfileId, SpaceId};
use gpui::Context;

use super::{ElyShell, ShellState};

const RECENT_HISTORY_CLEAR_WINDOW: Duration = Duration::from_secs(3_600);

#[derive(Clone, Debug)]
pub(super) struct PendingHistoryDomainClear {
    profile_id: ProfileId,
    space_id: SpaceId,
    host: String,
}

impl PendingHistoryDomainClear {
    fn new(profile_id: ProfileId, space_id: SpaceId, host: String) -> Self {
        Self { profile_id, space_id, host }
    }

    pub(super) fn host(&self) -> &str {
        &self.host
    }

    pub(super) fn matches_context(&self, profile_id: &ProfileId, space_id: &SpaceId) -> bool {
        &self.profile_id == profile_id && &self.space_id == space_id
    }
}

#[derive(Clone, Debug)]
pub(super) struct PendingHistoryTimeClear {
    profile_id: ProfileId,
    space_id: SpaceId,
    cutoff: SystemTime,
    label: &'static str,
}

impl PendingHistoryTimeClear {
    fn recent_hour(profile_id: ProfileId, space_id: SpaceId) -> Self {
        Self {
            profile_id,
            space_id,
            cutoff: SystemTime::now() - RECENT_HISTORY_CLEAR_WINDOW,
            label: "last hour",
        }
    }

    pub(super) fn cutoff(&self) -> SystemTime {
        self.cutoff
    }

    pub(super) fn label(&self) -> &'static str {
        self.label
    }

    pub(super) fn matches_context(&self, profile_id: &ProfileId, space_id: &SpaceId) -> bool {
        &self.profile_id == profile_id && &self.space_id == space_id
    }
}

impl ElyShell {
    pub(super) fn request_clear_history_confirmation(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
        {
            self.history_clear_confirmation = Some(snapshot.active_profile_id);
            self.pending_history_domain_clear = None;
            self.pending_history_time_clear = None;
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
            self.pending_history_domain_clear = None;
            self.pending_history_time_clear = None;
            cx.notify();
        }
    }

    pub(super) fn request_clear_history_domain_confirmation(
        &mut self,
        host: String,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
        {
            self.history_clear_confirmation = None;
            self.pending_history_time_clear = None;
            self.pending_history_domain_clear = Some(PendingHistoryDomainClear::new(
                snapshot.active_profile_id,
                snapshot.active_space_id,
                host,
            ));
            cx.notify();
        }
    }

    pub(super) fn cancel_clear_history_domain_confirmation(&mut self, cx: &mut Context<Self>) {
        self.pending_history_domain_clear = None;
        cx.notify();
    }

    pub(super) fn clear_active_space_history_for_pending_domain(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_history_domain_clear.clone() else {
            return;
        };

        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
            && pending.matches_context(&snapshot.active_profile_id, &snapshot.active_space_id)
            && core.clear_active_space_history_for_host(pending.host()).is_ok()
        {
            self.pending_history_domain_clear = None;
            cx.notify();
        }
    }

    pub(super) fn request_clear_recent_history_confirmation(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
        {
            self.history_clear_confirmation = None;
            self.pending_history_domain_clear = None;
            self.pending_history_time_clear = Some(PendingHistoryTimeClear::recent_hour(
                snapshot.active_profile_id,
                snapshot.active_space_id,
            ));
            cx.notify();
        }
    }

    pub(super) fn cancel_clear_recent_history_confirmation(&mut self, cx: &mut Context<Self>) {
        self.pending_history_time_clear = None;
        cx.notify();
    }

    pub(super) fn clear_active_space_history_for_pending_time(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_history_time_clear.clone() else {
            return;
        };

        if let ShellState::Ready(core) = &mut self.state
            && let Ok(snapshot) = core.snapshot()
            && pending.matches_context(&snapshot.active_profile_id, &snapshot.active_space_id)
            && core.clear_active_space_history_since(pending.cutoff()).is_ok()
        {
            self.pending_history_time_clear = None;
            cx.notify();
        }
    }
}
