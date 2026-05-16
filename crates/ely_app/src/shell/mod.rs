mod archive_labels;
mod auth;
mod bookmark_files;
mod bookmarks;
pub(crate) mod chrome;
mod command_actions;
mod download_targets;
mod downloads;
mod focus;
mod history;
mod internal_pages;
mod local_data_files;
mod navigation;
mod notes;
mod plugins;
mod reading_list;
mod render;
mod settings_actions;
mod shell_actions;
mod shortcut_files;
mod sidebar;
mod site_permissions;
mod space_files;
mod spaces;
mod splits;
mod sync_state;
mod tab_groups;
mod tab_lifecycle;
mod web_surface;
mod web_surface_cadence;
mod web_surface_controller;
mod web_surface_frame;
mod web_surface_geometry;
mod web_surface_keyboard;
mod web_surface_metadata;
mod web_surface_permissions;
mod web_surface_runtime;
mod web_surface_state;
mod web_surface_view;
mod web_surface_worker;

#[cfg(test)]
mod gpui_harness_tests;

use std::time::Duration;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{DEFAULT_TRANSLUCENCY_PCT, ProfileId, SpaceId, TabId};
use gpui::{AppContext, Context, Entity, FocusHandle, Subscription, Timer, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::slider::{SliderEvent, SliderState, SliderValue};

use crate::shortcuts::ShortcutProfile;
use bookmarks::PendingBookmarkEdit;
use downloads::PendingDownloadFileAction;
use history::{PendingHistoryDomainClear, PendingHistoryTimeClear};
use plugins::{PendingPluginInstall, PendingPluginUninstall};
use sync_state::SyncStateUpdate;
use web_surface::WebSurfaceStore;

enum ShellState {
    Ready(Box<BrowserCore>),
    StartupError(String),
}

pub struct ElyShell {
    state: ShellState,
    focus_handle: FocusHandle,
    command_input: Entity<InputState>,
    pub(crate) plugin_search_input: Entity<InputState>,
    pub(crate) translucency_slider: Entity<SliderState>,
    pub(crate) workspace_picker_open: bool,
    pub(crate) sidebar_hover_expanded: bool,
    pub(crate) command_selected_index: usize,
    /// Live state for sidebar-resize drag. While the user holds the
    /// resize handle: `(mouse_x_at_drag_start, sidebar_width_px_at_drag_start)`.
    /// `None` whenever no drag is in flight.
    pub(crate) sidebar_resize_origin: Option<(f32, u16)>,
    download_action_error: Option<String>,
    download_clear_confirmation: bool,
    download_security_confirmation: Option<PendingDownloadFileAction>,
    history_clear_confirmation: Option<ProfileId>,
    pending_history_domain_clear: Option<PendingHistoryDomainClear>,
    pending_history_time_clear: Option<PendingHistoryTimeClear>,
    site_permissions_clear_confirmation: Option<ProfileId>,
    pending_space_trash: Option<SpaceId>,
    space_file_error: Option<String>,
    space_file_notice: Option<String>,
    shortcut_file_error: Option<String>,
    shortcut_file_notice: Option<String>,
    shortcut_profile: ShortcutProfile,
    pending_bookmark_edit: Option<PendingBookmarkEdit>,
    bookmark_edit_error: Option<String>,
    bookmark_file_error: Option<String>,
    bookmark_file_notice: Option<String>,
    local_data_file_error: Option<String>,
    local_data_file_notice: Option<String>,
    plugin_install_error: Option<String>,
    pending_plugin_install: Option<PendingPluginInstall>,
    pending_plugin_uninstall: Option<PendingPluginUninstall>,
    web_surfaces: WebSurfaceStore,
    /// Receives sync upload outcomes from the off-thread worker so the
    /// shell can refresh the connection state without blocking. Sender
    /// is cloned for each spawned upload; receiver is drained on every
    /// `tick_external_web_surfaces`.
    sync_inbox_rx: std::sync::mpsc::Receiver<SyncStateUpdate>,
    pub(crate) sync_inbox_tx: std::sync::mpsc::Sender<SyncStateUpdate>,
    pub(crate) auth_email_input: Entity<InputState>,
    pub(crate) auth_otp_input: Entity<InputState>,
    pub(crate) auth_flow_phase: auth::AuthFlowPhase,
    _command_subscription: Subscription,
    _translucency_subscription: Subscription,
}

impl ElyShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_config(InitialBrowserConfig::ely_defaults(), window, cx)
    }

    pub fn new_private(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_config(InitialBrowserConfig::private_window(), window, cx)
    }

    fn new_with_config(
        config: Result<InitialBrowserConfig, ely_domain::DomainError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search ELY or type a command…"));
        let plugin_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search plugins…"));
        let auth_email_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("you@elydora.com"));
        let auth_otp_input = cx.new(|cx| InputState::new(window, cx).placeholder("123456"));
        let translucency_slider = cx.new(|_cx| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(f32::from(DEFAULT_TRANSLUCENCY_PCT))
        });

        let translucency_subscription = cx.subscribe(
            &translucency_slider,
            |shell: &mut Self, _state, event: &SliderEvent, cx| {
                let SliderEvent::Change(SliderValue::Single(value)) = event else {
                    return;
                };
                let pct = value.clamp(0.0, 100.0).round() as u8;
                shell.set_translucency_pct(pct, cx);
            },
        );

        let command_subscription = cx.subscribe_in(
            &command_input,
            window,
            |shell: &mut Self, input, event: &InputEvent, window, cx| {
                let mut submitted_intent = None;
                let mut sync_address = false;
                let submitted = matches!(event, InputEvent::PressEnter { .. });

                {
                    let ShellState::Ready(core) = &mut shell.state else {
                        return;
                    };

                    let value = input.read(cx).value().to_string();
                    core.set_command_query(value);

                    if submitted {
                        submitted_intent = core.submit_command().ok().flatten();
                        sync_address = core.command_query().is_empty();
                    }
                }

                shell.handle_shell_command_intent(submitted_intent.as_ref(), window, cx);

                if sync_address {
                    shell.sync_address_input(window, cx);
                }

                cx.notify();
            },
        );

        let state = match config.and_then(|config| {
            BrowserCore::new(config).map_err(|error| match error {
                ely_browser_core::CoreError::Domain(source) => source,
                _ => ely_domain::DomainError::InvalidCommand,
            })
        }) {
            Ok(core) => ShellState::Ready(Box::new(core)),
            Err(error) => ShellState::StartupError(error.to_string()),
        };

        let (sync_inbox_tx, sync_inbox_rx) = std::sync::mpsc::channel();
        let mut shell = Self {
            state,
            focus_handle: cx.focus_handle(),
            command_input,
            plugin_search_input,
            translucency_slider,
            workspace_picker_open: false,
            sidebar_hover_expanded: false,
            command_selected_index: 0,
            sidebar_resize_origin: None,
            download_action_error: None,
            download_clear_confirmation: false,
            download_security_confirmation: None,
            history_clear_confirmation: None,
            pending_history_domain_clear: None,
            pending_history_time_clear: None,
            site_permissions_clear_confirmation: None,
            pending_space_trash: None,
            space_file_error: None,
            space_file_notice: None,
            shortcut_file_error: None,
            shortcut_file_notice: None,
            shortcut_profile: ShortcutProfile::default_profile(),
            pending_bookmark_edit: None,
            bookmark_edit_error: None,
            bookmark_file_error: None,
            bookmark_file_notice: None,
            local_data_file_error: None,
            local_data_file_notice: None,
            plugin_install_error: None,
            pending_plugin_install: None,
            pending_plugin_uninstall: None,
            web_surfaces: WebSurfaceStore::new(),
            sync_inbox_rx,
            sync_inbox_tx,
            auth_email_input,
            auth_otp_input,
            auth_flow_phase: auth::AuthFlowPhase::Idle,
            _command_subscription: command_subscription,
            _translucency_subscription: translucency_subscription,
        };
        shell.probe_initial_sync_state();
        start_external_web_surface_timer(cx);
        shell
    }

    /// Inspect the on-disk bearer token (if any) and seed the
    /// `SyncConnectionState` so the Sync settings page reads the right
    /// label on first render — without any sync setting page open it
    /// would otherwise stay `SignedOut` until the user clicks Sync now.
    fn probe_initial_sync_state(&mut self) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };
        let Some(snapshot) = core.snapshot().ok() else {
            return;
        };
        let active_profile_id = snapshot.active_profile_id.clone();
        let Some(profile_root) = crate::services::servo_profile_data::default_profile_data_root()
        else {
            return;
        };
        let profile_dir = crate::services::servo_profile_data::profile_data_dir(
            &profile_root,
            &active_profile_id,
        );
        let bearer_path = profile_dir.join("sync").join("bearer.token");
        let bearer_present = std::fs::metadata(&bearer_path).map(|m| m.len() > 0).unwrap_or(false);
        let state = if bearer_present {
            ely_domain::SyncConnectionState::SignedIn
        } else {
            ely_domain::SyncConnectionState::SignedOut
        };
        core.set_sync_connection_state(state);
    }

    /// Drain any sync upload outcomes the off-thread worker pushed
    /// since the previous tick and stamp the resulting connection
    /// state on `BrowserCore`. Returns `true` when at least one
    /// update was applied so callers can `cx.notify()` accordingly.
    pub(super) fn drain_sync_updates(&mut self) -> bool {
        let mut latest_connection: Option<ely_domain::SyncConnectionState> = None;
        let mut auth_changed = false;
        let mut trigger_initial_sync = false;
        while let Ok(update) = self.sync_inbox_rx.try_recv() {
            match update {
                SyncStateUpdate::SignedOut => {
                    latest_connection = Some(ely_domain::SyncConnectionState::SignedOut);
                }
                SyncStateUpdate::AwaitingDeviceApproval => {
                    latest_connection =
                        Some(ely_domain::SyncConnectionState::AwaitingDeviceApproval);
                }
                SyncStateUpdate::SyncReady { last_synced_at_secs } => {
                    latest_connection =
                        Some(ely_domain::SyncConnectionState::SyncReady { last_synced_at_secs });
                }
                SyncStateUpdate::SyncError { message } => {
                    latest_connection =
                        Some(ely_domain::SyncConnectionState::SyncError { message });
                }
                SyncStateUpdate::AuthOtpSent { email } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::AwaitingOtp { email };
                    auth_changed = true;
                }
                SyncStateUpdate::AuthSucceeded { email } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::Idle;
                    latest_connection = Some(ely_domain::SyncConnectionState::SignedIn);
                    trigger_initial_sync = true;
                    tracing::info!(target: "ely::sync", email = %email, "email OTP sign-in succeeded");
                    auth_changed = true;
                }
                SyncStateUpdate::AuthError { email, message } => {
                    self.auth_flow_phase = auth::AuthFlowPhase::Error { email, message };
                    auth_changed = true;
                }
            }
        }
        if let (Some(state), ShellState::Ready(core)) = (latest_connection, &mut self.state) {
            core.set_sync_connection_state(state);
        }
        if trigger_initial_sync {
            self.trigger_cloud_sync_upload();
        }
        auth_changed || trigger_initial_sync
    }

    fn focus_command_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.set_command_query(">");
        }

        self.command_input.update(cx, |input, cx| {
            input.set_value(">", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn select_tab(&mut self, tab_id: &TabId, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_space(&mut self, space_id: &SpaceId, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_space(space_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_profile(
        &mut self,
        profile_id: &ProfileId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_profile(profile_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_next_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn select_previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_previous_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.close_active_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn restore_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.restore_last_archived_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn restore_archived_tab(
        &mut self,
        tab_id: &TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ShellState::Ready(core) = &mut self.state
            && core.restore_archived_tab(tab_id).is_ok()
        {
            self.sync_address_input(window, cx);
            cx.notify();
        }
    }

    fn toggle_active_tab_favorite(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_favorite().is_ok()
        {
            cx.notify();
        }
    }

    fn toggle_active_tab_pinned(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_pinned().is_ok()
        {
            cx.notify();
        }
    }

    fn zoom_active_tab_in(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.zoom_active_tab_in().is_ok()
        {
            cx.notify();
        }
    }

    fn zoom_active_tab_out(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.zoom_active_tab_out().is_ok()
        {
            cx.notify();
        }
    }

    fn reset_active_tab_zoom(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.reset_active_tab_zoom().is_ok()
        {
            cx.notify();
        }
    }
}

fn start_external_web_surface_timer(cx: &mut Context<ElyShell>) {
    cx.spawn(async move |shell, cx| {
        // 8 ms ≈ 125 Hz, matched to a 120 Hz display's frame budget.
        // The Servo worker thread does the blocking IPC, so this tick
        // only enqueues Poll requests and drains the response channel
        // — cheap enough to run twice as often as the previous 60 Hz
        // schedule and lets active pages feed the renderer a fresh
        // frame between every display refresh.
        loop {
            Timer::after(Duration::from_millis(8)).await;
            let result = shell.update(cx, |shell, cx| {
                if shell.tick_external_web_surfaces() {
                    cx.notify();
                }
            });
            if result.is_err() {
                break;
            }
        }
    })
    .detach();
}
