use ely_browser_core::{BrowserSnapshot, InstalledPlugin};
use ely_design_system::colors;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, px, rgb,
};

use super::ElyShell;

pub(super) fn render_editors_pick(
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let featured = featured_plugin(snapshot);

    div()
        .p(px(20.0))
        .rounded(px(16.0))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(0.0, 0.0, 1.0, 0.94), 0.0),
            linear_color_stop(hsla(263.0 / 360.0, 0.6, 0.86, 0.65), 1.0),
        ))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_3))
                .child("EDITOR'S PICK"),
        )
        .child(render_featured_body(featured))
        .child(render_featured_actions(featured, cx))
        .into_any_element()
}

fn render_featured_body(featured: Option<&InstalledPlugin>) -> AnyElement {
    let (name, desc) = match featured {
        Some(plugin) => (
            plugin.manifest().name().to_string(),
            plugin.manifest().description().to_string(),
        ),
        None => (
            "Install your first plugin".to_string(),
            "Drop a signed .rplug package onto ELY to extend the browser with sandboxed tools."
                .to_string(),
        ),
    };

    div()
        .flex()
        .gap(px(14.0))
        .child(
            div()
                .size(px(56.0))
                .rounded(px(14.0))
                .bg(linear_gradient(
                    135.0,
                    linear_color_stop(hsla(341.0 / 360.0, 0.78, 0.85, 1.0), 0.0),
                    linear_color_stop(hsla(228.0 / 360.0, 1.0, 0.86, 1.0), 1.0),
                ))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(28.0))
                .text_color(rgb(0xffffff))
                .child(name.chars().next().unwrap_or('✦').to_uppercase().to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight(600.0))
                        .text_color(rgb(colors::INK))
                        .child(name),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(rgb(colors::INK_3))
                        .child(desc),
                ),
        )
        .into_any_element()
}

fn render_featured_actions(
    featured: Option<&InstalledPlugin>,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    if let Some(plugin) = featured {
        let detail_route = format!("ely://plugin/{}", plugin.id().as_str());

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(action_button(
                "featured-open",
                "Open detail",
                cx,
                move |shell, window, cx| {
                    shell.open_internal_tab(&detail_route, window, cx);
                },
            ))
            .child(
                div()
                    .ml_auto()
                    .text_size(px(11.5))
                    .text_color(rgb(colors::INK_3))
                    .child(format!(
                        "{} permissions",
                        plugin.manifest().permissions().len()
                    )),
            )
            .into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(action_button(
                "featured-install",
                "Install",
                cx,
                |shell, window, cx| shell.choose_plugin_package(window, cx),
            ))
            .into_any_element()
    }
}

pub(super) fn action_button<F>(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<ElyShell>,
    handler: F,
) -> AnyElement
where
    F: Fn(&mut ElyShell, &mut gpui::Window, &mut Context<ElyShell>) + 'static,
{
    div()
        .id(SharedString::from(id))
        .px(px(14.0))
        .py(px(7.0))
        .rounded(px(8.0))
        .bg(rgb(colors::INK))
        .text_size(px(12.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .hover(|style| style.opacity(0.92))
        .active(|style| style.opacity(0.78))
        .on_click(cx.listener(move |shell, _, window, cx| handler(shell, window, cx)))
        .child(label)
        .into_any_element()
}

pub(super) fn featured_plugin(snapshot: &BrowserSnapshot) -> Option<&InstalledPlugin> {
    snapshot
        .installed_plugins
        .iter()
        .find(|plugin| plugin.enabled_for_profile(&snapshot.active_profile_kind))
        .or_else(|| snapshot.installed_plugins.first())
}
