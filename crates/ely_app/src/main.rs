mod services;
mod shell;
mod shortcuts;

use gpui::{
    App, AppContext, Application, Bounds, Focusable, Menu, MenuItem, SystemMenuType, WindowBounds,
    WindowOptions, actions, px, size,
};
use gpui_component_assets::Assets;
use shell::ElyShell;
use shortcuts::bind_shortcuts;

actions!(
    ely_app,
    [
        CloseCurrentTab,
        FocusAddressBar,
        FocusCommandMode,
        OpenDownloads,
        OpenHistory,
        OpenNewTab,
        OpenSettings,
        OpenTaskManager,
        Quit,
        RestoreClosedTab,
        SelectNextTab,
        SelectPreviousTab,
        SplitRight,
        ToggleFavoriteTab,
        TogglePinnedTab,
    ]
);

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(quit);
        bind_shortcuts(cx);
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
                    MenuItem::action("Split Right", SplitRight),
                    MenuItem::separator(),
                    MenuItem::action("Command Mode", FocusCommandMode),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", CloseCurrentTab),
                    MenuItem::separator(),
                    MenuItem::action("Restore Closed Tab", RestoreClosedTab),
                    MenuItem::separator(),
                    MenuItem::action("Open Downloads", OpenDownloads),
                    MenuItem::action("Open History", OpenHistory),
                    MenuItem::action("Open Task Manager", OpenTaskManager),
                    MenuItem::action("Open Settings", OpenSettings),
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
