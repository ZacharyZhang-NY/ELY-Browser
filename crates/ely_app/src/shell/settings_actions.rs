use ely_domain::{
    ArchivePolicy, DEFAULT_TRANSLUCENCY_PCT, DiagnosticsReportingPolicy, DownloadPolicy,
    FavoriteLimit, HistoryRecordingPolicy, NewTabDestination, ProfileId, ProfileSyncPolicy,
    SearchEngine, SyncObjectKind, SyncObjectPolicy, ThemeMode, UpdatePolicy, WallpaperTheme,
};
use gpui::Context;
use gpui_component::slider::SliderValue;

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

    pub(super) fn set_wallpaper_theme(
        &mut self,
        wallpaper: WallpaperTheme,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_wallpaper_theme(wallpaper);
            cx.notify();
        }
    }

    pub(super) fn set_theme_mode(
        &mut self,
        theme_mode: ThemeMode,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_theme_mode(theme_mode);
            cx.notify();
        }
    }

    /// Cycle the theme mode for the topbar's quick-toggle button:
    /// System → Light → Dark → System. Mirrors the segmented control
    /// in the appearance settings page so the topbar toggle reaches
    /// every state without spawning a settings page.
    pub(super) fn cycle_theme_mode(&mut self, cx: &mut Context<Self>) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };
        let next = match core.appearance().theme_mode() {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        };
        core.set_theme_mode(next);
        cx.notify();
    }

    pub(super) fn toggle_reduce_motion(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            let next = !core.appearance().reduce_motion();
            core.set_reduce_motion(next);
            cx.notify();
        }
    }

    pub(crate) fn set_translucency_pct(&mut self, value: u8, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_translucency_pct(value);
            cx.notify();
        }
    }

    pub(crate) fn set_translucency_pct_from_preset(
        &mut self,
        value: u8,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.set_translucency_pct(value, cx);
        let slider = self.translucency_slider.clone();
        slider.update(cx, |state, cx| {
            state.set_value(SliderValue::Single(f32::from(value)), window, cx);
        });
    }

    pub(super) fn reset_appearance(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_appearance();
            cx.notify();
        }
        let slider = self.translucency_slider.clone();
        slider.update(cx, |state, cx| {
            state.set_value(
                SliderValue::Single(f32::from(DEFAULT_TRANSLUCENCY_PCT)),
                window,
                cx,
            );
        });
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

    pub(super) fn set_diagnostics_reporting_policy(
        &mut self,
        policy: DiagnosticsReportingPolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_diagnostics_reporting_policy(policy);
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

    pub(super) fn set_update_policy(
        &mut self,
        update_policy: UpdatePolicy,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_update_policy(update_policy);
            cx.notify();
        }
    }

    pub(super) fn reset_update_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_update_settings();
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
