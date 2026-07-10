mod archive_labels;
mod auth;
mod auth_operation_gate;
mod bookmark_files;
mod bookmarks;
pub(crate) mod chrome;
mod chrome_motion;
mod command_actions;
mod download_targets;
mod downloads;
mod focus;
mod history;
mod internal_pages;
mod local_data_files;
mod local_persistence;
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
mod sync_devices;
mod sync_inbox;
mod sync_state;
mod tab_groups;
mod tab_lifecycle;
mod web_surface;
mod web_surface_cadence;
mod web_surface_controller;
mod web_surface_frame;
mod web_surface_geometry;
mod web_surface_input;
mod web_surface_keyboard;
mod web_surface_metadata;
mod web_surface_permissions;
mod web_surface_runtime;
mod web_surface_runtime_session;
mod web_surface_runtime_wire;
mod web_surface_state;
mod web_surface_view;
mod web_surface_worker;

#[cfg(test)]
mod gpui_harness_tests;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{DEFAULT_TRANSLUCENCY_PCT, ProfileId, SpaceId, TabId};
use gpui::{AppContext, Context, Entity, FocusHandle, Subscription, Timer, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::slider::{SliderEvent, SliderState, SliderValue};

use crate::shortcuts::ShortcutProfile;
use auth_operation_gate::{AuthenticatedOperationBarrier, AuthenticatedOperationGate};
use bookmarks::PendingBookmarkEdit;
use downloads::PendingDownloadFileAction;
use history::{PendingHistoryDomainClear, PendingHistoryTimeClear};
use plugins::{PendingPluginInstall, PendingPluginUninstall};
use sync_devices::SyncDeviceUiState;
use sync_inbox::{SyncStateMessage, SyncWorkGeneration};
use sync_state::PendingMergeUpload;
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
    chrome_motion: chrome_motion::ChromeMotionState,
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
    sync_inbox_rx: std::sync::mpsc::Receiver<SyncStateMessage>,
    sync_inbox_tx: std::sync::mpsc::Sender<SyncStateMessage>,
    sync_generation: SyncWorkGeneration,
    sync_profile_id: Option<ProfileId>,
    authenticated_operation_gate: AuthenticatedOperationGate,
    auth_flow_barrier: Option<AuthenticatedOperationBarrier>,
    sign_out_phases: std::collections::HashMap<ProfileId, auth::SignOutPhase>,
    sync_upload_scheduled: bool,
    sync_upload_in_flight: bool,
    sync_upload_pending: bool,
    sync_upload_pending_merge: Option<PendingMergeUpload>,
    sync_retry_at: Option<std::time::Instant>,
    default_profile_id: Option<ProfileId>,
    pub(crate) sync_devices: SyncDeviceUiState,
    pub(crate) sync_verification_input: Entity<InputState>,
    pub(crate) auth_email_input: Entity<InputState>,
    pub(crate) auth_otp_input: Entity<InputState>,
    pub(crate) auth_flow_phase: auth::AuthFlowPhase,
    pub(crate) local_state_path: Option<std::path::PathBuf>,
    pub(crate) local_state_save_scheduled: bool,
    _command_subscription: Subscription,
    _translucency_subscription: Subscription,
    _quit_save_subscription: Option<Subscription>,
}

impl ElyShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut default_profile_id = None;
        let config = InitialBrowserConfig::ely_defaults()
            .map_err(|error| error.to_string())
            .and_then(|mut config| {
                let profile_id = crate::services::profile_identity::default_standard_profile_id()
                    .map_err(|error| error.to_string())?;
                config.profile_id = Some(profile_id.clone());
                default_profile_id = Some(profile_id);
                Ok(config)
            });
        Self::new_with_config(config, default_profile_id, window, cx)
    }

    pub fn new_private(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_config(
            InitialBrowserConfig::private_window().map_err(|error| error.to_string()),
            None,
            window,
            cx,
        )
    }

    fn new_with_config(
        config: Result<InitialBrowserConfig, String>,
        default_profile_id: Option<ProfileId>,
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
        let sync_verification_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("ABCD-EF01-2345-6789"));
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
                    shell.schedule_cloud_sync_upload(cx);
                }

                cx.notify();
            },
        );

        let local_state_path =
            local_persistence::resolve_local_state_path(default_profile_id.as_ref());
        let state = match config
            .and_then(|config| BrowserCore::new(config).map_err(|error| error.to_string()))
        {
            Ok(mut core) => {
                if let Some(path) = &local_state_path {
                    local_persistence::restore_local_state(&mut core, path);
                }
                ShellState::Ready(Box::new(core))
            }
            Err(error) => ShellState::StartupError(error),
        };

        let (sync_inbox_tx, sync_inbox_rx) = std::sync::mpsc::channel();
        let sync_profile_id = match &state {
            ShellState::Ready(core) => {
                core.snapshot().ok().map(|snapshot| snapshot.active_profile_id)
            }
            ShellState::StartupError(_) => None,
        };
        let mut shell = Self {
            state,
            focus_handle: cx.focus_handle(),
            command_input,
            plugin_search_input,
            translucency_slider,
            workspace_picker_open: false,
            sidebar_hover_expanded: false,
            command_selected_index: 0,
            chrome_motion: chrome_motion::ChromeMotionState::default(),
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
            sync_generation: SyncWorkGeneration::default(),
            sync_profile_id,
            authenticated_operation_gate: AuthenticatedOperationGate::open(),
            auth_flow_barrier: None,
            sign_out_phases: std::collections::HashMap::new(),
            local_state_path,
            local_state_save_scheduled: false,
            sync_upload_scheduled: false,
            sync_upload_in_flight: false,
            sync_upload_pending: false,
            sync_upload_pending_merge: None,
            sync_retry_at: None,
            default_profile_id,
            sync_devices: SyncDeviceUiState::default(),
            sync_verification_input,
            auth_email_input,
            auth_otp_input,
            auth_flow_phase: auth::AuthFlowPhase::Idle,
            _command_subscription: command_subscription,
            _translucency_subscription: translucency_subscription,
            _quit_save_subscription: None,
        };
        shell._quit_save_subscription = Some(local_persistence::register_quit_save(cx));
        let should_run_initial_sync = shell.probe_initial_sync_state();
        if should_run_initial_sync {
            shell.trigger_cloud_sync_upload();
        }
        start_external_web_surface_timer(cx);
        shell
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
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn select_space(&mut self, space_id: &SpaceId, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_space(space_id).is_ok()
        {
            self.sync_address_input(window, cx);
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn select_profile(
        &mut self,
        profile_id: &ProfileId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = match &mut self.state {
            ShellState::Ready(core) => core.select_profile(profile_id).is_ok(),
            ShellState::StartupError(_) => false,
        };
        if !selected {
            return;
        }
        self.invalidate_sync_work();
        self.sync_profile_id = match &self.state {
            ShellState::Ready(core) => {
                core.snapshot().ok().map(|snapshot| snapshot.active_profile_id)
            }
            ShellState::StartupError(_) => None,
        };
        self.auth_flow_phase = auth::AuthFlowPhase::Idle;
        self.sync_address_input(window, cx);
        if self.probe_initial_sync_state() {
            self.schedule_cloud_sync_upload(cx);
        }
        cx.notify();
    }

    fn select_next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_next_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn select_previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.select_previous_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.close_active_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn restore_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.restore_last_archived_tab().is_ok()
        {
            self.sync_address_input(window, cx);
            self.schedule_cloud_sync_upload(cx);
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
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn toggle_active_tab_favorite(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_favorite().is_ok()
        {
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn toggle_active_tab_pinned(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.toggle_active_tab_pinned().is_ok()
        {
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn zoom_active_tab_in(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.zoom_active_tab_in().is_ok()
        {
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn zoom_active_tab_out(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.zoom_active_tab_out().is_ok()
        {
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }

    fn reset_active_tab_zoom(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.reset_active_tab_zoom().is_ok()
        {
            self.schedule_cloud_sync_upload(cx);
            cx.notify();
        }
    }
}

fn start_external_web_surface_timer(cx: &mut Context<ElyShell>) {
    cx.spawn(async move |shell, cx| {
        loop {
            let delay = match shell.update(cx, |shell, _| shell.external_web_surface_tick_delay()) {
                Ok(delay) => delay,
                Err(_) => break,
            };
            Timer::after(delay).await;
            let result = shell.update(cx, |shell, cx| {
                if shell.tick_external_web_surfaces(cx) {
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
