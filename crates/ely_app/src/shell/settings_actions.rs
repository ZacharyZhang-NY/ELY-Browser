use ely_domain::{
    ArchivePolicy, DownloadPolicy, FavoriteLimit, HistoryRecordingPolicy, NewTabDestination,
    ProfileId, ProfileSyncPolicy, SearchEngine, SyncObjectKind, SyncObjectPolicy,
};
use gpui::Context;

use super::{ElyShell, ShellState};

impl ElyShell {
    pub(super) fn set_active_space_archive_policy(
        &mut self,
        archive_policy: ArchivePolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_active_space_archive_policy(archive_policy).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn set_search_engine(
        &mut self,
        search_engine: SearchEngine,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_search_engine(search_engine);
            cx.notify();
        }
    }

    pub(super) fn reset_search_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_search_settings();
            cx.notify();
        }
    }

    pub(super) fn set_new_tab_destination(
        &mut self,
        destination: NewTabDestination,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_new_tab_destination(destination);
            cx.notify();
        }
    }

    pub(super) fn reset_general_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_general_settings();
            cx.notify();
        }
    }

    pub(super) fn set_history_recording_policy(
        &mut self,
        policy: HistoryRecordingPolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_history_recording_policy(policy);
            cx.notify();
        }
    }

    pub(super) fn reset_privacy_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_privacy_settings();
            cx.notify();
        }
    }

    pub(super) fn set_active_profile_download_policy(
        &mut self,
        policy: DownloadPolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_active_profile_download_policy(policy).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn reset_active_profile_download_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.reset_active_profile_download_settings().is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn set_profile_sync_policy(
        &mut self,
        profile_id: &ProfileId,
        sync_policy: ProfileSyncPolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.set_profile_sync_policy(profile_id, sync_policy).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn reset_profile_sync_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_profile_sync_settings();
            cx.notify();
        }
    }

    pub(super) fn set_favorite_limit(
        &mut self,
        favorite_limit: FavoriteLimit,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_favorite_limit(favorite_limit);
            cx.notify();
        }
    }

    pub(super) fn reset_sidebar_tabs_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.reset_sidebar_tabs_settings().is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn set_sync_object_policy(
        &mut self,
        kind: SyncObjectKind,
        policy: SyncObjectPolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_sync_object_policy(kind, policy);
            cx.notify();
        }
    }

    pub(super) fn reset_sync_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_sync_settings();
            cx.notify();
        }
    }

    pub(super) fn archive_idle_tabs_now(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.archive_idle_tabs(std::time::SystemTime::now()).is_ok()
        {
            cx.notify();
        }
    }
}
