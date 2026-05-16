use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::UserDirs;
use ely_browser_core::ELYBOOKMARKS_FILE_EXTENSION;
use gpui::{Context, PathPromptOptions, Window};

use super::{ElyShell, ShellState};

const BOOKMARKS_URL: &str = "ely://bookmarks";

impl ElyShell {
    pub(super) fn export_bookmarks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_bookmarks_surface(window, cx);
        self.clear_bookmark_file_message();

        let export = match &mut self.state {
            ShellState::Ready(core) => {
                let package_json = match core.export_bookmarks_package_json() {
                    Ok(package_json) => package_json,
                    Err(error) => {
                        self.set_bookmark_file_error(error.to_string(), cx);
                        return;
                    }
                };
                match core.snapshot() {
                    Ok(snapshot) => Ok((snapshot.active_profile_name, package_json)),
                    Err(error) => Err(error.to_string()),
                }
            }
            ShellState::StartupError(message) => Err(message.clone()),
        };

        let (profile_name, package_json) = match export {
            Ok(export) => export,
            Err(error) => {
                self.set_bookmark_file_error(error, cx);
                return;
            }
        };
        let directory = match default_export_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.set_bookmark_file_error(error, cx);
                return;
            }
        };
        let suggested_name = bookmarks_export_filename(&profile_name);
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(path)) => path,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_bookmark_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_bookmark_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { write_bookmarks_package(path, package_json) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_bookmark_export_result(result, cx);
            });
        })
        .detach();
    }

    pub(super) fn choose_bookmark_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_bookmarks_surface(window, cx);
        self.clear_bookmark_file_message();

        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select .elybookmarks file".into()),
        });

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_bookmark_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_bookmark_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { read_bookmarks_package(path) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_bookmark_import_result(result, cx);
            });
        })
        .detach();
    }

    fn ensure_bookmarks_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab_matches_url(BOOKMARKS_URL) {
            return;
        }

        self.open_internal_tab(BOOKMARKS_URL, window, cx);
    }

    fn clear_bookmark_file_message(&mut self) {
        self.bookmark_file_error = None;
        self.bookmark_file_notice = None;
    }

    fn set_bookmark_file_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.bookmark_file_error = Some(message);
        self.bookmark_file_notice = None;
        cx.notify();
    }

    fn set_bookmark_file_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.bookmark_file_notice = Some(message);
        self.bookmark_file_error = None;
        cx.notify();
    }

    fn handle_bookmark_export_result(
        &mut self,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => self.set_bookmark_file_notice(format!("Exported {}", path.display()), cx),
            Err(error) => self.set_bookmark_file_error(error, cx),
        }
    }

    fn handle_bookmark_import_result(
        &mut self,
        result: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        let package_json = match result {
            Ok(package_json) => package_json,
            Err(error) => {
                self.set_bookmark_file_error(error, cx);
                return;
            }
        };

        let import_result = match &mut self.state {
            ShellState::Ready(core) => core
                .import_bookmarks_package_json(&package_json)
                .map(|summary| summary.label())
                .map_err(|error| error.to_string()),
            ShellState::StartupError(message) => Err(message.clone()),
        };

        match import_result {
            Ok(message) => {
                self.set_bookmark_file_notice(message, cx);
                self.schedule_cloud_sync_upload(cx);
            }
            Err(error) => self.set_bookmark_file_error(error, cx),
        }
    }
}

fn default_export_directory() -> Result<PathBuf, String> {
    UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .ok_or_else(|| "Documents directory is unavailable.".to_string())
}

fn write_bookmarks_package(path: PathBuf, package_json: String) -> Result<PathBuf, String> {
    let path = normalize_export_path(path)?;
    fs::write(&path, package_json)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    Ok(path)
}

fn read_bookmarks_package(path: PathBuf) -> Result<String, String> {
    if !path_has_elybookmarks_extension(&path) {
        return Err("Selected file must use .elybookmarks extension.".to_string());
    }

    fs::read_to_string(&path).map_err(|error| format!("Unable to read {}: {error}", path.display()))
}

fn normalize_export_path(mut path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_none() {
        path.set_extension(ELYBOOKMARKS_FILE_EXTENSION);
        return Ok(path);
    }

    if path_has_elybookmarks_extension(&path) {
        Ok(path)
    } else {
        Err("Export path must use .elybookmarks extension.".to_string())
    }
}

fn path_has_elybookmarks_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.to_string_lossy().eq_ignore_ascii_case(ELYBOOKMARKS_FILE_EXTENSION)
    })
}

fn bookmarks_export_filename(profile_name: &str) -> String {
    let mut stem = String::new();
    let mut previous_separator = false;

    for ch in profile_name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            stem.push(ch);
            previous_separator = false;
        } else if !previous_separator {
            stem.push('-');
            previous_separator = true;
        }
    }

    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "bookmarks" } else { stem };
    format!("{stem}.{ELYBOOKMARKS_FILE_EXTENSION}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{bookmarks_export_filename, normalize_export_path};

    #[test]
    fn bookmarks_export_filename_sanitizes_profile_names() {
        assert_eq!(bookmarks_export_filename("Default"), "Default.elybookmarks");
        assert_eq!(bookmarks_export_filename("Work / Research"), "Work-Research.elybookmarks");
        assert_eq!(bookmarks_export_filename("  "), "bookmarks.elybookmarks");
    }

    #[test]
    fn normalize_export_path_adds_elybookmarks_extension() -> Result<(), String> {
        let path = normalize_export_path(PathBuf::from("Default"))?;

        assert_eq!(path, PathBuf::from("Default.elybookmarks"));
        Ok(())
    }

    #[test]
    fn normalize_export_path_rejects_other_extensions() {
        let error = normalize_export_path(PathBuf::from("Default.json"));

        assert_eq!(error, Err("Export path must use .elybookmarks extension.".to_string()));
    }
}
