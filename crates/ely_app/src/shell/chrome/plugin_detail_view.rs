use ely_browser_core::InstalledPlugin;
use ely_design_system::colors;
use ely_domain::{
    PluginContributionPoint, PluginManifest, PluginPermission, PluginPermissionRisk, ProfileKind,
};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, px, rgb,
    rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;
use crate::shell::chrome::SERIF_FAMILY;
use crate::shell::chrome::plugin_labels::{
    contribution_detail, contribution_title, permission_scope_label,
};

pub(crate) fn render_plugin_detail_view(
    plugin: &InstalledPlugin,
    profile_kind: &ProfileKind,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .size_full()
        .pt(px(40.0))
        .px(px(56.0))
        .pb(px(40.0))
        .child(
            div()
                .max_w(px(960.0))
                .mx_auto()
                .p(px(24.0))
                .rounded(px(16.0))
                .bg(rgba(CARD_BG))
                .grid()
                .grid_cols(8)
                .gap(px(24.0))
                .child(div().col_span(3).child(render_left_column(plugin, profile_kind, cx)))
                .child(div().col_span(5).child(render_right_column(plugin))),
        )
        .into_any_element()
}

fn render_left_column(
    plugin: &InstalledPlugin,
    profile_kind: &ProfileKind,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_cover(plugin))
        .child(render_install_row(plugin, profile_kind, cx))
        .child(render_permissions_block(plugin.manifest()))
        .into_any_element()
}

fn render_cover(plugin: &InstalledPlugin) -> AnyElement {
    let initial = plugin.manifest().name().chars().next().unwrap_or('◇').to_string().to_uppercase();

    div()
        .h(px(220.0))
        .rounded(px(14.0))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(341.0 / 360.0, 0.78, 0.85, 1.0), 0.0),
            linear_color_stop(hsla(228.0 / 360.0, 1.0, 0.86, 1.0), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(80.0))
        .text_color(rgb(0xffffff))
        .child(initial)
        .into_any_element()
}

fn render_install_row(
    plugin: &InstalledPlugin,
    profile_kind: &ProfileKind,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let plugin_id = plugin.id().clone();
    let enabled = plugin.enabled_for_profile(profile_kind);
    let primary_label = if enabled { "Disable" } else { "Enable" };
    let target_enabled = !plugin.enabled();

    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(render_primary_button(primary_label, cx, move |shell, _, cx| {
            shell.set_plugin_enabled(plugin_id.clone(), target_enabled, cx);
        }))
        .child(render_secondary_button(IconName::Info, cx, |shell, window, cx| {
            shell.open_internal_tab("ely://settings/plugins", window, cx);
        }))
        .into_any_element()
}

fn render_primary_button<F>(
    label: &'static str,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from("plugin-detail-primary"))
        .flex_1()
        .h(px(40.0))
        .rounded(px(10.0))
        .bg(rgb(colors::ink()))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .hover(|style| style.opacity(0.92))
        .active(|style| style.opacity(0.78))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(div().text_color(rgb(0xffffff)).child(IconName::Check))
        .child(label)
        .into_any_element()
}

fn render_secondary_button<F>(icon: IconName, cx: &mut Context<ElyShell>, handler: F) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from("plugin-detail-secondary"))
        .size(px(40.0))
        .rounded(px(10.0))
        .bg(rgba(SECONDARY_BG))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::ink_3()))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(SECONDARY_BG_HOVER)).text_color(rgb(colors::ink())))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(icon)
        .into_any_element()
}

fn render_permissions_block(manifest: &PluginManifest) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .pt(px(4.0))
        .child(div().text_size(px(11.0)).text_color(rgb(colors::ink_3())).child("PERMISSIONS"))
        .child(if manifest.permissions().is_empty() {
            div()
                .text_size(px(12.0))
                .text_color(rgb(colors::ink_4()))
                .child("This plugin declares no permissions.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(manifest.permissions().iter().map(render_permission_row))
                .into_any_element()
        })
        .into_any_element()
}

fn render_permission_row(permission: &PluginPermission) -> AnyElement {
    let high_risk = matches!(permission.risk(), PluginPermissionRisk::High);
    let badge_bg = if high_risk { 0xfde7e7 } else { 0xe8f3ee };
    let badge_color = if high_risk { 0xb53737 } else { 0x2f7d68 };

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(12.0))
        .child(
            div()
                .size(px(14.0))
                .rounded(px(4.0))
                .bg(rgb(badge_bg))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(badge_color))
                .text_size(px(10.0))
                .child(if high_risk { "!" } else { "✓" }),
        )
        .child(
            div()
                .flex_1()
                .text_color(rgb(colors::ink_2()))
                .child(permission_scope_label(permission)),
        )
        .into_any_element()
}

fn render_right_column(plugin: &InstalledPlugin) -> AnyElement {
    let manifest = plugin.manifest();

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_right_header(plugin))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(rgb(colors::ink_2()))
                .child(manifest.description().to_string()),
        )
        .child(render_stats_grid(plugin))
        .child(render_contributions(manifest))
        .into_any_element()
}

fn render_right_header(plugin: &InstalledPlugin) -> AnyElement {
    let manifest = plugin.manifest();

    div()
        .flex()
        .items_start()
        .gap(px(14.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(colors::ink_4()))
                        .child(category_label(plugin)),
                )
                .child(
                    div()
                        .font_family(SERIF_FAMILY)
                        .text_size(px(32.0))
                        .font_weight(FontWeight(400.0))
                        .text_color(rgb(colors::ink()))
                        .child(manifest.name().to_string()),
                )
                .child(div().text_size(px(12.5)).text_color(rgb(colors::ink_3())).child(format!(
                    "by {} · min ELY {}",
                    manifest.author(),
                    manifest.min_ely_build()
                ))),
        )
        .child(render_status_chip(plugin))
        .into_any_element()
}

fn render_status_chip(plugin: &InstalledPlugin) -> AnyElement {
    let enabled = plugin.enabled();
    let label = if enabled { "Enabled" } else { "Disabled" };
    let bg = if enabled { 0xe8f3ee } else { 0x281e140d };
    let color = if enabled { 0x2f7d68 } else { colors::ink_3() };

    div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(rgba(bg))
        .text_size(px(11.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(color))
        .child(label)
        .into_any_element()
}

fn render_stats_grid(plugin: &InstalledPlugin) -> AnyElement {
    let manifest = plugin.manifest();
    let permissions = manifest.permissions().len();
    let high_risk = manifest.high_risk_permissions().count();
    let contributes = manifest.contributes().len();
    let signature = manifest.signature().algorithm().as_str();

    div()
        .grid()
        .grid_cols(4)
        .gap(px(8.0))
        .child(stat_card("Permissions", permissions.to_string()))
        .child(stat_card("High risk", high_risk.to_string()))
        .child(stat_card("Contributes", contributes.to_string()))
        .child(stat_card("Signature", signature.to_string()))
        .into_any_element()
}

fn stat_card(label: &'static str, value: String) -> AnyElement {
    div()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .bg(rgba(STAT_BG))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(div().text_size(px(10.5)).text_color(rgb(colors::ink_4())).child(label))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink()))
                .child(value),
        )
        .into_any_element()
}

fn render_contributions(manifest: &PluginManifest) -> AnyElement {
    let contributions = manifest.contributes();

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight(600.0))
                .text_color(rgb(colors::ink()))
                .child("What it adds to ELY"),
        )
        .child(if contributions.is_empty() {
            div()
                .text_size(px(12.0))
                .text_color(rgb(colors::ink_4()))
                .child("This plugin does not register any contributions.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(contributions.iter().map(render_contribution_row))
                .into_any_element()
        })
        .into_any_element()
}

fn render_contribution_row(contribution: &PluginContributionPoint) -> AnyElement {
    div()
        .flex()
        .items_start()
        .gap(px(10.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .bg(rgba(CONTRIBUTION_BG))
        .child(
            div()
                .size(px(18.0))
                .rounded(px(5.0))
                .bg(rgba(CONTRIBUTION_BADGE_BG))
                .flex()
                .items_center()
                .justify_center()
                .mt(px(1.0))
                .text_color(rgb(colors::accent()))
                .text_size(px(11.0))
                .child(IconName::Check),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child(contribution_title(contribution)),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgb(colors::ink_3()))
                        .child(contribution_detail(contribution)),
                ),
        )
        .into_any_element()
}

fn category_label(plugin: &InstalledPlugin) -> &'static str {
    let high_risk = plugin.manifest().high_risk_permissions().next().is_some();
    if high_risk { "ELY · AUDIT NEEDED" } else { "ELY · SANDBOXED" }
}

const CARD_BG: u32 = 0xffffffd9;
const SECONDARY_BG: u32 = 0xffffffd9;
const SECONDARY_BG_HOVER: u32 = 0xffffffeb;
const STAT_BG: u32 = 0xffffffb3;
const CONTRIBUTION_BG: u32 = 0xffffff8c;
const CONTRIBUTION_BADGE_BG: u32 = 0xc964421f;
