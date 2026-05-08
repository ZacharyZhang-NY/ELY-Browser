use std::collections::BTreeMap;

use gpui::{App, KeyBinding};

use crate::{
    CloseCurrentTab, FocusAddressBar, FocusCommandMode, OpenDownloads, OpenHistory, OpenNewTab,
    OpenSettings, OpenTaskManager, Quit, RestoreClosedTab, SelectNextSpace, SelectNextTab,
    SelectPreviousSpace, SelectPreviousTab, SplitRight, ToggleFavoriteTab, ToggleSidebar,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ShortcutPlatform {
    Macos,
    WindowsLinux,
}

impl ShortcutPlatform {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Macos => "macOS",
            Self::WindowsLinux => "Windows/Linux",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ShortcutAction {
    FocusAddressBar,
    FocusCommandMode,
    OpenNewTab,
    CloseCurrentTab,
    RestoreClosedTab,
    SelectNextSpace,
    SelectPreviousSpace,
    SelectNextTab,
    SelectPreviousTab,
    SplitRight,
    ToggleSidebar,
    ToggleFavoriteTab,
    OpenDownloads,
    OpenHistory,
    OpenSettings,
    OpenTaskManager,
    Quit,
}

impl ShortcutAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::FocusAddressBar => "Command Bar",
            Self::FocusCommandMode => "Command Mode",
            Self::OpenNewTab => "New Tab",
            Self::CloseCurrentTab => "Close Tab",
            Self::RestoreClosedTab => "Restore Closed Tab",
            Self::SelectNextSpace => "Next Space",
            Self::SelectPreviousSpace => "Previous Space",
            Self::SelectNextTab => "Next Tab",
            Self::SelectPreviousTab => "Previous Tab",
            Self::SplitRight => "Split Right",
            Self::ToggleSidebar => "Toggle Sidebar",
            Self::ToggleFavoriteTab => "Toggle Favorite",
            Self::OpenDownloads => "Open Downloads",
            Self::OpenHistory => "Open History",
            Self::OpenSettings => "Open Settings",
            Self::OpenTaskManager => "Task Manager",
            Self::Quit => "Quit ELY Browser",
        }
    }

    pub(crate) fn category(self) -> &'static str {
        match self {
            Self::FocusAddressBar | Self::FocusCommandMode => "Command",
            Self::OpenNewTab
            | Self::CloseCurrentTab
            | Self::RestoreClosedTab
            | Self::SelectNextSpace
            | Self::SelectPreviousSpace
            | Self::SelectNextTab
            | Self::SelectPreviousTab
            | Self::SplitRight
            | Self::ToggleSidebar
            | Self::ToggleFavoriteTab => "Tabs",
            Self::OpenDownloads | Self::OpenHistory => "Library",
            Self::OpenSettings | Self::OpenTaskManager => "System",
            Self::Quit => "Application",
        }
    }

    pub(crate) fn command(self) -> Option<&'static str> {
        match self {
            Self::FocusAddressBar => None,
            Self::FocusCommandMode => None,
            Self::OpenNewTab => Some(">new-tab"),
            Self::CloseCurrentTab => Some(">close-tab"),
            Self::RestoreClosedTab => Some(">restore-tab"),
            Self::SelectNextSpace => None,
            Self::SelectPreviousSpace => None,
            Self::SelectNextTab => None,
            Self::SelectPreviousTab => None,
            Self::SplitRight => Some(">split-right"),
            Self::ToggleSidebar => None,
            Self::ToggleFavoriteTab => Some(">favorite"),
            Self::OpenDownloads => Some(">open-downloads"),
            Self::OpenHistory => Some(">open-history"),
            Self::OpenSettings => Some(">open-settings"),
            Self::OpenTaskManager => Some(">open-task-manager"),
            Self::Quit => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutBinding {
    action: ShortcutAction,
    platform: ShortcutPlatform,
    keystroke: &'static str,
}

impl ShortcutBinding {
    pub(crate) fn display_keystroke(self) -> String {
        display_keystroke(self.keystroke, self.platform)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutConflict {
    pub(crate) platform: ShortcutPlatform,
    pub(crate) keystroke: &'static str,
    pub(crate) actions: Vec<ShortcutAction>,
}

pub(crate) const SHORTCUT_ACTIONS: &[ShortcutAction] = &[
    ShortcutAction::FocusAddressBar,
    ShortcutAction::FocusCommandMode,
    ShortcutAction::OpenNewTab,
    ShortcutAction::CloseCurrentTab,
    ShortcutAction::RestoreClosedTab,
    ShortcutAction::SelectNextSpace,
    ShortcutAction::SelectPreviousSpace,
    ShortcutAction::SelectNextTab,
    ShortcutAction::SelectPreviousTab,
    ShortcutAction::SplitRight,
    ShortcutAction::ToggleSidebar,
    ShortcutAction::ToggleFavoriteTab,
    ShortcutAction::OpenDownloads,
    ShortcutAction::OpenHistory,
    ShortcutAction::OpenSettings,
    ShortcutAction::OpenTaskManager,
    ShortcutAction::Quit,
];

pub(crate) const SHORTCUT_BINDINGS: &[ShortcutBinding] = &[
    shortcut(ShortcutAction::OpenNewTab, ShortcutPlatform::Macos, "cmd-t"),
    shortcut(ShortcutAction::OpenNewTab, ShortcutPlatform::WindowsLinux, "ctrl-t"),
    shortcut(ShortcutAction::SplitRight, ShortcutPlatform::Macos, "cmd-\\"),
    shortcut(ShortcutAction::SplitRight, ShortcutPlatform::WindowsLinux, "ctrl-\\"),
    shortcut(ShortcutAction::ToggleSidebar, ShortcutPlatform::Macos, "cmd-b"),
    shortcut(ShortcutAction::ToggleSidebar, ShortcutPlatform::WindowsLinux, "ctrl-b"),
    shortcut(ShortcutAction::OpenDownloads, ShortcutPlatform::Macos, "cmd-shift-j"),
    shortcut(ShortcutAction::OpenDownloads, ShortcutPlatform::WindowsLinux, "ctrl-shift-j"),
    shortcut(ShortcutAction::OpenHistory, ShortcutPlatform::Macos, "cmd-y"),
    shortcut(ShortcutAction::OpenHistory, ShortcutPlatform::WindowsLinux, "ctrl-h"),
    shortcut(ShortcutAction::OpenTaskManager, ShortcutPlatform::Macos, "cmd-escape"),
    shortcut(ShortcutAction::OpenTaskManager, ShortcutPlatform::WindowsLinux, "shift-escape"),
    shortcut(ShortcutAction::OpenSettings, ShortcutPlatform::Macos, "cmd-,"),
    shortcut(ShortcutAction::OpenSettings, ShortcutPlatform::WindowsLinux, "ctrl-,"),
    shortcut(ShortcutAction::FocusAddressBar, ShortcutPlatform::Macos, "cmd-l"),
    shortcut(ShortcutAction::FocusAddressBar, ShortcutPlatform::WindowsLinux, "ctrl-l"),
    shortcut(ShortcutAction::CloseCurrentTab, ShortcutPlatform::Macos, "cmd-w"),
    shortcut(ShortcutAction::CloseCurrentTab, ShortcutPlatform::WindowsLinux, "ctrl-w"),
    shortcut(ShortcutAction::RestoreClosedTab, ShortcutPlatform::Macos, "cmd-shift-t"),
    shortcut(ShortcutAction::RestoreClosedTab, ShortcutPlatform::WindowsLinux, "ctrl-shift-t"),
    shortcut(ShortcutAction::ToggleFavoriteTab, ShortcutPlatform::Macos, "cmd-shift-f"),
    shortcut(ShortcutAction::ToggleFavoriteTab, ShortcutPlatform::WindowsLinux, "ctrl-shift-f"),
    shortcut(ShortcutAction::FocusCommandMode, ShortcutPlatform::Macos, "cmd-shift-p"),
    shortcut(ShortcutAction::FocusCommandMode, ShortcutPlatform::WindowsLinux, "ctrl-shift-p"),
    shortcut(ShortcutAction::SelectNextSpace, ShortcutPlatform::Macos, "cmd-alt-right"),
    shortcut(ShortcutAction::SelectNextSpace, ShortcutPlatform::WindowsLinux, "ctrl-alt-right"),
    shortcut(ShortcutAction::SelectPreviousSpace, ShortcutPlatform::Macos, "cmd-alt-left"),
    shortcut(ShortcutAction::SelectPreviousSpace, ShortcutPlatform::WindowsLinux, "ctrl-alt-left"),
    shortcut(ShortcutAction::SelectNextTab, ShortcutPlatform::Macos, "cmd-shift-]"),
    shortcut(ShortcutAction::SelectNextTab, ShortcutPlatform::Macos, "cmd-alt-down"),
    shortcut(ShortcutAction::SelectNextTab, ShortcutPlatform::WindowsLinux, "ctrl-tab"),
    shortcut(ShortcutAction::SelectNextTab, ShortcutPlatform::WindowsLinux, "ctrl-alt-down"),
    shortcut(ShortcutAction::SelectPreviousTab, ShortcutPlatform::Macos, "cmd-shift-["),
    shortcut(ShortcutAction::SelectPreviousTab, ShortcutPlatform::Macos, "cmd-alt-up"),
    shortcut(ShortcutAction::SelectPreviousTab, ShortcutPlatform::WindowsLinux, "ctrl-shift-tab"),
    shortcut(ShortcutAction::SelectPreviousTab, ShortcutPlatform::WindowsLinux, "ctrl-alt-up"),
    shortcut(ShortcutAction::Quit, ShortcutPlatform::Macos, "cmd-q"),
];

const fn shortcut(
    action: ShortcutAction,
    platform: ShortcutPlatform,
    keystroke: &'static str,
) -> ShortcutBinding {
    ShortcutBinding { action, platform, keystroke }
}

pub(crate) fn bind_shortcuts(cx: &mut App) {
    cx.bind_keys(SHORTCUT_BINDINGS.iter().map(|binding| binding.key_binding()));
}

pub(crate) fn bindings_for_action(
    action: ShortcutAction,
    platform: ShortcutPlatform,
) -> impl Iterator<Item = ShortcutBinding> {
    SHORTCUT_BINDINGS
        .iter()
        .copied()
        .filter(move |binding| binding.action == action && binding.platform == platform)
}

pub(crate) fn shortcut_conflicts() -> Vec<ShortcutConflict> {
    let mut bindings_by_key: BTreeMap<(ShortcutPlatform, &'static str), Vec<ShortcutAction>> =
        BTreeMap::new();

    for binding in SHORTCUT_BINDINGS {
        bindings_by_key
            .entry((binding.platform, binding.keystroke))
            .or_default()
            .push(binding.action);
    }

    bindings_by_key
        .into_iter()
        .filter_map(|((platform, keystroke), actions)| {
            (actions.len() > 1).then_some(ShortcutConflict { platform, keystroke, actions })
        })
        .collect()
}

impl ShortcutBinding {
    fn key_binding(self) -> KeyBinding {
        match self.action {
            ShortcutAction::CloseCurrentTab => {
                KeyBinding::new(self.keystroke, CloseCurrentTab, None)
            }
            ShortcutAction::FocusAddressBar => {
                KeyBinding::new(self.keystroke, FocusAddressBar, None)
            }
            ShortcutAction::FocusCommandMode => {
                KeyBinding::new(self.keystroke, FocusCommandMode, None)
            }
            ShortcutAction::OpenDownloads => KeyBinding::new(self.keystroke, OpenDownloads, None),
            ShortcutAction::OpenHistory => KeyBinding::new(self.keystroke, OpenHistory, None),
            ShortcutAction::OpenNewTab => KeyBinding::new(self.keystroke, OpenNewTab, None),
            ShortcutAction::OpenSettings => KeyBinding::new(self.keystroke, OpenSettings, None),
            ShortcutAction::OpenTaskManager => {
                KeyBinding::new(self.keystroke, OpenTaskManager, None)
            }
            ShortcutAction::Quit => KeyBinding::new(self.keystroke, Quit, None),
            ShortcutAction::RestoreClosedTab => {
                KeyBinding::new(self.keystroke, RestoreClosedTab, None)
            }
            ShortcutAction::SelectNextSpace => {
                KeyBinding::new(self.keystroke, SelectNextSpace, None)
            }
            ShortcutAction::SelectNextTab => KeyBinding::new(self.keystroke, SelectNextTab, None),
            ShortcutAction::SelectPreviousSpace => {
                KeyBinding::new(self.keystroke, SelectPreviousSpace, None)
            }
            ShortcutAction::SelectPreviousTab => {
                KeyBinding::new(self.keystroke, SelectPreviousTab, None)
            }
            ShortcutAction::SplitRight => KeyBinding::new(self.keystroke, SplitRight, None),
            ShortcutAction::ToggleFavoriteTab => {
                KeyBinding::new(self.keystroke, ToggleFavoriteTab, None)
            }
            ShortcutAction::ToggleSidebar => KeyBinding::new(self.keystroke, ToggleSidebar, None),
        }
    }
}

fn display_keystroke(keystroke: &str, platform: ShortcutPlatform) -> String {
    keystroke
        .split('-')
        .map(|part| display_key_part(part, platform))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn display_key_part(part: &str, platform: ShortcutPlatform) -> String {
    match part {
        "cmd" => "Cmd".to_string(),
        "ctrl" => "Ctrl".to_string(),
        "alt" if platform == ShortcutPlatform::Macos => "Option".to_string(),
        "alt" => "Alt".to_string(),
        "shift" => "Shift".to_string(),
        "escape" => "Esc".to_string(),
        value => value.to_ascii_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SHORTCUT_ACTIONS, SHORTCUT_BINDINGS, ShortcutAction, ShortcutPlatform, bindings_for_action,
        shortcut_conflicts,
    };

    #[test]
    fn registered_shortcuts_have_no_conflicts() {
        assert_eq!(shortcut_conflicts(), Vec::new());
    }

    #[test]
    fn open_settings_shortcut_has_platform_bindings() {
        let bindings = bindings_for_action(ShortcutAction::OpenSettings, ShortcutPlatform::Macos)
            .chain(bindings_for_action(
                ShortcutAction::OpenSettings,
                ShortcutPlatform::WindowsLinux,
            ))
            .map(|binding| binding.display_keystroke())
            .collect::<Vec<_>>();

        assert_eq!(bindings, vec!["Cmd + ,".to_string(), "Ctrl + ,".to_string()]);
    }

    #[test]
    fn toggle_sidebar_shortcut_has_platform_bindings() {
        let bindings = bindings_for_action(ShortcutAction::ToggleSidebar, ShortcutPlatform::Macos)
            .chain(bindings_for_action(
                ShortcutAction::ToggleSidebar,
                ShortcutPlatform::WindowsLinux,
            ))
            .map(|binding| binding.display_keystroke())
            .collect::<Vec<_>>();

        assert_eq!(bindings, vec!["Cmd + B".to_string(), "Ctrl + B".to_string()]);
    }

    #[test]
    fn space_switch_shortcuts_have_platform_bindings() {
        let next_bindings =
            bindings_for_action(ShortcutAction::SelectNextSpace, ShortcutPlatform::Macos)
                .chain(bindings_for_action(
                    ShortcutAction::SelectNextSpace,
                    ShortcutPlatform::WindowsLinux,
                ))
                .map(|binding| binding.display_keystroke())
                .collect::<Vec<_>>();
        let previous_bindings =
            bindings_for_action(ShortcutAction::SelectPreviousSpace, ShortcutPlatform::Macos)
                .chain(bindings_for_action(
                    ShortcutAction::SelectPreviousSpace,
                    ShortcutPlatform::WindowsLinux,
                ))
                .map(|binding| binding.display_keystroke())
                .collect::<Vec<_>>();

        assert_eq!(
            next_bindings,
            vec!["Cmd + Option + RIGHT".to_string(), "Ctrl + Alt + RIGHT".to_string()]
        );
        assert_eq!(
            previous_bindings,
            vec!["Cmd + Option + LEFT".to_string(), "Ctrl + Alt + LEFT".to_string()]
        );
    }

    #[test]
    fn every_declared_action_has_a_binding() {
        for action in SHORTCUT_ACTIONS {
            assert!(SHORTCUT_BINDINGS.iter().any(|binding| binding.action == *action));
        }
    }
}
