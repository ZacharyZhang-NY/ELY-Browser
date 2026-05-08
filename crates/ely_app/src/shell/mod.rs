mod internal_pages;
mod render;

use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::{CommandIntent, DownloadId, SpaceId, TabId, UrlText};
use gpui::{App, AppContext, Context, Entity, FocusHandle, Focusable, Subscription, Window};
use gpui_component::input::{InputEvent, InputState, SelectAll};

use crate::{
    CloseCurrentTab, FocusAddressBar, FocusCommandMode, OpenDownloads, OpenHistory, OpenNewTab,
    OpenSettings, RestoreClosedTab, SelectNextTab, SelectPreviousTab, ToggleFavoriteTab,
    TogglePinnedTab,
    services::{
        download_checksums::DownloadChecksumCalculator, download_files::DownloadFileAction,
    },
};

enum ShellState {
    Ready(Box<BrowserCore>),
    StartupError(String),
}

pub struct ElyShell {
    state: ShellState,
    focus_handle: FocusHandle,
    command_input: Entity<InputState>,
    last_intent: Option<CommandIntent>,
    download_action_error: Option<String>,
    download_clear_confirmation: bool,
    _command_subscription: Subscription,
}

impl ElyShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or enter address"));

        let command_subscription = cx.subscribe_in(
            &command_input,
            window,
            |shell: &mut Self, input, event: &InputEvent, window, cx| {
                let mut submitted_intent = None;
                let mut sync_address = false;
                let submitted = matches!(event, InputEvent::PressEnter { .. });

                let ShellState::Ready(core) = &mut shell.state else {
                    return;
                };

                let value = input.read(cx).value().to_string();
                core.set_command_query(value);

                if submitted {
                    submitted_intent = core.submit_command().ok().flatten();
                    sync_address = core.command_query().is_empty();
                }

                if submitted {
                    shell.last_intent = submitted_intent;
                }

                if sync_address {
                    shell.sync_address_input(window, cx);
                }

                cx.notify();
            },
        );

        let state = match InitialBrowserConfig::ely_defaults().and_then(|config| {
            BrowserCore::new(config).map_err(|error| match error {
                ely_browser_core::CoreError::Domain(source) => source,
                _ => ely_domain::DomainError::InvalidCommand,
            })
        }) {
            Ok(core) => ShellState::Ready(Box::new(core)),
            Err(error) => ShellState::StartupError(error.to_string()),
        };

        Self {
            state,
            focus_handle: cx.focus_handle(),
            command_input,
            last_intent: None,
            download_action_error: None,
            download_clear_confirmation: false,
            _command_subscription: command_subscription,
        }
    }

    fn open_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://new-tab", window, cx);
    }

    fn open_downloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://downloads", window, cx);
    }

    fn open_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://history", window, cx);
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_internal_tab("ely://settings", window, cx);
    }

    fn open_internal_tab(&mut self, url_text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(url) = UrlText::parse(url_text) {
            self.open_url(url, window, cx);
        }
    }

    fn open_url(&mut self, url: UrlText, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.open_tab(url);
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    fn focus_address_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        window.dispatch_action(Box::new(SelectAll), cx);
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

    fn pause_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.pause_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    fn resume_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.resume_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    fn cancel_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.cancel_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    fn retry_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.retry_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    fn request_clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        self.download_clear_confirmation = true;
        cx.notify();
    }

    fn cancel_clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        self.download_clear_confirmation = false;
        cx.notify();
    }

    fn clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.clear_downloads_for_active_profile();
        }
        self.download_clear_confirmation = false;
        self.download_action_error = None;
        cx.notify();
    }

    fn calculate_download_checksum(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        let result = match &mut self.state {
            ShellState::Ready(core) => core
                .download_target_file_path(download_id)
                .map_err(|error| error.to_string())
                .and_then(|path| {
                    DownloadChecksumCalculator::sha256(&path).map_err(|error| error.to_string())
                })
                .and_then(|checksum| {
                    core.record_download_checksum(download_id, checksum)
                        .map_err(|error| error.to_string())
                }),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.download_action_error = result.err();
        cx.notify();
    }

    fn open_download_file(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        self.run_download_file_action(download_id, DownloadFileAction::Open, cx);
    }

    fn reveal_download_file(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        self.run_download_file_action(download_id, DownloadFileAction::Reveal, cx);
    }

    fn run_download_file_action(
        &mut self,
        download_id: &DownloadId,
        action: DownloadFileAction,
        cx: &mut Context<Self>,
    ) {
        let result = match &self.state {
            ShellState::Ready(core) => core
                .download_target_file_path(download_id)
                .map_err(|error| error.to_string())
                .and_then(|path| action.run(&path).map_err(|error| error.to_string())),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.download_action_error = result.err();
        cx.notify();
    }

    fn on_close_current_tab(
        &mut self,
        _: &CloseCurrentTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_tab(window, cx);
    }

    fn on_focus_address_bar(
        &mut self,
        _: &FocusAddressBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_address_bar(window, cx);
    }

    fn on_focus_command_mode(
        &mut self,
        _: &FocusCommandMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_command_mode(window, cx);
    }

    fn on_open_new_tab(&mut self, _: &OpenNewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.open_new_tab(window, cx);
    }

    fn on_open_downloads(
        &mut self,
        _: &OpenDownloads,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_downloads(window, cx);
    }

    fn on_open_history(&mut self, _: &OpenHistory, window: &mut Window, cx: &mut Context<Self>) {
        self.open_history(window, cx);
    }

    fn on_open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(window, cx);
    }

    fn on_restore_closed_tab(
        &mut self,
        _: &RestoreClosedTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.restore_closed_tab(window, cx);
    }

    fn on_select_next_tab(
        &mut self,
        _: &SelectNextTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_next_tab(window, cx);
    }

    fn on_select_previous_tab(
        &mut self,
        _: &SelectPreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_previous_tab(window, cx);
    }

    fn on_toggle_favorite_tab(
        &mut self,
        _: &ToggleFavoriteTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_favorite(cx);
    }

    fn on_toggle_pinned_tab(
        &mut self,
        _: &TogglePinnedTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_active_tab_pinned(cx);
    }

    fn sync_address_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };

        if let Ok(active_tab) = core.active_tab() {
            let value = active_tab.url().as_str().to_string();
            self.command_input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
            });
        }
    }
}

impl Focusable for ElyShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
