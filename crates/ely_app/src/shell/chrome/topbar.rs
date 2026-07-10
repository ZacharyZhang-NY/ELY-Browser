use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{BrowserTab, ThemeMode};
use gpui::{
    AnyElement, BoxShadow, Context, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, hsla, point,
    prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, input::Input};

use crate::shell::ElyShell;
use crate::shell::chrome::animations::chrome_motion_feedback;
use crate::shell::sidebar::render_command_bar_identity;

pub(crate) fn render_topbar(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    sidebar_collapsed: bool,
    window: &mut Window,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .h(px(spacing::TOPBAR_HEIGHT))
        .px(px(14.0))
        .gap(px(8.0))
        .flex()
        .items_center()
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .when(sidebar_collapsed, |el| el.child(render_command_bar_identity(snapshot, 56.0, true)))
        .child(render_nav_arrow(
            shell,
            "nav-back",
            IconName::ArrowLeft,
            active_tab.can_navigate_back(),
            cx,
            |shell, window, cx| shell.navigate_active_tab_back(window, cx),
        ))
        .child(render_nav_arrow(
            shell,
            "nav-forward",
            IconName::ArrowRight,
            active_tab.can_navigate_forward(),
            cx,
            |shell, window, cx| shell.navigate_active_tab_forward(window, cx),
        ))
        .child(render_omnibar(shell, active_tab, window, cx))
        .child(render_topbar_action(shell, "share-url", IconName::Copy, cx, |shell, window, cx| {
            shell.copy_active_tab_url(window, cx)
        }))
        .child(render_topbar_action(
            shell,
            "open-downloads",
            IconName::Folder,
            cx,
            |shell, window, cx| shell.open_downloads(window, cx),
        ))
        .child(render_topbar_action(
            shell,
            "toggle-theme",
            theme_mode_icon(snapshot.appearance.theme_mode()),
            cx,
            |shell, _window, cx| shell.cycle_theme_mode(cx),
        ))
        .child(render_topbar_action(shell, "open-menu", IconName::Menu, cx, |shell, window, cx| {
            shell.open_internal_tab("ely://settings", window, cx)
        }))
        .into_any_element()
}

fn render_omnibar(
    shell: &mut ElyShell,
    active_tab: &BrowserTab,
    window: &mut Window,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let favorite_active = active_tab.flags().favorite;
    let favorite_icon = if favorite_active { IconName::Star } else { IconName::StarOff };
    let active_url = active_tab.url().as_str().to_string();
    let command_focused = shell.command_input.read(cx).focus_handle(cx).is_focused(window);
    let show_styled = !command_focused && active_url != "ely://new-tab";
    let secure = super::pane_url_is_secure(active_tab);
    let omnibar_motion_target = "omnibar-content";
    let omnibar_press_id = shell.chrome_motion_animation_id(omnibar_motion_target);

    div()
        .flex_1()
        .h(px(spacing::OMNIBAR_HEIGHT))
        .rounded(px(spacing::RADIUS_PILL))
        .bg(rgba(omnibar_bg()))
        .shadow(soft_shadow())
        .px(px(14.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(render_lock_or_search(secure, show_styled))
        .child(chrome_motion_feedback(
            omnibar_press_id,
            "omnibar-content-selection",
            false,
            div()
                .id(SharedString::from("omnibar-content"))
                .flex_1()
                .min_w_0()
                .cursor_pointer()
                .hover(|style| style.opacity(0.92))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.trigger_chrome_motion(omnibar_motion_target);
                    shell.focus_address_bar(window, cx);
                }))
                .child(if show_styled {
                    render_styled_url(active_tab)
                } else {
                    render_omnibar_input(shell)
                }),
        ))
        .child(render_omnibar_chip(
            shell,
            "omnibar-filters",
            IconName::Settings2,
            false,
            cx,
            |shell, window, cx| shell.focus_address_bar(window, cx),
        ))
        .child(render_omnibar_chip(
            shell,
            "omnibar-favorite",
            favorite_icon,
            favorite_active,
            cx,
            |shell, _window, cx| shell.toggle_active_tab_favorite(cx),
        ))
        .into_any_element()
}

fn render_omnibar_input(shell: &ElyShell) -> AnyElement {
    Input::new(&shell.command_input).appearance(false).cleanable(true).into_any_element()
}

fn render_styled_url(active_tab: &BrowserTab) -> AnyElement {
    let host = active_tab.url().host().map(|host| host.to_string()).unwrap_or_default();
    let url = active_tab.url().as_str();
    let path_start = url.find("://").map(|prefix| prefix + 3).unwrap_or(0);
    let from_path = &url[path_start..];
    let path = match from_path.find('/') {
        Some(slash) => from_path[slash..].to_string(),
        None => String::new(),
    };

    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .text_size(px(13.0))
        .child(div().font_weight(FontWeight(500.0)).text_color(rgb(colors::ink())).child(host))
        .child(div().min_w_0().truncate().text_color(rgb(colors::ink_3())).child(path))
        .into_any_element()
}

fn render_lock_or_search(secure: bool, show_styled: bool) -> AnyElement {
    let icon = if show_styled && !secure { IconName::Globe } else { IconName::Search };
    div().text_color(rgb(colors::ink_3())).child(icon).into_any_element()
}

fn render_omnibar_chip<F>(
    shell: &ElyShell,
    id: &'static str,
    icon: IconName,
    active: bool,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let color = if active { colors::accent() } else { colors::ink_4() };
    let press_id = shell.chrome_motion_animation_id(id);
    let element = div()
        .id(SharedString::from(id))
        .size(px(22.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(chip_hover_bg())).text_color(rgb(colors::ink())))
        .active(|style| style.opacity(0.7))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.trigger_chrome_motion(id);
            handler(shell, window, cx);
        }))
        .child(icon);

    chrome_motion_feedback(press_id, SharedString::from(format!("{id}-selection")), active, element)
}

fn render_nav_arrow<F>(
    shell: &ElyShell,
    id: &'static str,
    icon: IconName,
    enabled: bool,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let color = if enabled { colors::ink_3() } else { colors::ink_5() };
    let press_id = if enabled { shell.chrome_motion_animation_id(id) } else { None };
    let element = div()
        .id(SharedString::from(id))
        .size(px(30.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .when(enabled, |el| {
            el.cursor_pointer()
                .hover(|style| style.bg(rgba(omnibar_bg())).text_color(rgb(colors::ink())))
                .active(|style| style.opacity(0.82))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.trigger_chrome_motion(id);
                    handler(shell, window, cx);
                }))
        })
        .child(icon);

    chrome_motion_feedback(press_id, SharedString::from(format!("{id}-selection")), false, element)
}

fn render_topbar_action<F>(
    shell: &ElyShell,
    id: &'static str,
    icon: IconName,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let press_id = shell.chrome_motion_animation_id(id);
    let element = div()
        .id(SharedString::from(id))
        .size(px(30.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(rgb(colors::ink_3()))
        .hover(|style| style.bg(rgba(omnibar_bg())).text_color(rgb(colors::ink())))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.trigger_chrome_motion(id);
            handler(shell, window, cx);
        }))
        .child(icon);

    chrome_motion_feedback(press_id, SharedString::from(format!("{id}-selection")), false, element)
}

fn omnibar_bg() -> u32 {
    colors::pick(0xffffff8c, 0x1f1d1b8c)
}
fn chip_hover_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}

/// Topbar quick-toggle icon for the current theme mode. The button
/// cycles System → Light → Dark → System, and the icon previews the
/// state the user is in: sun for light, moon for dark. System falls
/// back to moon since the bundled icon set has no combined sun-moon
/// glyph and the OS-driven mode visually leans neutral.
fn theme_mode_icon(mode: ThemeMode) -> IconName {
    match mode {
        ThemeMode::Light => IconName::Sun,
        ThemeMode::System | ThemeMode::Dark => IconName::Moon,
    }
}

fn soft_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(0.0, 0.0, 1.0, 0.7),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
        },
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.08),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
        },
    ]
}
