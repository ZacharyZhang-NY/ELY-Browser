use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use ely_domain::ProfileKind;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, px, rgb,
    rgba,
};
use gpui_component::{IconName, scroll::ScrollableElement};

use super::plugin_editors_pick::{action_button, render_editors_pick};
use super::{ElyShell, render_canvas_surface};
use crate::shell::chrome::SERIF_FAMILY;

impl ElyShell {
    pub(super) fn render_plugin_catalog_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_canvas_surface(
            div()
                .size_full()
                .pt(px(40.0))
                .px(px(56.0))
                .pb(px(40.0))
                .flex()
                .flex_col()
                .gap(px(24.0))
                .child(render_hero(snapshot, cx))
                .child(render_summary(snapshot))
                .child(render_grid(snapshot, cx)),
        )
    }
}

fn render_hero(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .grid()
        .grid_cols(2)
        .gap(px(28.0))
        .child(render_hero_left(snapshot, cx))
        .child(render_editors_pick(snapshot, cx))
        .into_any_element()
}

fn render_hero_left(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    let total = snapshot.installed_plugins.len();

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_4))
                .child("PLUGINS · NATIVE TO ELY"),
        )
        .child(
            div()
                .font_family(SERIF_FAMILY)
                .text_size(px(48.0))
                .font_weight(FontWeight(400.0))
                .text_color(rgb(colors::INK))
                .child("Quiet tools. Everyday magic."),
        )
        .child(
            div()
                .max_w(px(520.0))
                .text_size(px(13.5))
                .text_color(rgb(colors::INK_2))
                .child(
                    "ELY plugins are sandboxed, theme-aware, and ship with their own controls in \
                     your sidebar — no Chrome extension framework required.",
                ),
        )
        .child(render_install_button(total, cx))
        .into_any_element()
}

fn render_install_button(total: usize, cx: &mut Context<ElyShell>) -> AnyElement {
    div().flex().child(
    div()
        .id(SharedString::from("plugin-install"))
        .h(px(42.0))
        .px(px(16.0))
        .rounded(px(12.0))
        .bg(rgba(SEARCH_BG))
        .flex()
        .items_center()
        .gap(px(10.0))
        .text_size(px(13.0))
        .text_color(rgb(colors::INK_3))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(SEARCH_BG_HOVER)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(|shell, _, window, cx| {
            shell.choose_plugin_package(window, cx);
        }))
        .child(
            div()
                .text_color(rgb(colors::INK_3))
                .child(IconName::Plus),
        )
        .child(format!(
            "Install plugin · {} active",
            total
        )))
        .into_any_element()
}

fn render_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(category_chip("All", true))
        .child(category_chip("Installed", false))
        .child(category_chip(
            sandbox_chip_label(snapshot),
            false,
        ))
        .child(
            div()
                .ml_auto()
                .text_size(px(11.5))
                .text_color(rgb(colors::INK_3))
                .child(format!(
                    "{} signed · {} high-risk",
                    snapshot.installed_plugins.len(),
                    high_risk_plugin_count(snapshot)
                )),
        )
        .into_any_element()
}

fn category_chip(label: &'static str, active: bool) -> AnyElement {
    let bg = if active { 0x1d1c1aff } else { 0xffffffb3 };
    let text_color = if active { 0xffffff } else { colors::INK_2 };

    div()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(999.0))
        .bg(rgba(bg))
        .text_size(px(12.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(text_color))
        .child(label)
        .into_any_element()
}

fn render_grid(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.installed_plugins.is_empty() {
        return render_empty_state(cx);
    }

    div()
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .grid()
        .grid_cols(4)
        .gap(px(14.0))
        .children(
            snapshot
                .installed_plugins
                .iter()
                .enumerate()
                .map(|(index, plugin)| {
                    render_plugin_card(index, plugin, &snapshot.active_profile_kind, cx)
                }),
        )
        .into_any_element()
}

fn render_empty_state(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.0))
        .py(px(60.0))
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::INK))
                .child("No plugins yet."),
        )
        .child(
            div()
                .max_w(px(420.0))
                .text_size(px(13.0))
                .text_color(rgb(colors::INK_3))
                .child(
                    "Drop a signed .rplug package on ELY to extend the browser with sandboxed \
                     tools. Plugins ship with their own sidebar controls.",
                ),
        )
        .child(action_button(
            "empty-install",
            "Install plugin",
            cx,
            |shell, window, cx| shell.choose_plugin_package(window, cx),
        ))
        .into_any_element()
}

fn render_plugin_card(
    index: usize,
    plugin: &InstalledPlugin,
    profile_kind: &ProfileKind,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let detail_route = format!("ely://plugin/{}", plugin.id().as_str());
    let enabled = plugin.enabled_for_profile(profile_kind);
    let status = if enabled { "Enabled" } else { "Disabled" };
    let status_color = if enabled { colors::SUCCESS } else { colors::INK_4 };
    let high_risk = plugin.manifest().high_risk_permissions().count();
    let glyph = plugin.manifest().name().chars().next().unwrap_or('◇').to_string();
    let initial = glyph.to_uppercase().to_string();

    div()
        .id(SharedString::from(format!("plugin-card-{index}")))
        .p(px(14.0))
        .rounded(px(16.0))
        .bg(rgba(CARD_BG))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(CARD_BG_HOVER)))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_internal_tab(&detail_route, window, cx);
        }))
        .child(
            div()
                .h(px(108.0))
                .rounded(px(10.0))
                .bg(linear_gradient(
                    135.0,
                    linear_color_stop(hsla(341.0 / 360.0, 0.78, 0.85, 1.0), 0.0),
                    linear_color_stop(hsla(228.0 / 360.0, 1.0, 0.86, 1.0), 1.0),
                ))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(36.0))
                .text_color(rgb(0xffffff))
                .child(initial),
        )
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgb(colors::INK))
                                .truncate()
                                .child(plugin.manifest().name().to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(colors::INK_4))
                                .truncate()
                                .child(plugin.manifest().author().to_string()),
                        ),
                )
                .child(
                    div()
                        .px(px(9.0))
                        .py(px(3.0))
                        .rounded(px(6.0))
                        .bg(rgba(STATUS_BG))
                        .text_size(px(11.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(status_color))
                        .child(status),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(colors::INK_3))
                .max_h(px(48.0))
                .overflow_hidden()
                .child(plugin.manifest().description().to_string()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .pt(px(8.0))
                .border_t_1()
                .border_color(rgba(colors::DIVIDER))
                .text_size(px(10.5))
                .text_color(rgb(colors::INK_4))
                .child(format!(
                    "{} permissions",
                    plugin.manifest().permissions().len()
                ))
                .child("·")
                .child(format!("{high_risk} high risk"))
                .child(
                    div()
                        .ml_auto()
                        .flex()
                        .items_center()
                        .gap(px(3.0))
                        .text_color(rgb(colors::SUCCESS))
                        .child("Sandboxed"),
                ),
        )
        .into_any_element()
}

fn high_risk_plugin_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot
        .installed_plugins
        .iter()
        .filter(|plugin| plugin.manifest().high_risk_permissions().next().is_some())
        .count()
}

fn sandbox_chip_label(snapshot: &BrowserSnapshot) -> &'static str {
    if snapshot
        .installed_plugins
        .iter()
        .all(|plugin| plugin.manifest().high_risk_permissions().next().is_none())
    {
        "Sandboxed"
    } else {
        "Audit needed"
    }
}

const SEARCH_BG: u32 = 0xffffffd9;
const SEARCH_BG_HOVER: u32 = 0xffffffeb;
const CARD_BG: u32 = 0xffffffc7;
const CARD_BG_HOVER: u32 = 0xffffffeb;
const STATUS_BG: u32 = 0xffffffd9;
