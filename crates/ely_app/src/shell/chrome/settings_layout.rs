use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};
use gpui_component::{IconName, scroll::ScrollableElement};

use crate::shell::ElyShell;
use crate::shell::chrome::render_appearance_form;

struct NavGroup {
    label: &'static str,
    items: &'static [NavItem],
}

struct NavItem {
    icon: IconName,
    label: &'static str,
    route: &'static str,
}

const NAV_GROUPS: &[NavGroup] = &[
    NavGroup {
        label: "GENERAL",
        items: &[
            NavItem {
                icon: IconName::Palette,
                label: "Appearance",
                route: "ely://settings/appearance",
            },
            NavItem {
                icon: IconName::PanelLeft,
                label: "Sidebar & Tabs",
                route: "ely://settings/sidebar-tabs",
            },
            NavItem {
                icon: IconName::GalleryVerticalEnd,
                label: "Spaces",
                route: "ely://settings/spaces",
            },
            NavItem {
                icon: IconName::Search,
                label: "Search",
                route: "ely://settings/search",
            },
            NavItem {
                icon: IconName::SquareTerminal,
                label: "Shortcuts",
                route: "ely://settings/shortcuts",
            },
        ],
    },
    NavGroup {
        label: "ACCOUNT",
        items: &[
            NavItem {
                icon: IconName::ChartPie,
                label: "Sync",
                route: "ely://settings/sync",
            },
            NavItem {
                icon: IconName::Eye,
                label: "Privacy & Security",
                route: "ely://settings/privacy-security",
            },
            NavItem {
                icon: IconName::CircleUser,
                label: "Profiles",
                route: "ely://settings/profiles",
            },
            NavItem {
                icon: IconName::Folder,
                label: "Downloads",
                route: "ely://settings/downloads",
            },
            NavItem {
                icon: IconName::Globe,
                label: "Site Permissions",
                route: "ely://settings/site-permissions",
            },
        ],
    },
    NavGroup {
        label: "POWER",
        items: &[
            NavItem {
                icon: IconName::Asterisk,
                label: "Plugins",
                route: "ely://settings/plugins",
            },
            NavItem {
                icon: IconName::LoaderCircle,
                label: "Updates",
                route: "ely://settings/updates",
            },
            NavItem {
                icon: IconName::Inspector,
                label: "Advanced",
                route: "ely://settings/advanced",
            },
        ],
    },
    NavGroup {
        label: "ABOUT",
        items: &[NavItem {
            icon: IconName::Info,
            label: "About",
            route: "ely://about",
        }],
    },
];

pub(crate) fn render_settings_landing(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    active_route: &str,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .flex()
        .child(render_nav_column(snapshot, active_route, cx))
        .child(render_appearance_form(shell, snapshot, cx))
        .into_any_element()
}

fn render_nav_column(
    snapshot: &BrowserSnapshot,
    active_route: &str,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .w(px(232.0))
        .h_full()
        .flex_shrink_0()
        .border_r_1()
        .border_color(rgba(colors::DIVIDER))
        .px(px(10.0))
        .pt(px(16.0))
        .pb(px(12.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .overflow_y_scrollbar()
        .child(render_nav_brand(snapshot))
        .children(NAV_GROUPS.iter().enumerate().map(|(group_index, group)| {
            render_nav_group(group_index, group, active_route, cx)
        }))
        .into_any_element()
}

fn render_nav_brand(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .px(px(12.0))
        .pb(px(12.0))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::INK))
                .child("Settings"),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(colors::INK_4))
                .child(format!(
                    "ELY 0.42 · Profile: {}",
                    snapshot.active_profile_name
                )),
        )
        .into_any_element()
}

fn render_nav_group(
    group_index: usize,
    group: &NavGroup,
    active_route: &str,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(render_nav_label(group.label))
        .children(group.items.iter().enumerate().map(|(item_index, item)| {
            render_nav_item(group_index, item_index, item, active_route, cx)
        }))
        .into_any_element()
}

fn render_nav_label(label: &'static str) -> AnyElement {
    div()
        .pt(px(8.0))
        .pb(px(4.0))
        .px(px(12.0))
        .text_size(px(10.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::INK_4))
        .child(label)
        .into_any_element()
}

fn render_nav_item(
    group_index: usize,
    item_index: usize,
    item: &NavItem,
    active_route: &str,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let active = item.route == active_route;
    let bg = if active { ACTIVE_BG } else { 0x00000000 };
    let text_color = if active { colors::INK } else { colors::INK_2 };
    let route = item.route;
    let icon = item.icon.clone();

    div()
        .id(SharedString::from(format!(
            "settings-nav-{group_index}-{item_index}"
        )))
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(12.0))
        .py(px(7.0))
        .rounded(px(8.0))
        .bg(rgba(bg))
        .text_size(px(13.0))
        .text_color(rgb(text_color))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(HOVER_BG)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(route, window, cx);
        }))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(icon),
        )
        .child(div().flex_1().truncate().child(item.label))
        .into_any_element()
}

const ACTIVE_BG: u32 = 0xffffffd9;
const HOVER_BG: u32 = 0xffffff8c;
