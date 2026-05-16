use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use ely_domain::ProfileKind;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, px, rgb,
    rgba,
};
use gpui_component::{IconName, input::Input, scroll::ScrollableElement};

use super::plugin_editors_pick::{action_button, render_editors_pick};
use super::{ElyShell, render_canvas_surface};
use crate::shell::chrome::SERIF_FAMILY;

impl ElyShell {
    pub(super) fn render_plugin_catalog_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let needle = self.plugin_search_input.read(cx).value().to_string();
        let needle_lower = needle.trim().to_lowercase();

        render_canvas_surface(
            div()
                .size_full()
                .pt(px(40.0))
                .px(px(56.0))
                .pb(px(40.0))
                .flex()
                .flex_col()
                .gap(px(24.0))
                .child(self.render_hero(snapshot, cx))
                .child(render_summary(snapshot))
                .child(render_grid(snapshot, &needle_lower, cx)),
        )
    }

    fn render_hero(&mut self, snapshot: &BrowserSnapshot, cx: &mut Context<Self>) -> AnyElement {
        div()
            .grid()
            .grid_cols(2)
            .gap(px(28.0))
            .child(self.render_hero_left(snapshot, cx))
            .child(render_editors_pick(snapshot, cx))
            .into_any_element()
    }

    fn render_hero_left(
        &mut self,
        _snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(colors::ink_4()))
                    .child("PLUGINS · NATIVE TO ELY"),
            )
            .child(
                div()
                    .font_family(SERIF_FAMILY)
                    .text_size(px(48.0))
                    .font_weight(FontWeight(400.0))
                    .text_color(rgb(colors::ink()))
                    .child("Quiet tools. Everyday magic."),
            )
            .child(
                div().max_w(px(520.0)).text_size(px(13.5)).text_color(rgb(colors::ink_2())).child(
                    "ELY plugins are sandboxed, theme-aware, and ship with their own \
                         controls in your sidebar — no Chrome extension framework required.",
                ),
            )
            .child(self.render_search_row(cx))
            .into_any_element()
    }

    fn render_search_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .max_w(px(520.0))
            .child(
                div()
                    .flex_1()
                    .h(px(42.0))
                    .px(px(16.0))
                    .rounded(px(12.0))
                    .bg(rgba(search_bg()))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(div().text_color(rgb(colors::ink_3())).child(IconName::Search))
                    .child(div().flex_1().child(
                        Input::new(&self.plugin_search_input).appearance(false).cleanable(true),
                    )),
            )
            .child(
                div()
                    .id(SharedString::from("plugin-install-action"))
                    .h(px(42.0))
                    .px(px(16.0))
                    .rounded(px(12.0))
                    .bg(rgba(INSTALL_BG))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.92))
                    .active(|style| style.opacity(0.78))
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.choose_plugin_package(window, cx);
                    }))
                    .child(div().text_color(rgb(0xffffff)).child(IconName::Plus))
                    .child("Install"),
            )
            .into_any_element()
    }
}

fn render_summary(snapshot: &BrowserSnapshot) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(category_chip("All", true))
        .child(category_chip("Installed", false))
        .child(category_chip(sandbox_chip_label(snapshot), false))
        .child(div().ml_auto().text_size(px(11.5)).text_color(rgb(colors::ink_3())).child(format!(
            "{} signed · {} high-risk",
            snapshot.installed_plugins.len(),
            high_risk_plugin_count(snapshot)
        )))
        .into_any_element()
}

fn category_chip(label: &'static str, active: bool) -> AnyElement {
    let bg = if active { 0x1d1c1aff } else { 0xffffffb3 };
    let text_color = if active { 0xffffff } else { colors::ink_2() };

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

fn render_grid(snapshot: &BrowserSnapshot, needle: &str, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.installed_plugins.is_empty() {
        return render_empty_state(cx);
    }

    let matches: Vec<&InstalledPlugin> =
        snapshot.installed_plugins.iter().filter(|plugin| matches_plugin(plugin, needle)).collect();

    if matches.is_empty() {
        return render_no_match_state(needle);
    }

    div()
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .grid()
        .grid_cols(4)
        .gap(px(14.0))
        .children(matches.into_iter().enumerate().map(|(index, plugin)| {
            render_plugin_card(index, plugin, &snapshot.active_profile_kind, cx)
        }))
        .into_any_element()
}

fn matches_plugin(plugin: &InstalledPlugin, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let manifest = plugin.manifest();
    manifest.name().to_lowercase().contains(needle)
        || manifest.author().to_lowercase().contains(needle)
        || manifest.description().to_lowercase().contains(needle)
}

fn render_no_match_state(needle: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .py(px(40.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(colors::ink()))
                .child(format!("No plugins match \"{needle}\".")),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(colors::ink_3()))
                .child("Try a different keyword."),
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
                .text_color(rgb(colors::ink()))
                .child("No plugins yet."),
        )
        .child(div().max_w(px(420.0)).text_size(px(13.0)).text_color(rgb(colors::ink_3())).child(
            "Drop a signed .rplug package on ELY to extend the browser with sandboxed \
                     tools. Plugins ship with their own sidebar controls.",
        ))
        .child(action_button("empty-install", "Install plugin", cx, |shell, window, cx| {
            shell.choose_plugin_package(window, cx)
        }))
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
    let status_color = if enabled { colors::success() } else { colors::ink_4() };
    let high_risk = plugin.manifest().high_risk_permissions().count();
    let glyph = plugin.manifest().name().chars().next().unwrap_or('◇').to_string();
    let initial = glyph.to_uppercase().to_string();
    let (cover_a, cover_b) = plugin_cover_gradient(plugin.manifest().name());

    div()
        .id(SharedString::from(format!("plugin-card-{index}")))
        .p(px(14.0))
        .rounded(px(16.0))
        .bg(rgba(card_bg()))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(card_bg_hover())))
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
                    linear_color_stop(cover_a, 0.0),
                    linear_color_stop(cover_b, 1.0),
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
                                .text_color(rgb(colors::ink()))
                                .truncate()
                                .child(plugin.manifest().name().to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(colors::ink_4()))
                                .truncate()
                                .child(plugin.manifest().author().to_string()),
                        ),
                )
                .child(
                    div()
                        .px(px(9.0))
                        .py(px(3.0))
                        .rounded(px(6.0))
                        .bg(rgba(status_bg()))
                        .text_size(px(11.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(status_color))
                        .child(status),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(colors::ink_3()))
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
                .border_color(rgba(colors::divider()))
                .text_size(px(10.5))
                .text_color(rgb(colors::ink_4()))
                .child(format!("{} permissions", plugin.manifest().permissions().len()))
                .child("·")
                .child(format!("{high_risk} high risk"))
                .child(
                    div()
                        .ml_auto()
                        .flex()
                        .items_center()
                        .gap(px(3.0))
                        .text_color(rgb(colors::success()))
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

fn search_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
const INSTALL_BG: u32 = 0x1d1c1aff;
fn card_bg() -> u32 {
    colors::pick(0xffffffc7, 0x1f1d1bc7)
}
fn card_bg_hover() -> u32 {
    colors::pick(0xffffffeb, 0x1f1d1beb)
}
fn status_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}

/// Pick a deterministic gradient pair for a plugin cover. Mirrors the
/// design's palette of 8 pastel-to-bold ramps used across `plugins.jsx`,
/// hashed by plugin name so the same plugin always lands on the same
/// ramp without needing any new manifest fields.
fn plugin_cover_gradient(name: &str) -> (gpui::Hsla, gpui::Hsla) {
    const PALETTE: &[(f32, f32, f32, f32, f32, f32)] = &[
        (341.0, 0.78, 0.85, 228.0, 1.00, 0.86),
        (148.0, 0.65, 0.85, 252.0, 0.85, 0.71),
        (251.0, 0.85, 0.71, 341.0, 0.78, 0.85),
        (20.0, 1.00, 0.95, 17.0, 0.71, 0.84),
        (263.0, 1.00, 0.88, 341.0, 0.78, 0.85),
        (33.0, 1.00, 0.86, 343.0, 0.78, 0.67),
        (162.0, 0.43, 0.51, 30.0, 0.04, 0.04),
        (343.0, 0.78, 0.67, 251.0, 0.85, 0.71),
    ];

    let bucket =
        name.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32)) as usize % PALETTE.len();
    let (h1, s1, l1, h2, s2, l2) = PALETTE[bucket];
    (hsla(h1 / 360.0, s1, l1, 1.0), hsla(h2 / 360.0, s2, l2, 1.0))
}
