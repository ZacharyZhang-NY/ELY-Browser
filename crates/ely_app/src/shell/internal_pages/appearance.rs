use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use super::{ElyShell, render_canvas_surface};

struct ColorToken {
    label: &'static str,
    detail: &'static str,
    value: u32,
}

const THEME_TOKENS: &[ColorToken] = &[
    ColorToken { label: "Canvas", detail: "Browser frame background", value: colors::CANVAS },
    ColorToken {
        label: "Canvas Soft",
        detail: "Internal page backdrop",
        value: colors::CANVAS_SOFT,
    },
    ColorToken { label: "Surface", detail: "Cards and panels", value: colors::SURFACE_CARD },
    ColorToken { label: "Ink", detail: "Primary text", value: colors::INK },
    ColorToken { label: "Muted", detail: "Secondary text", value: colors::MUTED },
    ColorToken { label: "Hairline", detail: "Borders and dividers", value: colors::HAIRLINE },
    ColorToken { label: "Accent", detail: "Primary actions", value: colors::PRIMARY },
    ColorToken { label: "Success", detail: "Resolved state", value: colors::SUCCESS },
    ColorToken { label: "Error", detail: "Attention state", value: colors::ERROR },
];

impl ElyShell {
    pub(super) fn render_appearance_page(&mut self, snapshot: &BrowserSnapshot) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .child(render_appearance_header(snapshot))
                .child(render_theme_summary(snapshot))
                .child(render_color_sections(snapshot)),
        )
    }
}

fn render_appearance_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Appearance"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Space: {}", snapshot.active_space_name)),
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
                .child(IconName::Palette)
                .child(format!("{} tokens", THEME_TOKENS.len())),
        )
        .into_any_element()
}

fn render_theme_summary(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_color(rgb(colors::PRIMARY)).child(IconName::Palette))
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
                                .child("ELY Theme"),
                        )
                        .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(
                            format!(
                                "{} profile on {} space",
                                snapshot.active_profile_name, snapshot.active_space_name
                            ),
                        )),
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
                .child(color_swatch(colors::PRIMARY))
                .child(hex_label(colors::PRIMARY)),
        )
        .into_any_element()
}

fn render_color_sections(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .child(render_section_label("Theme Tokens"))
        .children(THEME_TOKENS.iter().map(render_token_row))
        .child(render_section_label("Current Context"))
        .child(render_active_space_color(snapshot))
        .child(render_active_profile_color(snapshot))
        .into_any_element()
}

fn render_section_label(label: &'static str) -> AnyElement {
    div()
        .pt_4()
        .pb_2()
        .text_xs()
        .font_semibold()
        .text_color(rgb(colors::MUTED))
        .child(label)
        .into_any_element()
}

fn render_token_row(token: &ColorToken) -> AnyElement {
    render_color_row(token.label, token.detail.to_string(), token.value)
}

fn render_active_space_color(snapshot: &BrowserSnapshot) -> AnyElement {
    let detail = format!("Space: {}", snapshot.active_space_name);
    let value = snapshot
        .spaces
        .iter()
        .find(|space| space.id() == &snapshot.active_space_id)
        .map_or(colors::ERROR, |space| space.accent_hex());

    render_color_row("Active Space Accent", detail, value)
}

fn render_active_profile_color(snapshot: &BrowserSnapshot) -> AnyElement {
    let detail = format!("Profile: {}", snapshot.active_profile_name);
    let value = snapshot
        .profiles
        .iter()
        .find(|profile| profile.id() == &snapshot.active_profile_id)
        .map_or(colors::ERROR, |profile| profile.color_hex());

    render_color_row("Active Profile Color", detail, value)
}

fn render_color_row(label: &'static str, detail: String, value: u32) -> AnyElement {
    div()
        .py_3()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div().min_w_0().flex().items_center().gap_3().child(color_swatch(value)).child(
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
                    .child(div().text_xs().truncate().text_color(rgb(colors::MUTED)).child(detail)),
            ),
        )
        .child(
            div().text_xs().font_semibold().text_color(rgb(colors::BODY)).child(hex_label(value)),
        )
        .into_any_element()
}

fn color_swatch(value: u32) -> AnyElement {
    div()
        .w(px(30.0))
        .h(px(22.0))
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE_STRONG))
        .bg(rgb(value))
        .into_any_element()
}

fn hex_label(value: u32) -> String {
    format!("#{value:06X}")
}
