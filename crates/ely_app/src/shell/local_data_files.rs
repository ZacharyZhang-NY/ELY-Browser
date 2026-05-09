use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::UserDirs;
use ely_browser_core::ELYDATA_FILE_EXTENSION;
use gpui::{Context, Window};

use super::{ElyShell, ShellState};

const PRIVACY_SECURITY_URL: &str = "ely://settings/privacy-security";

impl ElyShell {
    pub(super) fn export_local_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_privacy_security_surface(window, cx);
        self.clear_local_data_file_message();

        let export = match &mut self.state {
            ShellState::Ready(core) => {
                let package_json = match core.export_local_data_package_json() {
                    Ok(package_json) => package_json,
                    Err(error) => {
                        self.set_local_data_file_error(error.to_string(), cx);
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
                self.set_local_data_file_error(error, cx);
                return;
            }
        };
        let directory = match default_export_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.set_local_data_file_error(error, cx);
                return;
            }
        };
        let suggested_name = local_data_export_filename(&profile_name);
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));

        cx.spawn_in(window, async move |shell, window| {
            let selected_path = match prompt.await {
                Ok(Ok(path)) => path,
                Ok(Err(error)) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_local_data_file_error(error.to_string(), cx);
                    });
                    return;
                }
                Err(error) => {
                    _ = shell.update_in(window, |shell, _, cx| {
                        shell.set_local_data_file_error(error.to_string(), cx);
                    });
                    return;
                }
            };

            let Some(path) = selected_path else {
                return;
            };

            let result = window
                .background_executor()
                .spawn(async move { write_local_data_package(path, package_json) })
                .await;
            _ = shell.update_in(window, |shell, _, cx| {
                shell.handle_local_data_export_result(result, cx);
            });
        })
        .detach();
    }

    fn ensure_privacy_security_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab_matches_url(PRIVACY_SECURITY_URL) {
            return;
        }

        self.open_internal_tab(PRIVACY_SECURITY_URL, window, cx);
    }

    fn clear_local_data_file_message(&mut self) {
        self.local_data_file_error = None;
        self.local_data_file_notice = None;
    }

    fn set_local_data_file_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.local_data_file_error = Some(message);
        self.local_data_file_notice = None;
        cx.notify();
    }

    fn set_local_data_file_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.local_data_file_notice = Some(message);
        self.local_data_file_error = None;
        cx.notify();
    }

    fn handle_local_data_export_result(
        &mut self,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => self.set_local_data_file_notice(format!("Exported {}", path.display()), cx),
            Err(error) => self.set_local_data_file_error(error, cx),
        }
    }
}

fn default_export_directory() -> Result<PathBuf, String> {
    UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .ok_or_else(|| "Documents directory is unavailable.".to_string())
}

fn write_local_data_package(path: PathBuf, package_json: String) -> Result<PathBuf, String> {
    let path = normalize_export_path(path)?;
    fs::write(&path, package_json)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    Ok(path)
}

fn normalize_export_path(mut path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_none() {
        path.set_extension(ELYDATA_FILE_EXTENSION);
        return Ok(path);
    }

    if path_has_elydata_extension(&path) {
        Ok(path)
    } else {
        Err("Export path must use .elydata extension.".to_string())
    }
}

fn path_has_elydata_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.to_string_lossy().eq_ignore_ascii_case(ELYDATA_FILE_EXTENSION)
    })
}

fn local_data_export_filename(profile_name: &str) -> String {
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
    let stem = if stem.is_empty() { "local-data" } else { stem };
    format!("{stem}.{ELYDATA_FILE_EXTENSION}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{local_data_export_filename, normalize_export_path};

    #[test]
    fn local_data_export_filename_sanitizes_profile_names() {
        assert_eq!(local_data_export_filename("Default"), "Default.elydata");
        assert_eq!(local_data_export_filename("Work / Client"), "Work-Client.elydata");
        assert_eq!(local_data_export_filename("  "), "local-data.elydata");
    }

    #[test]
    fn normalize_export_path_adds_elydata_extension() -> Result<(), String> {
        let path = normalize_export_path(PathBuf::from("Default"))?;

        assert_eq!(path, PathBuf::from("Default.elydata"));
        Ok(())
    }

    #[test]
    fn normalize_export_path_rejects_other_extensions() {
        let error = normalize_export_path(PathBuf::from("Default.json"));

        assert_eq!(error, Err("Export path must use .elydata extension.".to_string()));
    }
}
