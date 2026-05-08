use ely_browser_core::{BrowserSnapshot, InstalledPlugin, PluginAuditAction, PluginAuditEvent};
use ely_design_system::colors;
use ely_domain::PluginId;
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::super::plugins::PendingPluginInstall;
use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_plugins_page(
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
                .child(
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(26.0))
                                        .text_color(rgb(colors::INK))
                                        .child("Plugins"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(colors::MUTED))
                                        .child("Browser scope"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap_1()
                                .child(div().text_xs().text_color(rgb(colors::MUTED)).child(
                                    format!("{} installed", snapshot.installed_plugins.len()),
                                ))
                                .child(div().text_xs().text_color(rgb(colors::MUTED)).child(
                                    format!("{} audit events", snapshot.plugin_audit_events.len()),
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child("Install signed .rplug packages from local disk."),
                        )
                        .child(
                            Button::new("install-plugin-package")
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
                .when_some(self.plugin_install_error.clone(), |this, message| {
                    this.child(render_plugin_install_error(message))
                })
                .when_some(self.pending_plugin_install.clone(), |this, pending| {
                    this.child(render_plugin_install_confirmation(&pending, cx))
                })
                .child(render_plugin_list(snapshot, cx))
                .child(render_plugin_audit_list(snapshot)),
        )
    }
}

fn render_plugin_install_error(message: String) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
        .px_3()
        .py_2()
        .text_xs()
        .text_color(rgb(colors::ERROR))
        .child(message)
        .into_any_element()
}

fn render_plugin_install_confirmation(
    pending: &PendingPluginInstall,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::ERROR))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(rgb(colors::ERROR))
                        .child(format!("Confirm install for {}", pending.manifest().name())),
                )
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(high_risk_permissions_label(pending)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("cancel-plugin-install").ghost().xsmall().label("Cancel").on_click(
                        cx.listener(|shell, _, _, cx| {
                            shell.cancel_plugin_install(cx);
                        }),
                    ),
                )
                .child(
                    Button::new("confirm-plugin-install")
                        .danger()
                        .xsmall()
                        .label("Confirm")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.confirm_plugin_install(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_plugin_list(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    if snapshot.installed_plugins.is_empty() {
        return div()
            .border_t_1()
            .border_color(rgb(colors::HAIRLINE))
            .pt_5()
            .text_sm()
            .text_color(rgb(colors::MUTED))
            .child("No plugins installed.")
            .into_any_element();
    }

    let rows = snapshot
        .installed_plugins
        .iter()
        .enumerate()
        .map(|(index, plugin)| render_plugin_row(index, plugin, cx))
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(rows)
        .into_any_element()
}

fn render_plugin_row(
    index: usize,
    plugin: &InstalledPlugin,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let high_risk_count = plugin.manifest().high_risk_permissions().count();
    let status_color = if plugin.enabled() { colors::SUCCESS } else { colors::MUTED };
    let status_label = if plugin.enabled() { "Enabled" } else { "Disabled" };
    let plugin_id = plugin.id().clone();
    let target_enabled = !plugin.enabled();

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
                        .child(plugin.id().as_str().to_string()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .text_color(rgb(colors::MUTED))
                .child(format!("{} permissions", plugin.manifest().permissions().len()))
                .child(format!("{high_risk_count} high risk"))
                .child(div().text_color(rgb(status_color)).child(status_label))
                .child(render_plugin_state_button(index, plugin, plugin_id, target_enabled, cx)),
        )
        .into_any_element()
}

fn render_plugin_state_button(
    index: usize,
    plugin: &InstalledPlugin,
    plugin_id: PluginId,
    target_enabled: bool,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let label = if plugin.enabled() { "Disable" } else { "Enable" };
    let tooltip = if plugin.enabled() { "Disable Plugin" } else { "Enable Plugin" };
    let icon = if plugin.enabled() { IconName::CircleX } else { IconName::Check };
    let button = Button::new(("set-plugin-enabled", index))
        .xsmall()
        .icon(icon)
        .label(label)
        .tooltip(tooltip)
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_plugin_enabled(plugin_id.clone(), target_enabled, cx);
        }));

    if plugin.enabled() {
        button.danger().into_any_element()
    } else {
        button.primary().into_any_element()
    }
}

fn render_plugin_audit_list(snapshot: &BrowserSnapshot) -> AnyElement {
    if snapshot.plugin_audit_events.is_empty() {
        return div().into_any_element();
    }

    div()
        .flex_1()
        .flex()
        .flex_col()
        .min_h_0()
        .gap_3()
        .child(div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child("Audit"))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scrollbar()
                .children(snapshot.plugin_audit_events.iter().rev().map(render_plugin_audit_row)),
        )
        .into_any_element()
}

fn render_plugin_audit_row(event: &PluginAuditEvent) -> AnyElement {
    div()
        .py_2()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_xs()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(colors::BODY))
                .child(event.plugin_id().as_str().to_string()),
        )
        .child(
            div().text_color(rgb(colors::MUTED)).child(plugin_audit_action_label(event.action())),
        )
        .into_any_element()
}

fn plugin_audit_action_label(action: &PluginAuditAction) -> &'static str {
    match action {
        PluginAuditAction::Installed => "Installed",
        PluginAuditAction::Enabled => "Enabled",
        PluginAuditAction::Disabled => "Disabled",
    }
}

fn high_risk_permissions_label(pending: &PendingPluginInstall) -> String {
    let permissions = pending
        .high_risk_permissions()
        .iter()
        .map(|permission| permission.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("High risk permissions: {permissions}")
}
