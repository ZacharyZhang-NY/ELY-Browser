use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use ely_domain::PluginId;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};
use crate::shell::chrome::{SERIF_FAMILY, render_plugin_detail_view};

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

        match plugin {
            Some(plugin) => render_canvas_surface(
                div().size_full().overflow_y_scrollbar().child(
                    render_plugin_detail_view(plugin, &snapshot.active_profile_kind, cx),
                ),
            ),
            None => render_canvas_surface(render_missing_plugin_detail(plugin_id.as_ref(), cx)),
        }
    }
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
        .pt(px(40.0))
        .px(px(56.0))
        .pb(px(40.0))
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .font_family(SERIF_FAMILY)
                        .text_size(px(28.0))
                        .text_color(rgb(colors::INK))
                        .child("Plugin Details"),
                )
                .child(div().text_sm().text_color(rgb(colors::MUTED)).child(detail)),
        )
        .child(
            div()
                .border_t_1()
                .border_color(rgb(colors::HAIRLINE))
                .pt(px(20.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.0))
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
