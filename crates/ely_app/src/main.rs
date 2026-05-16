mod brand;
mod services;
mod shell;
mod shortcuts;

use std::{
    cell::RefCell,
    env,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use ely_domain::UrlText;
use gpui::{
    AnyWindowHandle, App, AppContext, Application, Bounds, Entity, Focusable, Menu, MenuItem,
    SystemMenuType, Timer, TitlebarOptions, WindowBounds, WindowOptions, actions, point, px, size,
};
use gpui_component_assets::Assets;
use shell::ElyShell;
use shell::chrome::{TRAFFIC_LIGHT_ORIGIN_X, TRAFFIC_LIGHT_ORIGIN_Y};
use shortcuts::bind_shortcuts;

use crate::brand::{DEEP_LINK_PREFIX, PRODUCT_NAME};

actions!(
    ely_app,
    [
        CloseCurrentTab,
        DownloadCurrentPage,
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
        ResetZoom,
        SelectNextSpace,
        SelectNextTab,
        SelectPreviousSpace,
        SelectPreviousTab,
        SplitRight,
        ToggleFavoriteTab,
        TogglePinnedTab,
        ToggleSidebar,
        ZoomIn,
        ZoomOut,
    ]
);

fn main() {
    init_tracing();
    let pending_deep_links = PendingDeepLinks::default();
    pending_deep_links.push(startup_deep_links(env::args().skip(1)));
    let open_url_queue = pending_deep_links.clone();
    let application = Application::new().with_assets(Assets);

    application.on_open_urls(move |urls| open_url_queue.push(urls));
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);
        if let Err(error) = shell::chrome::register_serif_fonts(cx) {
            eprintln!("ELY: failed to register serif fonts: {error}");
        }
        // gpui-component's Input, tooltip, context menu, search popover,
        // notification surface, etc. all read their font from
        // `cx.theme().font_family` directly instead of inheriting from
        // the parent. Default theme value is `.SystemUIFont` (SF Pro on
        // macOS), so without this override the user types into the
        // omnibar in SF Pro while every other surface renders Geist —
        // exactly the "fonts still wrong" the screenshot showed.
        // Override the theme so every gpui-component sub-element uses
        // Geist too.
        gpui_component::Theme::global_mut(cx).font_family = shell::chrome::SANS_FAMILY.into();
        cx.on_action(quit);
        bind_shortcuts(cx);
        cx.on_action(open_private_window);
        cx.set_menus(vec![
            Menu {
                name: PRODUCT_NAME.into(),
                items: vec![
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action(format!("Quit {PRODUCT_NAME}"), Quit),
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
                    MenuItem::action("Download Current Page", DownloadCurrentPage),
                    MenuItem::action("Open Downloads", OpenDownloads),
                    MenuItem::separator(),
                    MenuItem::action("Zoom In", ZoomIn),
                    MenuItem::action("Zoom Out", ZoomOut),
                    MenuItem::action("Reset Zoom", ResetZoom),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", CloseCurrentTab),
                    MenuItem::separator(),
                    MenuItem::action("Restore Closed Tab", RestoreClosedTab),
                    MenuItem::separator(),
                    MenuItem::action("Next Space", SelectNextSpace),
                    MenuItem::action("Previous Space", SelectPreviousSpace),
                    MenuItem::separator(),
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

        let current_window =
            Rc::new(RefCell::new(open_browser_window(cx, BrowserWindowMode::Standard)));

        if current_window.borrow().is_none() {
            cx.quit();
            return;
        }

        route_pending_deep_links(&pending_deep_links, &current_window, cx);
        watch_deep_links(pending_deep_links, current_window, cx);
        cx.activate(true);
    });
}

#[derive(Clone)]
struct BrowserWindowTarget {
    window: AnyWindowHandle,
    shell: Entity<ElyShell>,
}

#[derive(Clone, Default)]
struct PendingDeepLinks {
    urls: Arc<Mutex<Vec<String>>>,
}

impl PendingDeepLinks {
    fn push(&self, incoming_urls: Vec<String>) {
        if let Ok(mut urls) = self.urls.lock() {
            urls.extend(incoming_urls);
        }
    }

    fn drain(&self) -> Vec<String> {
        self.urls.lock().map(|mut urls| urls.drain(..).collect()).unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum BrowserWindowMode {
    Standard,
    Private,
}

impl BrowserWindowMode {
    fn title(self) -> &'static str {
        match self {
            Self::Standard => PRODUCT_NAME,
            Self::Private => "ELY Browser - Private",
        }
    }
}

fn open_browser_window(cx: &mut App, mode: BrowserWindowMode) -> Option<BrowserWindowTarget> {
    let bounds = Bounds::centered(None, size(px(1240.0), px(780.0)), cx);
    let target = Rc::new(RefCell::new(None));
    let created_target = target.clone();
    let opened = cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(mode.title().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(
                    px(TRAFFIC_LIGHT_ORIGIN_X),
                    px(TRAFFIC_LIGHT_ORIGIN_Y),
                )),
            }),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        },
        |window, cx| {
            let shell = cx.new(|cx| match mode {
                BrowserWindowMode::Standard => ElyShell::new(window, cx),
                BrowserWindowMode::Private => ElyShell::new_private(window, cx),
            });
            *created_target.borrow_mut() =
                Some(BrowserWindowTarget { window: window.window_handle(), shell: shell.clone() });
            let focus_handle = shell.focus_handle(cx);
            window.defer(cx, move |window, cx| {
                if window.focused(cx).is_none() {
                    focus_handle.focus(window);
                }
            });
            cx.new(|cx| gpui_component::Root::new(shell, window, cx))
        },
    );

    opened.ok()?;
    target.borrow().clone()
}

fn route_pending_deep_links(
    pending_deep_links: &PendingDeepLinks,
    current_window: &Rc<RefCell<Option<BrowserWindowTarget>>>,
    cx: &mut App,
) {
    let urls = pending_deep_links.drain();
    open_deep_links(urls, current_window, cx);
}

fn watch_deep_links(
    pending_deep_links: PendingDeepLinks,
    current_window: Rc<RefCell<Option<BrowserWindowTarget>>>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        loop {
            Timer::after(Duration::from_millis(150)).await;
            let urls = pending_deep_links.drain();

            if urls.is_empty() {
                continue;
            }

            let _ = cx.update(|cx| open_deep_links(urls, &current_window, cx));
        }
    })
    .detach();
}

fn open_deep_links(
    urls: Vec<String>,
    current_window: &Rc<RefCell<Option<BrowserWindowTarget>>>,
    cx: &mut App,
) {
    let urls = urls.into_iter().filter_map(|url| parse_ely_deep_link(&url)).collect::<Vec<_>>();

    if urls.is_empty() {
        return;
    }

    let Some(mut target) = ensure_browser_window_target(current_window, cx) else {
        return;
    };

    let mut opened_any = false;
    for url in urls {
        if open_deep_link(&target, url.clone(), cx) {
            opened_any = true;
            continue;
        }

        current_window.borrow_mut().take();
        let Some(next_target) = ensure_browser_window_target(current_window, cx) else {
            return;
        };
        target = next_target;

        if open_deep_link(&target, url, cx) {
            opened_any = true;
        }
    }

    if opened_any {
        cx.activate(true);
    }
}

fn ensure_browser_window_target(
    current_window: &Rc<RefCell<Option<BrowserWindowTarget>>>,
    cx: &mut App,
) -> Option<BrowserWindowTarget> {
    if let Some(target) = current_window.borrow().clone() {
        return Some(target);
    }

    let target = open_browser_window(cx, BrowserWindowMode::Standard)?;
    current_window.borrow_mut().replace(target.clone());
    Some(target)
}

fn open_deep_link(target: &BrowserWindowTarget, url: UrlText, cx: &mut App) -> bool {
    target
        .window
        .update(cx, |_, window, cx| {
            target.shell.update(cx, |shell, cx| shell.open_url(url, window, cx));
        })
        .is_ok()
}

fn parse_ely_deep_link(value: &str) -> Option<UrlText> {
    let trimmed = value.trim();
    let scheme = trimmed.get(..DEEP_LINK_PREFIX.len())?;
    let route = trimmed.get(DEEP_LINK_PREFIX.len()..)?;

    scheme
        .eq_ignore_ascii_case(DEEP_LINK_PREFIX)
        .then(|| UrlText::parse(format!("{DEEP_LINK_PREFIX}{route}")).ok())
        .flatten()
}

fn startup_deep_links(args: impl IntoIterator<Item = String>) -> Vec<String> {
    args.into_iter()
        .filter_map(|arg| parse_ely_deep_link(&arg).map(|url| url.as_str().to_string()))
        .collect()
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

/// Install the global tracing subscriber. `RUST_LOG` drives the
/// filter; absent it, only `warn` and above leak through so day-to-day
/// runs stay quiet. The perf target is silent by default —
/// `RUST_LOG=ely::servo::perf=info` flips on the frame-time stream
/// without touching the rest of the app. We swallow re-init errors so
/// tests that share the process state with main don't blow up.
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt().with_env_filter(filter).with_target(true).try_init();
}

fn open_private_window(_: &OpenPrivateWindow, cx: &mut App) {
    if open_browser_window(cx, BrowserWindowMode::Private).is_some() {
        cx.activate(true);
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingDeepLinks, parse_ely_deep_link, startup_deep_links};

    #[test]
    fn pending_deep_links_drains_urls_in_order() {
        let pending = PendingDeepLinks::default();

        pending.push(vec!["ely://history".into(), "ely://settings".into()]);

        assert_eq!(
            pending.drain(),
            vec!["ely://history".to_string(), "ely://settings".to_string()]
        );
        assert!(pending.drain().is_empty());
    }

    #[test]
    fn parse_ely_deep_link_accepts_internal_routes() {
        let url = parse_ely_deep_link(" ely://auth/callback?code=abc ");

        assert_eq!(url.as_ref().map(|url| url.as_str()), Some("ely://auth/callback?code=abc"));
    }

    #[test]
    fn parse_ely_deep_link_normalizes_scheme_case() {
        let url = parse_ely_deep_link("ELY://history");

        assert_eq!(url.as_ref().map(|url| url.as_str()), Some("ely://history"));
    }

    #[test]
    fn parse_ely_deep_link_filters_other_schemes() {
        assert!(parse_ely_deep_link("https://example.com").is_none());
    }

    #[test]
    fn startup_deep_links_filter_and_normalize_args() {
        let links = startup_deep_links(
            ["--ignored", " ELY://history ", "https://example.com", "ely://auth/callback?code=abc"]
                .into_iter()
                .map(str::to_string),
        );

        assert_eq!(
            links,
            vec!["ely://history".to_string(), "ely://auth/callback?code=abc".to_string()]
        );
    }
}
