mod shell;

use gpui::{
    App, AppContext, Application, Bounds, Focusable, KeyBinding, Menu, MenuItem, SystemMenuType,
    WindowBounds, WindowOptions, actions, px, size,
};
use gpui_component_assets::Assets;
use shell::ElyShell;

actions!(
    ely_app,
    [
        CloseCurrentTab,
        FocusAddressBar,
        OpenDownloads,
        OpenNewTab,
        Quit,
        RestoreClosedTab,
        SelectNextTab,
        SelectPreviousTab,
        ToggleFavoriteTab,
        TogglePinnedTab,
    ]
);

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(quit);
        cx.bind_keys([
            KeyBinding::new("cmd-t", OpenNewTab, None),
            KeyBinding::new("ctrl-t", OpenNewTab, None),
            KeyBinding::new("cmd-shift-j", OpenDownloads, None),
            KeyBinding::new("ctrl-shift-j", OpenDownloads, None),
            KeyBinding::new("cmd-l", FocusAddressBar, None),
            KeyBinding::new("ctrl-l", FocusAddressBar, None),
            KeyBinding::new("cmd-w", CloseCurrentTab, None),
            KeyBinding::new("ctrl-w", CloseCurrentTab, None),
            KeyBinding::new("cmd-shift-t", RestoreClosedTab, None),
            KeyBinding::new("ctrl-shift-t", RestoreClosedTab, None),
            KeyBinding::new("cmd-shift-f", ToggleFavoriteTab, None),
            KeyBinding::new("ctrl-shift-f", ToggleFavoriteTab, None),
            KeyBinding::new("cmd-shift-p", TogglePinnedTab, None),
            KeyBinding::new("ctrl-shift-p", TogglePinnedTab, None),
            KeyBinding::new("cmd-shift-]", SelectNextTab, None),
            KeyBinding::new("ctrl-tab", SelectNextTab, None),
            KeyBinding::new("cmd-shift-[", SelectPreviousTab, None),
            KeyBinding::new("ctrl-shift-tab", SelectPreviousTab, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "ELY Browser".into(),
                items: vec![
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Quit ELY Browser", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("New Tab", OpenNewTab),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", CloseCurrentTab),
                    MenuItem::separator(),
                    MenuItem::action("Restore Closed Tab", RestoreClosedTab),
                    MenuItem::separator(),
                    MenuItem::action("Open Downloads", OpenDownloads),
                    MenuItem::separator(),
                    MenuItem::action("Toggle Pin", TogglePinnedTab),
                ],
            },
            Menu {
                name: "Bookmarks".into(),
                items: vec![MenuItem::action("Toggle Favorite", ToggleFavoriteTab)],
            },
        ]);

        let bounds = Bounds::centered(None, size(px(1240.0), px(780.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            |window, cx| {
                let shell = cx.new(|cx| ElyShell::new(window, cx));
                let focus_handle = shell.focus_handle(cx);
                window.defer(cx, move |window, cx| {
                    if window.focused(cx).is_none() {
                        focus_handle.focus(window);
                    }
                });
                cx.new(|cx| gpui_component::Root::new(shell, window, cx))
            },
        );

        if opened.is_ok() {
            cx.activate(true);
        } else {
            cx.quit();
        }
    });
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
