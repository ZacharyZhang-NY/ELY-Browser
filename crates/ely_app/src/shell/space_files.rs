use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::UserDirs;
use ely_browser_core::{ELYSPACE_FILE_EXTENSION, SpaceImportProfileMapping};
use ely_domain::SpaceId;
use gpui::{Context, PathPromptOptions, Window};

use super::{ElyShell, ShellState};

const SPACE_SETTINGS_URL: &str = "ely://settings/spaces";

impl ElyShell {
    pub(super) fn export_active_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let space_id = match &self.state {
            ShellState::Ready(core) => match core.snapshot() {
                Ok(snapshot) => snapshot.active_space_id,
                Err(error) => {
                    self.set_space_file_error(error.to_string(), cx);
                    return;
                }
            },
            ShellState::StartupError(message) => {
                self.set_space_file_error(message.clone(), cx);
                return;
            }
        };

        self.export_space(&space_id, window, cx);
    }

    pub(super) fn export_space(
        &mut self,
        space_id: &SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_space_settings_surface(window, cx);
        self.clear_space_file_message();

        let export = match &mut self.state {
            ShellState::Ready(core) => {
                let package_json = match core.export_space_package_json(space_id) {
                    Ok(package_json) => package_json,
                    Err(error) => {
                        self.set_space_file_error(error.to_string(), cx);
                        return;
                    }
                };
                let package = match core.export_space_package(space_id) {
                    Ok(package) => package,
                    Err(error) => {
                        self.set_space_file_error(error.to_string(), cx);
                        return;
                    }
                };
                Ok((package.space_name().to_string(), package_json))
            }
            ShellState::StartupError(message) => Err(message.clone()),
        };

        let (space_name, package_json) = match export {
            Ok(export) => export,
            Err(error) => {
                self.set_space_file_error(error, cx);
                return;
            }
        };
        let directory = match default_export_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.set_space_file_error(error, cx);
                return;
            }
        };
        let suggested_name = space_export_filename(&space_name);
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(path)) => path,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_space_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_space_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { write_space_package(path, package_json) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_space_export_result(result, cx);
            });
        })
        .detach();
    }

    pub(super) fn choose_space_import(
        &mut self,
        profile_mapping: SpaceImportProfileMapping,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_space_settings_surface(window, cx);
        self.clear_space_file_message();

        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select .elyspace file".into()),
        });

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_space_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_space_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result =
                window.background_executor().spawn(async move { read_space_package(path) }).await;
            _ = shell.update_in(window, |shell, window, cx| {
                shell.handle_space_import_result(result, profile_mapping, window, cx);
            });
        })
        .detach();
    }

    fn ensure_space_settings_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab_matches_url(SPACE_SETTINGS_URL) {
            return;
        }

        self.open_internal_tab(SPACE_SETTINGS_URL, window, cx);
    }

    fn clear_space_file_message(&mut self) {
        self.space_file_error = None;
        self.space_file_notice = None;
    }

    fn set_space_file_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.space_file_error = Some(message);
        self.space_file_notice = None;
        cx.notify();
    }

    fn set_space_file_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.space_file_notice = Some(message);
        self.space_file_error = None;
        cx.notify();
    }

    fn handle_space_export_result(
        &mut self,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => self.set_space_file_notice(format!("Exported {}", path.display()), cx),
            Err(error) => self.set_space_file_error(error, cx),
        }
    }

    fn handle_space_import_result(
        &mut self,
        result: Result<String, String>,
        profile_mapping: SpaceImportProfileMapping,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let package_json = match result {
            Ok(package_json) => package_json,
            Err(error) => {
                self.set_space_file_error(error, cx);
                return;
            }
        };

        let import_result = match &mut self.state {
            ShellState::Ready(core) => core
                .import_space_package_json(&package_json, profile_mapping)
                .and_then(|space_id| {
                    core.snapshot().map(|snapshot| {
                        snapshot.spaces.iter().find(|space| space.id() == &space_id).map_or_else(
                            || "Imported Space".to_string(),
                            |space| format!("Imported {}", space.name()),
                        )
                    })
                })
                .map_err(|error| error.to_string()),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        match import_result {
            Ok(message) => {
                self.sync_address_input(window, cx);
                self.set_space_file_notice(message, cx);
            }
            Err(error) => self.set_space_file_error(error, cx),
        }
    }
}

fn default_export_directory() -> Result<PathBuf, String> {
    UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .ok_or_else(|| "Documents directory is unavailable.".to_string())
}

fn write_space_package(path: PathBuf, package_json: String) -> Result<PathBuf, String> {
    let path = normalize_export_path(path)?;
    fs::write(&path, package_json)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    Ok(path)
}

fn read_space_package(path: PathBuf) -> Result<String, String> {
    if !path_has_elyspace_extension(&path) {
        return Err("Selected file must use .elyspace extension.".to_string());
    }

    fs::read_to_string(&path).map_err(|error| format!("Unable to read {}: {error}", path.display()))
}

fn normalize_export_path(mut path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_none() {
        path.set_extension(ELYSPACE_FILE_EXTENSION);
        return Ok(path);
    }

    if path_has_elyspace_extension(&path) {
        Ok(path)
    } else {
        Err("Export path must use .elyspace extension.".to_string())
    }
}

fn path_has_elyspace_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.to_string_lossy().eq_ignore_ascii_case(ELYSPACE_FILE_EXTENSION)
    })
}

fn space_export_filename(space_name: &str) -> String {
    let mut stem = String::new();
    let mut previous_separator = false;

    for ch in space_name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            stem.push(ch);
            previous_separator = false;
        } else if !previous_separator {
            stem.push('-');
            previous_separator = true;
        }
    }

    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "space" } else { stem };
    format!("{stem}.{ELYSPACE_FILE_EXTENSION}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{normalize_export_path, space_export_filename};

    #[test]
    fn space_export_filename_sanitizes_names() {
        assert_eq!(space_export_filename("Work"), "Work.elyspace");
        assert_eq!(space_export_filename("Client / Research"), "Client-Research.elyspace");
        assert_eq!(space_export_filename("  "), "space.elyspace");
    }

    #[test]
    fn normalize_export_path_adds_missing_extension() -> Result<(), String> {
        let path = normalize_export_path(PathBuf::from("Work"))?;

        assert_eq!(path, PathBuf::from("Work.elyspace"));
        Ok(())
    }

    #[test]
    fn normalize_export_path_rejects_other_extensions() {
        let error = normalize_export_path(PathBuf::from("Work.json"));

        assert_eq!(error, Err("Export path must use .elyspace extension.".to_string()));
    }
}
