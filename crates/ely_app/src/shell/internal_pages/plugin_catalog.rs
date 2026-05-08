use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_plugin_catalog_page(
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
                .child(render_plugin_catalog_header(snapshot, cx))
                .child(render_plugin_catalog_summary(snapshot))
                .child(render_plugin_catalog_list(snapshot, cx)),
        )
    }
}

fn render_plugin_catalog_header(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
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
                        .text_color(rgb(colors::INK))
                        .child("Plugin Marketplace"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child("Signed local .rplug packages"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} installed", snapshot.installed_plugins.len()))
                .child(
                    Button::new("plugin-market-install")
                        .primary()
                        .small()
                        .icon(IconName::Plus)
                        .label("Install")
                        .tooltip("Install Plugin from File")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.choose_plugin_package(window, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_plugin_catalog_summary(snapshot: &BrowserSnapshot) -> AnyElement {
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
            plugin_metric("Installed", snapshot.installed_plugins.len()),
            plugin_metric("Enabled", enabled_plugin_count(snapshot)),
            plugin_metric("High Risk", high_risk_plugin_count(snapshot)),
            plugin_metric("Audit Events", snapshot.plugin_audit_events.len()),
        ])
        .into_any_element()
}

fn plugin_metric(label: &'static str, value: usize) -> AnyElement {
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

fn render_plugin_catalog_list(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if snapshot.installed_plugins.is_empty() {
        return div()
            .flex_1()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .pt_5()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(rgb(colors::INK))
                            .child("No plugins installed."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors::MUTED))
                            .child("Install a signed .rplug package from local disk."),
                    ),
            )
            .child(
                Button::new("plugin-market-empty-install")
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label("Install")
                    .tooltip("Install Plugin from File")
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.choose_plugin_package(window, cx);
                    })),
            )
            .into_any_element();
    }

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(
            snapshot
                .installed_plugins
                .iter()
                .enumerate()
                .map(|(index, plugin)| render_plugin_catalog_row(index, plugin, cx)),
        )
        .into_any_element()
}

fn render_plugin_catalog_row(
    index: usize,
    plugin: &InstalledPlugin,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let detail_route = format!("ely://plugin/{}", plugin.id().as_str());
    let high_risk_count = plugin.manifest().high_risk_permissions().count();
    let status_color = if plugin.enabled() { colors::SUCCESS } else { colors::MUTED };
    let status_label = if plugin.enabled() { "Enabled" } else { "Disabled" };

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
                .child(div().text_color(rgb(colors::MUTED_SOFT)).child(IconName::Asterisk))
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
                                .child(plugin.manifest().name().to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(plugin.manifest().description().to_string()),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_3()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} permissions", plugin.manifest().permissions().len()))
                .child(format!("{high_risk_count} high risk"))
                .child(div().text_color(rgb(status_color)).child(status_label))
                .child(
                    Button::new(("plugin-market-details", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::ExternalLink)
                        .label("Details")
                        .tooltip("Open Plugin Details")
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.open_internal_tab(&detail_route, window, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn enabled_plugin_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot.installed_plugins.iter().filter(|plugin| plugin.enabled()).count()
}

fn high_risk_plugin_count(snapshot: &BrowserSnapshot) -> usize {
    snapshot
        .installed_plugins
        .iter()
        .filter(|plugin| plugin.manifest().high_risk_permissions().next().is_some())
        .count()
}
