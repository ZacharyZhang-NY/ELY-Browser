mod shell;

use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, SystemMenuType, WindowBounds,
    WindowOptions, actions, px, size,
};
use shell::ElyShell;

actions!(ely_app, [Quit]);

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(quit);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus(vec![Menu {
            name: "ELY Browser".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit ELY Browser", Quit),
            ],
        }]);

        let bounds = Bounds::centered(None, size(px(1240.0), px(780.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            |window, cx| {
                let shell = cx.new(|cx| ElyShell::new(window, cx));
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
