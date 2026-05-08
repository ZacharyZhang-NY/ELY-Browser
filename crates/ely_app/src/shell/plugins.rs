use std::path::PathBuf;

use ely_domain::{PluginId, PluginManifest, PluginPermission};
use gpui::{Context, PathPromptOptions, Window};

use crate::services::{
    plugin_package_store::PluginPackageStore,
    plugin_packages::{PluginPackageError, PluginPackageReader, VerifiedPluginPackage},
};

use super::{ElyShell, ShellState};

#[derive(Clone, Debug)]
pub(super) struct PendingPluginInstall {
    package: VerifiedPluginPackage,
    high_risk_permissions: Vec<PluginPermission>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingPluginUninstall {
    plugin_id: PluginId,
    plugin_name: String,
}

impl PendingPluginInstall {
    fn new(package: VerifiedPluginPackage, high_risk_permissions: Vec<PluginPermission>) -> Self {
        Self { package, high_risk_permissions }
    }

    pub(super) fn manifest(&self) -> &PluginManifest {
        self.package.manifest()
    }

    pub(super) fn high_risk_permissions(&self) -> &[PluginPermission] {
        &self.high_risk_permissions
    }
}

impl PendingPluginUninstall {
    fn new(plugin_id: PluginId, plugin_name: String) -> Self {
        Self { plugin_id, plugin_name }
    }

    pub(super) fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    pub(super) fn plugin_name(&self) -> &str {
        &self.plugin_name
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

        self.install_plugin_package(pending.package, true, cx);
    }

    pub(super) fn cancel_plugin_install(&mut self, cx: &mut Context<Self>) {
        self.pending_plugin_install = None;
        cx.notify();
    }

    pub(super) fn request_plugin_uninstall(
        &mut self,
        plugin_id: PluginId,
        plugin_name: String,
        cx: &mut Context<Self>,
    ) {
        self.pending_plugin_uninstall = Some(PendingPluginUninstall::new(plugin_id, plugin_name));
        self.plugin_install_error = None;
        cx.notify();
    }

    pub(super) fn cancel_plugin_uninstall(&mut self, cx: &mut Context<Self>) {
        self.pending_plugin_uninstall = None;
        cx.notify();
    }

    pub(super) fn confirm_plugin_uninstall(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_plugin_uninstall.take() else {
            cx.notify();
            return;
        };

        self.uninstall_plugin(pending.plugin_id, cx);
    }

    pub(super) fn set_plugin_enabled(
        &mut self,
        plugin_id: PluginId,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let result = match &mut self.state {
            ShellState::Ready(core) => {
                let action = if enabled {
                    core.enable_plugin(&plugin_id)
                } else {
                    core.disable_plugin(&plugin_id)
                };
                action.map_err(|error| error.to_string())
            }
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.plugin_install_error = result.err();
        cx.notify();
    }

    fn uninstall_plugin(&mut self, plugin_id: PluginId, cx: &mut Context<Self>) {
        let result = match &mut self.state {
            ShellState::Ready(core) => PluginPackageStore::application()
                .and_then(|store| store.remove_plugin(&plugin_id))
                .map_err(|error| error.to_string())
                .and_then(|_| core.uninstall_plugin(&plugin_id).map_err(|error| error.to_string())),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.plugin_install_error = result.err();
        cx.notify();
    }

    fn handle_plugin_package_result(
        &mut self,
        result: Result<VerifiedPluginPackage, PluginPackageError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(package) => self.install_plugin_package(package, false, cx),
            Err(error) => {
                self.plugin_install_error = Some(error.to_string());
                self.pending_plugin_install = None;
                cx.notify();
            }
        }
    }

    fn install_plugin_package(
        &mut self,
        package: VerifiedPluginPackage,
        high_risk_confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        let high_risk_permissions =
            package.manifest().high_risk_permissions().cloned().collect::<Vec<_>>();
        if !high_risk_confirmed && !high_risk_permissions.is_empty() {
            self.pending_plugin_install =
                Some(PendingPluginInstall::new(package, high_risk_permissions));
            self.plugin_install_error = None;
            cx.notify();
            return;
        }

        let result = match &mut self.state {
            ShellState::Ready(core) => PluginPackageStore::application()
                .and_then(|store| store.store(&package))
                .map_err(|error| error.to_string())
                .and_then(|stored_package| {
                    core.install_plugin(stored_package.manifest().clone(), high_risk_confirmed)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        self.plugin_install_error = result.err();
        if self.plugin_install_error.is_none() {
            self.pending_plugin_install = None;
        }
        cx.notify();
    }
}

fn load_plugin_package(path: PathBuf) -> Result<VerifiedPluginPackage, PluginPackageError> {
    PluginPackageReader::read_directory_package(&path)
}
