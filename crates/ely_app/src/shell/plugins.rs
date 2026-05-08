use std::path::PathBuf;

use ely_domain::{PluginManifest, PluginPermission};
use gpui::{Context, PathPromptOptions, Window};

use crate::services::plugin_packages::{PluginPackageError, PluginPackageReader};

use super::{ElyShell, ShellState};

#[derive(Clone, Debug)]
pub(super) struct PendingPluginInstall {
    manifest: PluginManifest,
    high_risk_permissions: Vec<PluginPermission>,
}

impl PendingPluginInstall {
    fn new(manifest: PluginManifest, high_risk_permissions: Vec<PluginPermission>) -> Self {
        Self { manifest, high_risk_permissions }
    }

    pub(super) fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub(super) fn high_risk_permissions(&self) -> &[PluginPermission] {
        &self.high_risk_permissions
    }
}

impl ElyShell {
    pub(super) fn choose_plugin_package(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select .rplug package".into()),
        });

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.plugin_install_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.plugin_install_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result =
                window.background_executor().spawn(async move { load_plugin_package(path) }).await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_plugin_package_result(result, cx);
            });
        })
        .detach();
    }

    pub(super) fn confirm_plugin_install(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_plugin_install.take() else {
            cx.notify();
            return;
        };

        self.install_plugin_manifest(pending.manifest, true, cx);
    }

    pub(super) fn cancel_plugin_install(&mut self, cx: &mut Context<Self>) {
        self.pending_plugin_install = None;
        cx.notify();
    }

    fn handle_plugin_package_result(
        &mut self,
        result: Result<PluginManifest, PluginPackageError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(manifest) => self.install_plugin_manifest(manifest, false, cx),
            Err(error) => {
                self.plugin_install_error = Some(error.to_string());
                self.pending_plugin_install = None;
                cx.notify();
            }
        }
    }

    fn install_plugin_manifest(
        &mut self,
        manifest: PluginManifest,
        high_risk_confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        let high_risk_permissions = manifest.high_risk_permissions().cloned().collect::<Vec<_>>();
        if !high_risk_confirmed && !high_risk_permissions.is_empty() {
            self.pending_plugin_install =
                Some(PendingPluginInstall::new(manifest, high_risk_permissions));
            self.plugin_install_error = None;
            cx.notify();
            return;
        }

        let result = match &mut self.state {
            ShellState::Ready(core) => core
                .install_plugin(manifest, high_risk_confirmed)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.plugin_install_error = result.err();
        if self.plugin_install_error.is_none() {
            self.pending_plugin_install = None;
        }
        cx.notify();
    }
}

fn load_plugin_package(path: PathBuf) -> Result<PluginManifest, PluginPackageError> {
    PluginPackageReader::read_directory_package(&path)
}
