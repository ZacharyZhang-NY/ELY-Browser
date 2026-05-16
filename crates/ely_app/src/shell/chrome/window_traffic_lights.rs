use ely_design_system::spacing;
use gpui::{AnyElement, Context, IntoElement, div};
#[cfg(target_os = "macos")]
use gpui::{
    App, InteractiveElement, MouseButton, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window, px,
};

use crate::shell::ElyShell;

// Measured from the window's top-left to the close button origin.
// Places the macOS traffic lights inside the calm part of the corner curve.
pub(crate) const TRAFFIC_LIGHT_ORIGIN_X: f32 = spacing::SHELL_INSET + 34.0;
pub(crate) const TRAFFIC_LIGHT_ORIGIN_Y: f32 = spacing::SHELL_INSET + 22.0;

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_HITBOX_SIZE: f32 = 18.0;
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_HITBOX_OFFSET: f32 = -2.0;
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_BUTTON_SPACING: f32 = 20.0;

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum TrafficLightAction {
    Close,
    Minimize,
    Zoom,
}

#[cfg(target_os = "macos")]
pub(crate) fn render_macos_traffic_light_hitboxes(_cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .child(render_traffic_light_hitbox("macos-close-hitbox", 0.0, TrafficLightAction::Close))
        .child(render_traffic_light_hitbox(
            "macos-minimize-hitbox",
            1.0,
            TrafficLightAction::Minimize,
        ))
        .child(render_traffic_light_hitbox("macos-zoom-hitbox", 2.0, TrafficLightAction::Zoom))
        .into_any_element()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn render_macos_traffic_light_hitboxes(_cx: &mut Context<ElyShell>) -> AnyElement {
    div().into_any_element()
}

#[cfg(target_os = "macos")]
fn render_traffic_light_hitbox(
    id: &'static str,
    index: f32,
    action: TrafficLightAction,
) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .absolute()
        .left(px(TRAFFIC_LIGHT_ORIGIN_X
            + index * TRAFFIC_LIGHT_BUTTON_SPACING
            + TRAFFIC_LIGHT_HITBOX_OFFSET))
        .top(px(TRAFFIC_LIGHT_ORIGIN_Y + TRAFFIC_LIGHT_HITBOX_OFFSET))
        .size(px(TRAFFIC_LIGHT_HITBOX_SIZE))
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| {
            run_traffic_light_action(action, window, cx);
            cx.stop_propagation();
        })
        .into_any_element()
}

#[cfg(target_os = "macos")]
fn run_traffic_light_action(action: TrafficLightAction, window: &mut Window, _cx: &mut App) {
    match action {
        TrafficLightAction::Close => window.remove_window(),
        TrafficLightAction::Minimize => window.minimize_window(),
        TrafficLightAction::Zoom => window.zoom_window(),
    }
}
