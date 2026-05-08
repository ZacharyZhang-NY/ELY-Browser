use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use ely_domain::{PluginId, PluginManifest, PluginPermission, PluginPermissionRisk};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_plugin_detail_page(
        &mut self,
        snapshot: &BrowserSnapshot,
        route: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let plugin_id = parse_plugin_detail_route(route);
        let plugin =
            plugin_id.as_ref().and_then(|plugin_id| find_installed_plugin(snapshot, plugin_id));

        render_canvas_surface(
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_5()
                .when_some(plugin, |this, plugin| {
                    this.child(self.render_plugin_detail_header(plugin, cx))
                        .child(render_plugin_security_summary(plugin))
                        .child(render_plugin_manifest_rows(plugin.manifest()))
                        .child(render_plugin_permission_list(plugin.manifest()))
                })
                .when(plugin.is_none(), |this| {
                    this.child(render_missing_plugin_detail(plugin_id.as_ref(), cx))
                }),
        )
    }

    fn render_plugin_detail_header(
        &mut self,
        plugin: &InstalledPlugin,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let plugin_id = plugin.id().clone();
        let target_enabled = !plugin.enabled();
        let status_label = if plugin.enabled() { "Enabled" } else { "Disabled" };
        let status_color = if plugin.enabled() { colors::SUCCESS } else { colors::MUTED };
        let action_label = if plugin.enabled() { "Disable" } else { "Enable" };
        let action_icon = if plugin.enabled() { IconName::CircleX } else { IconName::Check };

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
                    .child(
                        div()
                            .text_size(px(26.0))
                            .truncate()
                            .text_color(rgb(colors::INK))
                            .child(plugin.manifest().name().to_string()),
                    )
                    .child(div().text_sm().truncate().text_color(rgb(colors::MUTED)).child(
                        format!("{} - {}", plugin.manifest().author(), plugin.id().as_str()),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(div().font_semibold().text_color(rgb(status_color)).child(status_label))
                    .child(
                        Button::new("toggle-plugin-detail-enabled")
                            .xsmall()
                            .icon(action_icon)
                            .label(action_label)
                            .tooltip("Set Plugin State")
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.set_plugin_enabled(plugin_id.clone(), target_enabled, cx);
                            })),
                    )
                    .child(
                        Button::new("open-plugin-settings")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Info)
                            .label("Settings")
                            .tooltip("Open Plugin Settings")
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.open_internal_tab("ely://settings/plugins", window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn render_plugin_security_summary(plugin: &InstalledPlugin) -> AnyElement {
    let manifest = plugin.manifest();
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
            detail_metric("Signature", manifest.signature().algorithm().as_str()),
            detail_metric("Key", manifest.signature().key_id()),
            detail_metric("High Risk", manifest.high_risk_permissions().count().to_string()),
            detail_metric("Sync", sync_participation_label(manifest)),
        ])
        .into_any_element()
}

fn detail_metric(label: &'static str, value: impl Into<String>) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(label))
        .child(
            div()
                .text_sm()
                .font_semibold()
                .truncate()
                .text_color(rgb(colors::INK))
                .child(value.into()),
        )
        .into_any_element()
}

fn render_plugin_manifest_rows(manifest: &PluginManifest) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .child(plugin_detail_row(
            IconName::Info,
            "Description",
            manifest.description(),
            "Manifest summary",
        ))
        .child(plugin_detail_row(IconName::User, "Author", manifest.author(), "Publisher"))
        .child(plugin_detail_row(IconName::Globe, "Homepage", manifest.homepage(), "Publisher URL"))
        .child(plugin_detail_row(
            IconName::CircleCheck,
            "Minimum Build",
            manifest.min_ely_build().to_string(),
            "Required ELY version",
        ))
        .child(plugin_detail_row(
            IconName::BookOpen,
            "Checksum",
            manifest.checksum(),
            "Wasm package checksum",
        ))
        .into_any_element()
}

fn plugin_detail_row(
    icon: IconName,
    label: &'static str,
    value: impl Into<String>,
    detail: &'static str,
) -> AnyElement {
    let value = value.into();

    div()
        .py_3()
        .border_t_1()
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
                        .child(div().text_xs().text_color(rgb(colors::MUTED)).child(detail)),
                ),
        )
        .child(
            div()
                .max_w(px(380.0))
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child(value),
        )
        .into_any_element()
}

fn render_plugin_permission_list(manifest: &PluginManifest) -> AnyElement {
    if manifest.permissions().is_empty() {
        return div()
            .text_sm()
            .text_color(rgb(colors::MUTED))
            .child("This plugin declares no permissions.")
            .into_any_element();
    }

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child("Permissions"))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .overflow_y_scrollbar()
                .children(manifest.permissions().iter().map(render_permission_row)),
        )
        .into_any_element()
}

fn render_permission_row(permission: &PluginPermission) -> AnyElement {
    let risk = permission.risk();
    let risk_color = match risk {
        PluginPermissionRisk::Standard => colors::MUTED,
        PluginPermissionRisk::High => colors::ERROR,
    };

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
                .child(div().text_color(rgb(risk_color)).child(permission_icon(permission)))
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
                                .child(permission.as_str()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(colors::MUTED))
                                .child(permission_scope_label(permission)),
                        ),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(rgb(risk_color))
                .child(permission_risk_label(risk)),
        )
        .into_any_element()
}

fn render_missing_plugin_detail(
    plugin_id: Option<&PluginId>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let detail = plugin_id
        .map(|plugin_id| plugin_id.as_str().to_string())
        .unwrap_or_else(|| "Invalid plugin route".to_string());

    div()
        .size_full()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("Plugin Details"),
                )
                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(detail)),
        )
        .child(
            div()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt_5()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors::MUTED))
                        .child("Plugin details are available for installed plugins."),
                )
                .child(
                    Button::new("open-plugin-marketplace")
                        .primary()
                        .small()
                        .icon(IconName::ExternalLink)
                        .label("Open Plugins")
                        .tooltip("Open Plugin Marketplace")
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.open_internal_tab("ely://plugins", window, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn parse_plugin_detail_route(route: &str) -> Option<PluginId> {
    let plugin_id = route.strip_prefix("ely://plugin/")?;
    PluginId::parse(plugin_id).ok()
}

fn find_installed_plugin<'a>(
    snapshot: &'a BrowserSnapshot,
    plugin_id: &PluginId,
) -> Option<&'a InstalledPlugin> {
    snapshot.installed_plugins.iter().find(|plugin| plugin.id() == plugin_id)
}

fn sync_participation_label(manifest: &PluginManifest) -> &'static str {
    if manifest
        .permissions()
        .iter()
        .any(|permission| matches!(permission, PluginPermission::SyncPlugin))
    {
        "Participates"
    } else {
        "Local"
    }
}

fn permission_risk_label(risk: PluginPermissionRisk) -> &'static str {
    match risk {
        PluginPermissionRisk::Standard => "Standard",
        PluginPermissionRisk::High => "High",
    }
}

fn permission_icon(permission: &PluginPermission) -> IconName {
    if permission.requires_separate_confirmation() {
        IconName::TriangleAlert
    } else {
        IconName::CircleCheck
    }
}

fn permission_scope_label(permission: &PluginPermission) -> &'static str {
    match permission {
        PluginPermission::TabsRead => "Reads tab metadata.",
        PluginPermission::TabsWrite => "Creates, moves, or closes tabs.",
        PluginPermission::SpacesRead => "Reads Space metadata.",
        PluginPermission::SpacesWrite => "Creates or changes Spaces.",
        PluginPermission::BookmarksRead => "Reads bookmarks.",
        PluginPermission::BookmarksWrite => "Writes bookmarks.",
        PluginPermission::HistoryRead => "Reads browsing history.",
        PluginPermission::DownloadsRead => "Reads download entries.",
        PluginPermission::DownloadsWrite => "Controls downloads.",
        PluginPermission::PageMetadata => "Reads active page metadata.",
        PluginPermission::PageScreenshot => "Captures page screenshots.",
        PluginPermission::PageScript => "Runs scoped page scripts.",
        PluginPermission::ClipboardRead => "Reads clipboard content.",
        PluginPermission::ClipboardWrite => "Writes clipboard content.",
        PluginPermission::FilesystemRead => "Reads user-selected files.",
        PluginPermission::FilesystemWrite => "Writes user-selected files.",
        PluginPermission::NetworkFetch => "Performs plugin network requests.",
        PluginPermission::SettingsRead => "Reads plugin settings.",
        PluginPermission::SettingsWrite => "Writes plugin settings.",
        PluginPermission::SyncPlugin => "Syncs plugin configuration.",
        PluginPermission::UiPanel => "Registers sidebar panels.",
        PluginPermission::UiCommand => "Registers command bar actions.",
        PluginPermission::UiContextMenu => "Registers context menu actions.",
    }
}
