use gpui::{App, KeyBinding};
use serde::{Deserialize, Serialize};

mod profile;

pub(crate) use profile::{
    ELYKEYS_FILE_EXTENSION, ShortcutProfile, parse_shortcut_profile_json,
    shortcut_rebinding_key_bindings,
};

use crate::{
    CloseCurrentTab, FocusAddressBar, FocusCommandMode, OpenDownloads, OpenHistory, OpenNewTab,
    OpenPrivateWindow, OpenSettings, OpenTaskManager, Quit, ReloadTab, ResetZoom, RestoreClosedTab,
    SelectNextSpace, SelectNextTab, SelectPreviousSpace, SelectPreviousTab, SplitRight,
    ToggleFavoriteTab, ToggleSidebar, ZoomIn, ZoomOut,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ShortcutAction {
    FocusAddressBar,
    FocusCommandMode,
    OpenNewTab,
    OpenPrivateWindow,
    CloseCurrentTab,
    ReloadTab,
    RestoreClosedTab,
    SelectNextSpace,
    SelectPreviousSpace,
    SelectNextTab,
    SelectPreviousTab,
    SplitRight,
    ToggleSidebar,
    ToggleFavoriteTab,
    ZoomIn,
    ZoomOut,
    ResetZoom,
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
            Self::OpenPrivateWindow => "New Private Window",
            Self::CloseCurrentTab => "Close Tab",
            Self::ReloadTab => "Reload Tab",
            Self::RestoreClosedTab => "Restore Closed Tab",
            Self::SelectNextSpace => "Next Space",
            Self::SelectPreviousSpace => "Previous Space",
            Self::SelectNextTab => "Next Tab",
            Self::SelectPreviousTab => "Previous Tab",
            Self::SplitRight => "Split Right",
            Self::ToggleSidebar => "Toggle Sidebar",
            Self::ToggleFavoriteTab => "Toggle Favorite",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ResetZoom => "Reset Zoom",
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
            | Self::OpenPrivateWindow
            | Self::CloseCurrentTab
            | Self::ReloadTab
            | Self::RestoreClosedTab
            | Self::SelectNextSpace
            | Self::SelectPreviousSpace
            | Self::SelectNextTab
            | Self::SelectPreviousTab
            | Self::SplitRight
            | Self::ToggleSidebar
            | Self::ToggleFavoriteTab
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ResetZoom => "Tabs",
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
            Self::OpenPrivateWindow => None,
            Self::CloseCurrentTab => Some(">close-tab"),
            Self::ReloadTab => None,
            Self::RestoreClosedTab => Some(">restore-tab"),
            Self::SelectNextSpace => None,
            Self::SelectPreviousSpace => None,
            Self::SelectNextTab => None,
            Self::SelectPreviousTab => None,
            Self::SplitRight => Some(">split-right"),
            Self::ToggleSidebar => None,
            Self::ToggleFavoriteTab => Some(">favorite"),
            Self::ZoomIn => Some(">zoom-in"),
            Self::ZoomOut => Some(">zoom-out"),
            Self::ResetZoom => Some(">reset-zoom"),
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
    pub(crate) fn action(self) -> ShortcutAction {
        self.action
    }

    pub(crate) fn platform(self) -> ShortcutPlatform {
        self.platform
    }

    pub(crate) fn keystroke(self) -> &'static str {
        self.keystroke
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutConflict {
    pub(crate) platform: ShortcutPlatform,
    pub(crate) keystroke: String,
    pub(crate) actions: Vec<ShortcutAction>,
}

pub(crate) const SHORTCUT_ACTIONS: &[ShortcutAction] = &[
    ShortcutAction::FocusAddressBar,
    ShortcutAction::FocusCommandMode,
    ShortcutAction::OpenNewTab,
    ShortcutAction::OpenPrivateWindow,
    ShortcutAction::CloseCurrentTab,
    ShortcutAction::ReloadTab,
    ShortcutAction::RestoreClosedTab,
    ShortcutAction::SelectNextSpace,
    ShortcutAction::SelectPreviousSpace,
    ShortcutAction::SelectNextTab,
    ShortcutAction::SelectPreviousTab,
    ShortcutAction::SplitRight,
    ShortcutAction::ToggleSidebar,
    ShortcutAction::ToggleFavoriteTab,
    ShortcutAction::ZoomIn,
    ShortcutAction::ZoomOut,
    ShortcutAction::ResetZoom,
    ShortcutAction::OpenDownloads,
    ShortcutAction::OpenHistory,
    ShortcutAction::OpenSettings,
    ShortcutAction::OpenTaskManager,
    ShortcutAction::Quit,
];

pub(crate) const SHORTCUT_BINDINGS: &[ShortcutBinding] = &[
    shortcut(ShortcutAction::OpenNewTab, ShortcutPlatform::Macos, "cmd-t"),
    shortcut(ShortcutAction::OpenNewTab, ShortcutPlatform::WindowsLinux, "ctrl-t"),
    shortcut(ShortcutAction::ReloadTab, ShortcutPlatform::Macos, "cmd-r"),
    shortcut(ShortcutAction::ReloadTab, ShortcutPlatform::WindowsLinux, "ctrl-r"),
    shortcut(ShortcutAction::OpenPrivateWindow, ShortcutPlatform::Macos, "cmd-shift-n"),
    shortcut(ShortcutAction::OpenPrivateWindow, ShortcutPlatform::WindowsLinux, "ctrl-shift-n"),
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
    shortcut(ShortcutAction::FocusAddressBar, ShortcutPlatform::Macos, "cmd-k"),
    shortcut(ShortcutAction::FocusAddressBar, ShortcutPlatform::WindowsLinux, "ctrl-l"),
    shortcut(ShortcutAction::FocusAddressBar, ShortcutPlatform::WindowsLinux, "ctrl-k"),
    shortcut(ShortcutAction::CloseCurrentTab, ShortcutPlatform::Macos, "cmd-w"),
    shortcut(ShortcutAction::CloseCurrentTab, ShortcutPlatform::WindowsLinux, "ctrl-w"),
    shortcut(ShortcutAction::RestoreClosedTab, ShortcutPlatform::Macos, "cmd-shift-t"),
    shortcut(ShortcutAction::RestoreClosedTab, ShortcutPlatform::WindowsLinux, "ctrl-shift-t"),
    shortcut(ShortcutAction::ToggleFavoriteTab, ShortcutPlatform::Macos, "cmd-shift-f"),
    shortcut(ShortcutAction::ToggleFavoriteTab, ShortcutPlatform::WindowsLinux, "ctrl-shift-f"),
    shortcut(ShortcutAction::ZoomIn, ShortcutPlatform::Macos, "cmd-="),
    shortcut(ShortcutAction::ZoomIn, ShortcutPlatform::WindowsLinux, "ctrl-="),
    shortcut(ShortcutAction::ZoomOut, ShortcutPlatform::Macos, "cmd--"),
    shortcut(ShortcutAction::ZoomOut, ShortcutPlatform::WindowsLinux, "ctrl--"),
    shortcut(ShortcutAction::ResetZoom, ShortcutPlatform::Macos, "cmd-0"),
    shortcut(ShortcutAction::ResetZoom, ShortcutPlatform::WindowsLinux, "ctrl-0"),
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

/// Only the current platform's keys may reach GPUI: registering the other
/// platform's `ctrl-*` set on macOS hijacks readline editing inside every
/// text input (and `cmd-*` is meaningless elsewhere).
fn platform_bindings() -> impl Iterator<Item = &'static ShortcutBinding> {
    let platform = if cfg!(target_os = "macos") {
        ShortcutPlatform::Macos
    } else {
        ShortcutPlatform::WindowsLinux
    };
    SHORTCUT_BINDINGS.iter().filter(move |binding| binding.platform == platform)
}

pub(crate) fn bind_shortcuts(cx: &mut App) {
    cx.bind_keys(platform_bindings().map(|binding| binding.key_binding()));
}

impl ShortcutBinding {
    fn key_binding(self) -> KeyBinding {
        key_binding_for_action(self.action, self.keystroke)
    }
}

pub(crate) fn key_binding_for_action(action: ShortcutAction, keystroke: &str) -> KeyBinding {
    match action {
        ShortcutAction::CloseCurrentTab => KeyBinding::new(keystroke, CloseCurrentTab, None),
        ShortcutAction::FocusAddressBar => KeyBinding::new(keystroke, FocusAddressBar, None),
        ShortcutAction::FocusCommandMode => KeyBinding::new(keystroke, FocusCommandMode, None),
        ShortcutAction::OpenDownloads => KeyBinding::new(keystroke, OpenDownloads, None),
        ShortcutAction::OpenHistory => KeyBinding::new(keystroke, OpenHistory, None),
        ShortcutAction::OpenNewTab => KeyBinding::new(keystroke, OpenNewTab, None),
        ShortcutAction::OpenPrivateWindow => KeyBinding::new(keystroke, OpenPrivateWindow, None),
        ShortcutAction::OpenSettings => KeyBinding::new(keystroke, OpenSettings, None),
        ShortcutAction::OpenTaskManager => KeyBinding::new(keystroke, OpenTaskManager, None),
        ShortcutAction::Quit => KeyBinding::new(keystroke, Quit, None),
        ShortcutAction::ReloadTab => KeyBinding::new(keystroke, ReloadTab, None),
        ShortcutAction::ResetZoom => KeyBinding::new(keystroke, ResetZoom, None),
        ShortcutAction::RestoreClosedTab => KeyBinding::new(keystroke, RestoreClosedTab, None),
        ShortcutAction::SelectNextSpace => KeyBinding::new(keystroke, SelectNextSpace, None),
        ShortcutAction::SelectNextTab => KeyBinding::new(keystroke, SelectNextTab, None),
        ShortcutAction::SelectPreviousSpace => {
            KeyBinding::new(keystroke, SelectPreviousSpace, None)
        }
        ShortcutAction::SelectPreviousTab => KeyBinding::new(keystroke, SelectPreviousTab, None),
        ShortcutAction::SplitRight => KeyBinding::new(keystroke, SplitRight, None),
        ShortcutAction::ToggleFavoriteTab => KeyBinding::new(keystroke, ToggleFavoriteTab, None),
        ShortcutAction::ToggleSidebar => KeyBinding::new(keystroke, ToggleSidebar, None),
        ShortcutAction::ZoomIn => KeyBinding::new(keystroke, ZoomIn, None),
        ShortcutAction::ZoomOut => KeyBinding::new(keystroke, ZoomOut, None),
    }
}

pub(crate) fn display_keystroke(keystroke: &str, platform: ShortcutPlatform) -> String {
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
        SHORTCUT_ACTIONS, SHORTCUT_BINDINGS, ShortcutAction, ShortcutPlatform, ShortcutProfile,
    };

    #[test]
    fn registered_shortcuts_have_no_conflicts() {
        assert_eq!(ShortcutProfile::default_profile().conflicts(), Vec::new());
    }

    #[test]
    fn open_settings_shortcut_has_platform_bindings() {
        let bindings = platform_labels(ShortcutAction::OpenSettings);

        assert_eq!(bindings, vec!["Cmd + ,".to_string(), "Ctrl + ,".to_string()]);
    }

    #[test]
    fn private_window_shortcut_has_platform_bindings() {
        let bindings = platform_labels(ShortcutAction::OpenPrivateWindow);

        assert_eq!(bindings, vec!["Cmd + Shift + N".to_string(), "Ctrl + Shift + N".to_string()]);
    }

    #[test]
    fn toggle_sidebar_shortcut_has_platform_bindings() {
        let bindings = platform_labels(ShortcutAction::ToggleSidebar);

        assert_eq!(bindings, vec!["Cmd + B".to_string(), "Ctrl + B".to_string()]);
    }

    #[test]
    fn space_switch_shortcuts_have_platform_bindings() {
        let next_bindings = platform_labels(ShortcutAction::SelectNextSpace);
        let previous_bindings = platform_labels(ShortcutAction::SelectPreviousSpace);

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
            assert!(SHORTCUT_BINDINGS.iter().any(|binding| binding.action() == *action));
        }
    }

    #[test]
    fn bound_shortcuts_are_scoped_to_the_current_platform() {
        let expected = if cfg!(target_os = "macos") {
            ShortcutPlatform::Macos
        } else {
            ShortcutPlatform::WindowsLinux
        };

        assert!(super::platform_bindings().next().is_some());
        assert!(super::platform_bindings().all(|binding| binding.platform == expected));
    }

    fn platform_labels(action: ShortcutAction) -> Vec<String> {
        let profile = ShortcutProfile::default_profile();
        [ShortcutPlatform::Macos, ShortcutPlatform::WindowsLinux]
            .into_iter()
            .map(|platform| profile.display_bindings_for_action(action, platform))
            .collect()
    }
}
