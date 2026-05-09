mod services;
mod shell;
mod shortcuts;

use gpui::{
    App, AppContext, Application, Bounds, Focusable, Menu, MenuItem, SystemMenuType,
    TitlebarOptions, WindowBounds, WindowOptions, actions, point, px, size,
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
        OpenPrivateWindow,
        OpenSettings,
        OpenTaskManager,
        Quit,
        RestoreClosedTab,
        SelectNextSpace,
        SelectNextTab,
        SelectPreviousSpace,
        SelectPreviousTab,
        SplitRight,
        ToggleFavoriteTab,
        TogglePinnedTab,
        ToggleSidebar,
    ]
);

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(quit);
        bind_shortcuts(cx);
        cx.on_action(open_private_window);
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
                    MenuItem::action("New Private Window", OpenPrivateWindow),
                    MenuItem::action("Split Right", SplitRight),
                    MenuItem::separator(),
                    MenuItem::action("Command Mode", FocusCommandMode),
                    MenuItem::action("Toggle Sidebar", ToggleSidebar),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", CloseCurrentTab),
                    MenuItem::separator(),
                    MenuItem::action("Restore Closed Tab", RestoreClosedTab),
                    MenuItem::separator(),
                    MenuItem::action("Next Space", SelectNextSpace),
                    MenuItem::action("Previous Space", SelectPreviousSpace),
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

        if open_browser_window(cx, BrowserWindowMode::Standard) {
            cx.activate(true);
        } else {
            cx.quit();
        }
    });
}

#[derive(Clone, Copy)]
enum BrowserWindowMode {
    Standard,
    Private,
}

impl BrowserWindowMode {
    fn title(self) -> &'static str {
        match self {
            Self::Standard => "ELY Browser",
            Self::Private => "ELY Browser - Private",
        }
    }
}

fn open_browser_window(cx: &mut App, mode: BrowserWindowMode) -> bool {
    let bounds = Bounds::centered(None, size(px(1240.0), px(780.0)), cx);
    let opened = cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(mode.title().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(18.0), px(24.0))),
            }),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        },
        |window, cx| {
            let shell = cx.new(|cx| match mode {
                BrowserWindowMode::Standard => ElyShell::new(window, cx),
                BrowserWindowMode::Private => ElyShell::new_private(window, cx),
            });
            let focus_handle = shell.focus_handle(cx);
            window.defer(cx, move |window, cx| {
                if window.focused(cx).is_none() {
                    focus_handle.focus(window);
                }
            });
            cx.new(|cx| gpui_component::Root::new(shell, window, cx))
        },
    );

    opened.is_ok()
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

fn open_private_window(_: &OpenPrivateWindow, cx: &mut App) {
    if open_browser_window(cx, BrowserWindowMode::Private) {
        cx.activate(true);
    }
}
