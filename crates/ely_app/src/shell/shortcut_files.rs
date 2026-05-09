use std::{
    borrow::BorrowMut,
    fs,
    path::{Path, PathBuf},
};

use directories::UserDirs;
use gpui::{App, Context, PathPromptOptions, Window};

use crate::shortcuts::{
    ELYKEYS_FILE_EXTENSION, ShortcutProfile, parse_shortcut_profile_json,
    shortcut_rebinding_key_bindings,
};

use super::ElyShell;

const SHORTCUT_SETTINGS_URL: &str = "ely://settings/shortcuts";

impl ElyShell {
    pub(super) fn export_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_shortcut_settings_surface(window, cx);
        self.clear_shortcut_file_message();

        let profile_json = match self.shortcut_profile.to_json() {
            Ok(profile_json) => profile_json,
            Err(error) => {
                self.set_shortcut_file_error(error.to_string(), cx);
                return;
            }
        };
        let directory = match default_export_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.set_shortcut_file_error(error, cx);
                return;
            }
        };
        let prompt = cx.prompt_for_new_path(&directory, Some("ELY Shortcuts.elykeys"));

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(path)) => path,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_shortcut_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_shortcut_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { write_shortcut_profile(path, profile_json) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_shortcut_export_result(result, cx);
            });
        })
        .detach();
    }

    pub(super) fn choose_shortcut_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_shortcut_settings_surface(window, cx);
        self.clear_shortcut_file_message();

        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select .elykeys file".into()),
        });

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_shortcut_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_shortcut_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { read_shortcut_profile(path) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_shortcut_import_result(result, cx);
            });
        })
        .detach();
    }

    fn ensure_shortcut_settings_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab_matches_url(SHORTCUT_SETTINGS_URL) {
            return;
        }

        self.open_internal_tab(SHORTCUT_SETTINGS_URL, window, cx);
    }

    fn clear_shortcut_file_message(&mut self) {
        self.shortcut_file_error = None;
        self.shortcut_file_notice = None;
    }

    fn set_shortcut_file_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.shortcut_file_error = Some(message);
        self.shortcut_file_notice = None;
        cx.notify();
    }

    fn set_shortcut_file_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.shortcut_file_notice = Some(message);
        self.shortcut_file_error = None;
        cx.notify();
    }

    fn handle_shortcut_export_result(
        &mut self,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => self.set_shortcut_file_notice(format!("Exported {}", path.display()), cx),
            Err(error) => self.set_shortcut_file_error(error, cx),
        }
    }

    fn handle_shortcut_import_result(
        &mut self,
        result: Result<ShortcutProfile, String>,
        cx: &mut Context<Self>,
    ) {
        let profile = match result {
            Ok(profile) => profile,
            Err(error) => {
                self.set_shortcut_file_error(error, cx);
                return;
            }
        };

        let previous_profile = self.shortcut_profile.clone();
        let app: &mut App = cx.borrow_mut();
        app.bind_keys(shortcut_rebinding_key_bindings(&previous_profile, &profile));
        self.shortcut_profile = profile;
        self.set_shortcut_file_notice("Imported shortcut profile".to_string(), cx);
    }
}

fn default_export_directory() -> Result<PathBuf, String> {
    UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .ok_or_else(|| "Documents directory is unavailable.".to_string())
}

fn write_shortcut_profile(path: PathBuf, profile_json: String) -> Result<PathBuf, String> {
    let path = normalize_export_path(path)?;
    fs::write(&path, profile_json)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    Ok(path)
}

fn read_shortcut_profile(path: PathBuf) -> Result<ShortcutProfile, String> {
    if !path_has_elykeys_extension(&path) {
        return Err("Selected file must use .elykeys extension.".to_string());
    }

    let profile_json = fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    parse_shortcut_profile_json(&profile_json).map_err(|error| error.to_string())
}

fn normalize_export_path(mut path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_none() {
        path.set_extension(ELYKEYS_FILE_EXTENSION);
        return Ok(path);
    }

    if path_has_elykeys_extension(&path) {
        Ok(path)
    } else {
        Err("Export path must use .elykeys extension.".to_string())
    }
}

fn path_has_elykeys_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.to_string_lossy().eq_ignore_ascii_case(ELYKEYS_FILE_EXTENSION)
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::normalize_export_path;

    #[test]
    fn normalize_export_path_adds_elykeys_extension() -> Result<(), String> {
        let path = normalize_export_path(PathBuf::from("ELY Shortcuts"))?;

        assert_eq!(path, PathBuf::from("ELY Shortcuts.elykeys"));
        Ok(())
    }

    #[test]
    fn normalize_export_path_rejects_other_extensions() {
        let error = normalize_export_path(PathBuf::from("shortcuts.json"));

        assert_eq!(error, Err("Export path must use .elykeys extension.".to_string()));
    }
}
