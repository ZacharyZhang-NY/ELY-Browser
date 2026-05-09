use ely_domain::CommandIntent;
use gpui::{Context, Window};

use super::ElyShell;

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

#[cfg(test)]
mod tests {
    use super::install_plugin_from_file_command;

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
}
