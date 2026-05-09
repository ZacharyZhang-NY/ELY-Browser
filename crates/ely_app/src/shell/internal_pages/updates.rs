use std::env;

use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::brand::SYNC_SERVICE_NAME;

use super::{ElyShell, render_canvas_surface};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_REVISION: &str = env!("ELY_BUILD_REVISION");
const RELEASE_MANIFEST_PATH: &str = "/api/releases/manifest";
const RELEASE_SIGNATURE_PATH: &str = "/api/releases/signature";
const RELEASE_MANIFEST_CACHE: &str = "release_manifest_cache";
const RELEASE_INTEGRITY: &str = "SHA-256 + Ed25519";

impl ElyShell {
    pub(super) fn render_updates_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_updates_header(snapshot))
                .child(render_updates_summary())
                .child(render_update_contract_rows()),
        )
    }
}

fn render_updates_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Updates"))
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
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(IconName::LoaderCircle)
                .child(format!("Build {BUILD_REVISION}")),
        )
        .into_any_element()
}

fn render_updates_summary() -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_4()
        .py_3()
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
                .child(div().text_color(rgb(colors::PRIMARY)).child(IconName::LoaderCircle))
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
                                .text_color(rgb(colors::INK))
                                .child("Release Manifest Contract"),
                        )
                        .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(
                            format!(
                                "{} target through {SYNC_SERVICE_NAME} release APIs",
                                release_target()
                            ),
                        )),
                ),
        )
        .child(div().text_xs().font_semibold().text_color(rgb(colors::SUCCESS)).child(APP_VERSION))
        .into_any_element()
}

fn render_update_contract_rows() -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .child(update_row(IconName::Info, "Current Version", APP_VERSION, "Cargo package version"))
        .child(update_row(IconName::GitHub, "Build Revision", BUILD_REVISION, "Git revision"))
        .child(update_row(
            IconName::Globe,
            "Release Target",
            release_target(),
            "Platform and architecture",
        ))
        .child(update_row(
            IconName::File,
            "Manifest API",
            RELEASE_MANIFEST_PATH,
            format!("KV namespace: {RELEASE_MANIFEST_CACHE}"),
        ))
        .child(update_row(
            IconName::File,
            "Signature API",
            signature_query_path(),
            "Targeted release signature document",
        ))
        .child(update_row(
            IconName::CircleCheck,
            "Artifact Integrity",
            RELEASE_INTEGRITY,
            "Release manifest requires package hash and signature",
        ))
        .into_any_element()
}

fn update_row(
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
                .child(div().text_color(rgb(colors::MUTED_SOFT)).child(icon))
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
                                .child(label),
                        )
                        .child(
                            div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(detail),
                        ),
                ),
        )
        .child(
            div()
                .max_w(px(360.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child(value),
        )
        .into_any_element()
}

fn release_target() -> String {
    format!("{} / {}", env::consts::OS, env::consts::ARCH)
}

fn signature_query_path() -> String {
    format!(
        "{RELEASE_SIGNATURE_PATH}?platform={}&architecture={}&version={APP_VERSION}",
        env::consts::OS,
        env::consts::ARCH
    )
}
