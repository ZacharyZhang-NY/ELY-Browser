use ely_browser_core::SyncEngine;
use ely_domain::{
    ArchivePolicy, DEFAULT_TRANSLUCENCY_PCT, DiagnosticsReportingPolicy, DownloadPolicy,
    FavoriteLimit, HistoryRecordingPolicy, NewTabDestination, ProfileId, ProfileSyncPolicy,
    SearchEngine, SyncObjectKind, SyncObjectPolicy, ThemeMode, UpdatePolicy, WallpaperTheme,
};
use gpui::Context;
use gpui_component::slider::SliderValue;

use crate::services::servo_profile_data::{default_profile_data_root, sync_profile_data_dir};

use super::sync_state::{SyncStateUpdate, sync_platform_label};
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

    pub(super) fn set_theme_mode(&mut self, theme_mode: ThemeMode, cx: &mut Context<Self>) {
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

    pub(super) fn reset_appearance(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_appearance();
            cx.notify();
        }
        let slider = self.translucency_slider.clone();
        slider.update(cx, |state, cx| {
            state.set_value(SliderValue::Single(f32::from(DEFAULT_TRANSLUCENCY_PCT)), window, cx);
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
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    pub(super) fn reset_profile_sync_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_profile_sync_settings();
            self.schedule_cloud_sync_upload(cx);
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
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    pub(super) fn reset_sync_settings(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.reset_sync_settings();
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    pub(crate) fn trigger_cloud_sync_upload(&mut self) {
        self.trigger_cloud_sync_upload_with_clock_floor(None);
    }

    pub(crate) fn trigger_cloud_sync_upload_after_remote(&mut self, logical_clock_floor: u64) {
        self.trigger_cloud_sync_upload_with_clock_floor(Some(logical_clock_floor));
    }

    fn trigger_cloud_sync_upload_with_clock_floor(&mut self, logical_clock_floor: Option<u64>) {
        if self.sync_upload_in_flight {
            self.sync_upload_scheduled = false;
            self.queue_cloud_sync_upload(logical_clock_floor);
            return;
        }
        self.sync_upload_scheduled = false;

        let ShellState::Ready(core) = &self.state else {
            return;
        };
        let Some(snapshot) = core.snapshot().ok() else {
            return;
        };
        let active_profile_id = snapshot.active_profile_id.clone();
        let device_name = format!("ELY · {}", snapshot.active_profile_name);
        let Some(profile_root) = default_profile_data_root() else {
            tracing::warn!(target: "ely::sync", "profile data root is unavailable");
            return;
        };
        let profile_dir = sync_profile_data_dir(&profile_root, &active_profile_id);
        let bytes = match core.build_sync_snapshot_bytes() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    target: "ely::sync",
                    error = %error,
                    "snapshot serialisation failed; aborting upload",
                );
                return;
            }
        };
        let tx = self.sync_inbox_tx.clone();
        let thread_name =
            if logical_clock_floor.is_some() { "ely-sync-merge-upload" } else { "ely-sync-upload" };
        self.sync_upload_in_flight = true;
        if let Err(error) =
            std::thread::Builder::new().name(thread_name.to_string()).spawn(move || {
                run_sync_upload(profile_dir, device_name, bytes, logical_clock_floor, tx)
            })
        {
            self.sync_upload_in_flight = false;
            tracing::warn!(
                target: "ely::sync",
                error = %error,
                "failed to spawn ely-sync-upload thread",
            );
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

fn run_sync_upload(
    profile_dir: std::path::PathBuf,
    device_name: String,
    bytes: Vec<u8>,
    logical_clock_floor: Option<u64>,
    inbox: std::sync::mpsc::Sender<SyncStateUpdate>,
) {
    let mut engine = match SyncEngine::for_profile_dir(
        &profile_dir,
        device_name,
        sync_platform_label(),
    ) {
        Ok(engine) => engine,
        Err(error) => {
            let message = error.to_string();
            tracing::warn!(target: "ely::sync", error = %message, "could not initialise sync engine");
            let _ = inbox.send(SyncStateUpdate::SyncError { message });
            return;
        }
    };
    let outcome = match logical_clock_floor {
        Some(floor) => engine.upload_merged_bytes(bytes, floor),
        None => engine.sync_bytes(bytes),
    };
    match outcome {
        Ok(ely_browser_core::SyncOutcome::SignedOut) => {
            tracing::info!(target: "ely::sync", "no bearer token on disk; sync skipped");
            let _ = inbox.send(SyncStateUpdate::SignedOut);
        }
        Ok(ely_browser_core::SyncOutcome::AwaitingDeviceApproval { device_id }) => {
            tracing::info!(
                target: "ely::sync",
                device_id = %device_id,
                "sync device is awaiting approval",
            );
            let _ = inbox.send(SyncStateUpdate::AwaitingDeviceApproval);
        }
        Ok(ely_browser_core::SyncOutcome::RemoteSnapshot {
            snapshot_id,
            logical_clock,
            payload_bytes,
            device_id,
            bytes,
        }) => {
            tracing::info!(
                target: "ely::sync",
                snapshot_id = %snapshot_id,
                logical_clock,
                payload_bytes,
                device_id = %device_id,
                "remote snapshot downloaded",
            );
            let _ = inbox.send(SyncStateUpdate::RemoteSnapshot { bytes, logical_clock });
        }
        Ok(ely_browser_core::SyncOutcome::AlreadyCurrent {
            snapshot_id,
            logical_clock,
            payload_bytes,
            device_id,
        }) => {
            tracing::info!(
                target: "ely::sync",
                snapshot_id = %snapshot_id,
                logical_clock,
                payload_bytes,
                device_id = %device_id,
                "snapshot already current",
            );
            let last_synced_at_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = inbox.send(SyncStateUpdate::SyncReady { last_synced_at_secs });
        }
        Ok(ely_browser_core::SyncOutcome::Uploaded {
            snapshot_id,
            logical_clock,
            payload_bytes,
            device_id,
        }) => {
            tracing::info!(
                target: "ely::sync",
                snapshot_id = %snapshot_id,
                logical_clock,
                payload_bytes,
                device_id = %device_id,
                "snapshot upload complete",
            );
            let last_synced_at_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = inbox.send(SyncStateUpdate::SyncReady { last_synced_at_secs });
        }
        Err(error) => {
            let message = error.to_string();
            tracing::warn!(target: "ely::sync", error = %message, "snapshot upload failed");
            let update = if message.contains("device_not_approved") {
                SyncStateUpdate::AwaitingDeviceApproval
            } else {
                SyncStateUpdate::SyncError { message }
            };
            let _ = inbox.send(update);
        }
    }
}
