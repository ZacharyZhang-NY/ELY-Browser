use std::path::PathBuf;

use ely_domain::{DownloadId, UrlText};
use gpui::{Context, Window};

use crate::services::{
    download_checksums::DownloadChecksumCalculator,
    download_files::DownloadFileAction,
    http_downloads::{HttpDownloadError, HttpDownloadPlan, HttpDownloadResult, download_to_file},
};

use super::{
    ElyShell, ShellState,
    download_targets::{ActiveTabDownloadTarget, active_tab_download_target},
};

#[derive(Clone, Debug)]
pub(super) struct PendingDownloadFileAction {
    download_id: DownloadId,
    file_name: String,
    action: DownloadFileAction,
}

impl PendingDownloadFileAction {
    fn new(download_id: DownloadId, file_name: String, action: DownloadFileAction) -> Self {
        Self { download_id, file_name, action }
    }

    pub(super) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(super) fn action_label(&self) -> &'static str {
        self.action.label()
    }
}

impl ElyShell {
    pub(super) fn download_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.download_action_error = None;
        let target = match &self.state {
            ShellState::Ready(core) => active_tab_download_target(core),
            ShellState::StartupError(message) => Err(message.clone()),
        };
        let target = match target {
            Ok(target) => target,
            Err(message) => {
                self.set_download_action_error(message, cx);
                return;
            }
        };

        match target {
            ActiveTabDownloadTarget::Ready { url, target_path } => {
                self.start_download_to_path(url, target_path, window, cx);
            }
            ActiveTabDownloadTarget::Prompt { url, directory, file_name } => {
                let prompt = cx.prompt_for_new_path(&directory, Some(&file_name));

                cx.spawn_in(window, async move |shell, window| {
                    let selected_path = match prompt.await {
                        Ok(Ok(path)) => path,
                        Ok(Err(error)) => {
                            _ = shell.update_in(window, |shell, _, cx| {
                                shell.set_download_action_error(error.to_string(), cx);
                            });
                            return;
                        }
                        Err(error) => {
                            _ = shell.update_in(window, |shell, _, cx| {
                                shell.set_download_action_error(error.to_string(), cx);
                            });
                            return;
                        }
                    };

                    let Some(path) = selected_path else {
                        return;
                    };

                    _ = shell.update_in(window, |shell, window, cx| {
                        shell.start_download_to_path(url, path, window, cx);
                    });
                })
                .detach();
            }
        }
    }

    pub(super) fn pause_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.pause_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn resume_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.resume_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn cancel_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.cancel_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn retry_download(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.retry_download(download_id).is_ok()
        {
            cx.notify();
        }
    }

    pub(super) fn request_clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        self.download_clear_confirmation = true;
        cx.notify();
    }

    pub(super) fn cancel_clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        self.download_clear_confirmation = false;
        cx.notify();
    }

    pub(super) fn clear_active_profile_downloads(&mut self, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state {
            core.clear_downloads_for_active_profile();
        }
        self.download_clear_confirmation = false;
        self.download_action_error = None;
        cx.notify();
    }

    pub(super) fn calculate_download_checksum(
        &mut self,
        download_id: &DownloadId,
        cx: &mut Context<Self>,
    ) {
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

    fn start_download_to_path(
        &mut self,
        source_url: UrlText,
        target_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let download_id = match &mut self.state {
            ShellState::Ready(core) => match core.record_download_started_at_path(
                source_url.clone(),
                target_path.clone(),
                None,
            ) {
                Ok(download_id) => Ok(download_id),
                Err(error) => Err(error.to_string()),
            },
            ShellState::StartupError(message) => Err(message.clone()),
        };

        let download_id = match download_id {
            Ok(download_id) => download_id,
            Err(message) => {
                self.set_download_action_error(message, cx);
                return;
            }
        };

        self.open_downloads(window, cx);
        cx.notify();

        let plan = HttpDownloadPlan::new(source_url, target_path);
        cx.spawn_in(window, async move |shell, window| {
            let result =
                window.background_executor().spawn(async move { download_to_file(plan) }).await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_http_download_result(download_id, result, cx);
            });
        })
        .detach();
    }

    fn handle_http_download_result(
        &mut self,
        download_id: DownloadId,
        result: Result<HttpDownloadResult, HttpDownloadError>,
        cx: &mut Context<Self>,
    ) {
        let update_result = match (&mut self.state, result) {
            (ShellState::Ready(core), Ok(result)) => core
                .complete_download(&download_id, result.received_bytes())
                .map(|()| None)
                .map_err(|error| error.to_string()),
            (ShellState::Ready(core), Err(error)) => {
                let message = error.to_string();
                match core.fail_download(&download_id) {
                    Ok(()) => Ok(Some(message)),
                    Err(state_error) => Err(format!("{message}; {state_error}")),
                }
            }
            (ShellState::StartupError(message), _) => Err(message.clone()),
        };

        match update_result {
            Ok(message) => self.download_action_error = message,
            Err(message) => self.download_action_error = Some(message),
        }
        cx.notify();
    }

    fn set_download_action_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.download_action_error = Some(message);
        cx.notify();
    }

    pub(super) fn open_download_file(&mut self, download_id: &DownloadId, cx: &mut Context<Self>) {
        self.run_download_file_action(download_id, DownloadFileAction::Open, cx);
    }

    pub(super) fn reveal_download_file(
        &mut self,
        download_id: &DownloadId,
        cx: &mut Context<Self>,
    ) {
        self.run_download_file_action(download_id, DownloadFileAction::Reveal, cx);
    }

    pub(super) fn confirm_download_security_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(pending_action) = self.download_security_confirmation.take() else {
            cx.notify();
            return;
        };

        let result = match &mut self.state {
            ShellState::Ready(core) => core
                .confirm_download_security_prompt(&pending_action.download_id)
                .map_err(|error| error.to_string()),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        if let Err(message) = result {
            self.download_action_error = Some(message);
            cx.notify();
            return;
        }

        self.execute_download_file_action(&pending_action.download_id, pending_action.action, cx);
    }

    pub(super) fn cancel_download_security_prompt(&mut self, cx: &mut Context<Self>) {
        self.download_security_confirmation = None;
        cx.notify();
    }

    fn run_download_file_action(
        &mut self,
        download_id: &DownloadId,
        action: DownloadFileAction,
        cx: &mut Context<Self>,
    ) {
        match self.queue_security_confirmation(download_id, action) {
            Ok(true) => {
                self.download_action_error = None;
                cx.notify();
                return;
            }
            Ok(false) => {}
            Err(message) => {
                self.download_action_error = Some(message);
                cx.notify();
                return;
            }
        }

        self.execute_download_file_action(download_id, action, cx);
    }

    fn queue_security_confirmation(
        &mut self,
        download_id: &DownloadId,
        action: DownloadFileAction,
    ) -> Result<bool, String> {
        let core = match &self.state {
            ShellState::Ready(core) => core,
            ShellState::StartupError(message) => return Err(message.clone()),
        };

        if !core
            .download_requires_security_confirmation(download_id)
            .map_err(|error| error.to_string())?
        {
            return Ok(false);
        }

        let file_name = core
            .snapshot()
            .map_err(|error| error.to_string())?
            .download_entries
            .into_iter()
            .find(|entry| entry.id() == download_id)
            .map(|entry| entry.file_name().to_string())
            .ok_or_else(|| format!("download not found: {download_id}"))?;

        self.download_security_confirmation =
            Some(PendingDownloadFileAction::new(download_id.clone(), file_name, action));
        Ok(true)
    }

    fn execute_download_file_action(
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
}
