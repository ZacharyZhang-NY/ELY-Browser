use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

#[derive(Clone)]
struct SettingsRoute {
    icon: IconName,
    title: &'static str,
    detail: &'static str,
    route: &'static str,
}

const SETTINGS_ROUTES: &[SettingsRoute] = &[
    SettingsRoute {
        icon: IconName::LayoutDashboard,
        title: "Sidebar & Tabs",
        detail: "Vertical tabs, pinned area, and auto archive policy.",
        route: "ely://settings/sidebar-tabs",
    },
    SettingsRoute {
        icon: IconName::CircleUser,
        title: "Profiles",
        detail: "Profile identity, color, and download policy.",
        route: "ely://settings/profiles",
    },
    SettingsRoute {
        icon: IconName::Globe,
        title: "Sync",
        detail: "Local sync state and object scope.",
        route: "ely://settings/sync",
    },
    SettingsRoute {
        icon: IconName::Asterisk,
        title: "Plugins",
        detail: "Installed plugin control and audit trail.",
        route: "ely://settings/plugins",
    },
    SettingsRoute {
        icon: IconName::Info,
        title: "About",
        detail: "Build, runtime, license, and product information.",
        route: "ely://about",
    },
];

impl ElyShell {
    pub(super) fn render_settings_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_settings_header(snapshot))
                .child(render_settings_summary(snapshot))
                .child(render_settings_routes(cx)),
        )
    }
}

fn render_settings_header(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Settings"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} routes", SETTINGS_ROUTES.len())),
        )
        .into_any_element()
}

fn render_settings_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .border_t_1()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .children([
            settings_metric("Profiles", snapshot.profiles.len()),
            settings_metric("Plugins", snapshot.installed_plugins.len()),
            settings_metric("Sync objects", snapshot.sync_status.objects().len()),
            settings_metric("Open tabs", snapshot.tabs.len()),
        ])
        .into_any_element()
}

fn settings_metric(label: &'static str, value: usize) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div().text_sm().font_semibold().text_color(rgb(colors::INK)).child(value.to_string()),
        )
        .into_any_element()
}

fn render_settings_routes(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .children(
            SETTINGS_ROUTES
                .iter()
                .enumerate()
                .map(|(index, route)| render_settings_route(index, route, cx)),
        )
        .into_any_element()
}

fn render_settings_route(
    index: usize,
    settings_route: &SettingsRoute,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let route = settings_route.route;

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(colors::MUTED)).child(settings_route.icon.clone()))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .text_color(rgb(colors::INK))
                                .child(settings_route.title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(settings_route.detail),
                        ),
                ),
        )
        .child(
            Button::new(("open-settings-route", index))
                .ghost()
                .xsmall()
                .label("Open")
                .tooltip(settings_route.title)
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.open_internal_tab(route, window, cx);
                })),
        )
        .into_any_element()
}
