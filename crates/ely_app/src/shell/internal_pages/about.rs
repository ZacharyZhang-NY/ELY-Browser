use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::brand::{
    AUTH_CALLBACK_URL, COMPANY_NAME, DEEP_LINK_PREFIX, DEEP_LINK_SCHEME, FORMAL_PRODUCT_NAME,
    PRODUCT_NAME, SYNC_SERVICE_NAME,
};

use super::{ElyShell, render_canvas_surface};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_REVISION: &str = env!("ELY_BUILD_REVISION");
const WORKSPACE_LICENSE: &str = env!("ELY_WORKSPACE_LICENSE");
const GPUI_VERSION: &str = env!("ELY_GPUI_VERSION");
const GPUI_COMPONENT_VERSION: &str = env!("ELY_GPUI_COMPONENT_VERSION");
const SERVO_VERSION: &str = env!("ELY_SERVO_VERSION");

impl ElyShell {
    pub(super) fn render_about_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_about_header())
                .child(render_about_rows(snapshot)),
        )
    }
}

fn render_about_header() -> AnyElement {
    div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(rgb(colors::ink()))
                        .child(format!("About {PRODUCT_NAME}")),
                )
                .child(div().text_sm().text_color(rgb(colors::muted())).child(FORMAL_PRODUCT_NAME)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::muted()))
                .child(IconName::Info)
                .child(format!("Build {BUILD_REVISION}")),
        )
        .into_any_element()
}

fn render_about_rows(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::hairline()))
        .child(about_row(
            IconName::Building2,
            "Product",
            FORMAL_PRODUCT_NAME,
            "Native Rust desktop browser",
        ))
        .child(about_row(IconName::Building2, "Publisher", COMPANY_NAME, "Brand owner"))
        .child(about_row(IconName::Globe, "Service", SYNC_SERVICE_NAME, "Sync and release service"))
        .child(about_row(
            IconName::ExternalLink,
            "Protocol",
            DEEP_LINK_PREFIX,
            format!("scheme: {DEEP_LINK_SCHEME}; auth: {AUTH_CALLBACK_URL}"),
        ))
        .child(about_row(IconName::Info, "Version", APP_VERSION, "Cargo package version"))
        .child(about_row(IconName::GitHub, "Build", BUILD_REVISION, "Build source"))
        .child(about_row(
            IconName::Frame,
            "GPUI",
            format!("gpui {GPUI_VERSION}"),
            "Native desktop renderer",
        ))
        .child(about_row(
            IconName::LayoutDashboard,
            "Components",
            format!("gpui-component {GPUI_COMPONENT_VERSION}"),
            "UI component toolkit",
        ))
        .child(about_row(
            IconName::Globe,
            "Servo",
            format!("servo {SERVO_VERSION}"),
            "Browser engine crate",
        ))
        .child(about_row(
            IconName::BookOpen,
            "License",
            WORKSPACE_LICENSE,
            "Workspace package license",
        ))
        .child(about_row(
            IconName::CircleUser,
            "Runtime",
            &snapshot.active_profile_name,
            format!("{} - {} open tabs", snapshot.active_space_name, snapshot.tabs.len()),
        ))
        .into_any_element()
}

fn about_row(
    icon: IconName,
    label: &'static str,
    value: impl Into<String>,
    detail: impl Into<String>,
) -> AnyElement {
    let value = value.into();
    let detail = detail.into();

    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::hairline()))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(colors::muted_soft())).child(icon))
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
                                .text_color(rgb(colors::ink()))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::muted()))
                                .child(detail),
                        ),
                ),
        )
        .child(
            div()
                .max_w(px(300.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::ink()))
                .child(value),
        )
        .into_any_element()
}
