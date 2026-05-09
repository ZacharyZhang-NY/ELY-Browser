use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, point, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, input::Input};

use crate::shell::ElyShell;
use crate::shell::sidebar::render_command_bar_identity;

pub(crate) fn render_topbar(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    active_tab: &BrowserTab,
    sidebar_collapsed: bool,
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
        .border_color(rgba(colors::DIVIDER))
        .when(sidebar_collapsed, |el| {
            el.child(render_command_bar_identity(snapshot, 56.0, true))
        })
        .child(render_nav_arrow("nav-back", IconName::ChevronLeft))
        .child(render_nav_arrow("nav-forward", IconName::ChevronRight))
        .child(render_omnibar(shell, active_tab, cx))
        .child(render_topbar_action(
            "share-url",
            IconName::Copy,
            cx,
            |shell, window, cx| shell.copy_active_tab_url(window, cx),
        ))
        .child(render_topbar_action(
            "open-downloads",
            IconName::Folder,
            cx,
            |shell, window, cx| shell.open_downloads(window, cx),
        ))
        .child(render_topbar_action(
            "toggle-theme",
            IconName::Moon,
            cx,
            |shell, window, cx| shell.open_internal_tab("ely://settings/appearance", window, cx),
        ))
        .child(render_topbar_action(
            "open-menu",
            IconName::Menu,
            cx,
            |shell, window, cx| shell.open_internal_tab("ely://settings", window, cx),
        ))
        .into_any_element()
}

fn render_omnibar(
    shell: &mut ElyShell,
    active_tab: &BrowserTab,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let favorite_active = active_tab.flags().favorite;
    let favorite_icon = if favorite_active {
        IconName::Star
    } else {
        IconName::StarOff
    };

    div()
        .flex_1()
        .h(px(spacing::OMNIBAR_HEIGHT))
        .rounded(px(spacing::RADIUS_PILL))
        .bg(rgba(OMNIBAR_BG))
        .shadow(soft_shadow())
        .pl(px(14.0))
        .pr(px(8.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(IconName::Search),
        )
        .child(
            div().flex_1().child(
                Input::new(&shell.command_input)
                    .appearance(false)
                    .cleanable(true),
            ),
        )
        .child(render_omnibar_chip(
            "omnibar-filters",
            IconName::Settings2,
            false,
            cx,
            |shell, window, cx| shell.focus_address_bar(window, cx),
        ))
        .child(render_omnibar_chip(
            "omnibar-favorite",
            favorite_icon,
            favorite_active,
            cx,
            |shell, _window, cx| shell.toggle_active_tab_favorite(cx),
        ))
        .into_any_element()
}

fn render_omnibar_chip<F>(
    id: &'static str,
    icon: IconName,
    active: bool,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    let color = if active { colors::ACCENT } else { colors::INK_4 };
    div()
        .id(SharedString::from(id))
        .size(px(22.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CHIP_HOVER_BG)).text_color(rgb(colors::INK)))
        .active(|style| style.opacity(0.7))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(icon)
        .into_any_element()
}

fn render_nav_arrow(id: &'static str, icon: IconName) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .size(px(30.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(rgb(colors::INK_3))
        .hover(|style| style.bg(rgba(OMNIBAR_BG)).text_color(rgb(colors::INK)))
        .active(|style| style.opacity(0.82))
        .child(icon)
        .into_any_element()
}

fn render_topbar_action<F>(
    id: &'static str,
    icon: IconName,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from(id))
        .size(px(30.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(rgb(colors::INK_3))
        .hover(|style| style.bg(rgba(OMNIBAR_BG)).text_color(rgb(colors::INK)))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(icon)
        .into_any_element()
}

const OMNIBAR_BG: u32 = 0xffffff8c;
const CHIP_HOVER_BG: u32 = 0xffffffd9;

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
