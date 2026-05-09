use ely_domain::CommandIntent;
use gpui::{Context, Window};

use super::ElyShell;
use ely_browser_core::SpaceImportProfileMapping;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpaceFileCommand {
    ExportActiveSpace,
    ImportToActiveProfile,
    ImportPreservingProfiles,
}

impl ElyShell {
    pub(super) fn handle_shell_command_intent(
        &mut self,
        intent: Option<&CommandIntent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(CommandIntent::Command(command)) = intent else {
            return;
        };

        if install_plugin_from_file_command(command) {
            self.choose_plugin_package(window, cx);
        }

        match space_file_command(command) {
            Some(SpaceFileCommand::ExportActiveSpace) => self.export_active_space(window, cx),
            Some(SpaceFileCommand::ImportToActiveProfile) => {
                self.choose_space_import(SpaceImportProfileMapping::UseActiveProfile, window, cx);
            }
            Some(SpaceFileCommand::ImportPreservingProfiles) => {
                self.choose_space_import(SpaceImportProfileMapping::PreserveExisting, window, cx);
            }
            None => {}
        }
    }
}

fn install_plugin_from_file_command(command: &str) -> bool {
    matches!(
        command.trim().to_ascii_lowercase().as_str(),
        "install-plugin-from-file"
            | "install plugin from file"
            | "install-plugin"
            | "install plugin"
    )
}

fn space_file_command(command: &str) -> Option<SpaceFileCommand> {
    match command.trim().to_ascii_lowercase().as_str() {
        "export-space" | "export space" | "export-active-space" | "export active space" => {
            Some(SpaceFileCommand::ExportActiveSpace)
        }
        "import-space"
        | "import space"
        | "import-space-active-profile"
        | "import space active profile" => Some(SpaceFileCommand::ImportToActiveProfile),
        "import-space-with-profiles"
        | "import space with profiles"
        | "import-space-preserve-profiles"
        | "import space preserve profiles" => Some(SpaceFileCommand::ImportPreservingProfiles),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SpaceFileCommand, install_plugin_from_file_command, space_file_command};

    #[test]
    fn install_plugin_from_file_command_matches_prd_aliases() {
        assert!(install_plugin_from_file_command("install-plugin-from-file"));
        assert!(install_plugin_from_file_command("Install Plugin from File"));
        assert!(install_plugin_from_file_command("install plugin"));
    }

    #[test]
    fn install_plugin_from_file_command_rejects_other_plugin_commands() {
        assert!(!install_plugin_from_file_command("plugins"));
        assert!(!install_plugin_from_file_command("open plugins"));
    }

    #[test]
    fn space_file_command_matches_export_and_import_aliases() {
        assert_eq!(space_file_command("export-space"), Some(SpaceFileCommand::ExportActiveSpace));
        assert_eq!(
            space_file_command("import space"),
            Some(SpaceFileCommand::ImportToActiveProfile)
        );
        assert_eq!(
            space_file_command("import-space-with-profiles"),
            Some(SpaceFileCommand::ImportPreservingProfiles)
        );
    }

    #[test]
    fn space_file_command_rejects_other_space_commands() {
        assert_eq!(space_file_command("new-space Research"), None);
        assert_eq!(space_file_command("spaces"), None);
    }
}
