use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, App, Entity, ImageSource, InteractiveElement, IntoElement, MouseButton, ObjectFit,
    ParentElement, Styled, StyledImage, Window, canvas, div, img, px, rgb,
};

use super::{ElyShell, web_surface_frame::WebSurfaceFrame};
use ely_design_system::colors;

pub(super) fn render_ready_web_surface(
    frame: &WebSurfaceFrame,
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        img(ImageSource::Render(frame.image.clone())).size_full().object_fit(ObjectFit::Fill),
    )
}

pub(super) fn render_loading_web_surface(
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(tab, state_entity, div().size_full())
}

pub(super) fn render_failed_web_surface(
    tab: &BrowserTab,
    message: &str,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(tab, state_entity, error_page(message))
}

fn error_page(message: &str) -> impl IntoElement {
    div()
        .size_full()
        .p(px(48.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(22.0))
                .font_weight(gpui::FontWeight(400.0))
                .text_color(rgb(colors::INK))
                .child("Page unavailable"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgb(colors::INK_3))
                .child(message.to_string()),
        )
}

fn render_web_surface(
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
    content: impl IntoElement,
) -> AnyElement {
    let input_tab_id = tab.id().clone();
    let input_url = tab.url().as_str().to_string();
    let input_entity = state_entity.clone();
    let tracker_entity = state_entity;

    div()
        .flex_1()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .child(
            div()
                .relative()
                .size_full()
                .overflow_hidden()
                .child(content)
                .child(render_viewport_tracker(tab.id().clone(), tracker_entity))
                .child(render_input_overlay(input_tab_id, input_url, input_entity)),
        )
        .into_any_element()
}

fn render_input_overlay(
    tab_id: TabId,
    url: String,
    state_entity: Entity<ElyShell>,
) -> impl IntoElement {
    let down_entity = state_entity.clone();
    let click_tab_id = tab_id.clone();
    let click_url = url.clone();
    let click_entity = state_entity.clone();
    let hover_tab_id = tab_id.clone();
    let hover_entity = state_entity.clone();
    let scroll_tab_id = tab_id;
    let scroll_url = url;
    let scroll_entity = state_entity;

    div()
        .absolute()
        .size_full()
        .occlude()
        // Mouse-down hands focus to the shell's root focus handle so
        // subsequent keystrokes route to the web surface instead of
        // the omnibar Input. Doing this on mouse-down (not mouse-up)
        // lets the user start typing as soon as they press, matching
        // native browser focus semantics.
        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            down_entity.update(cx, |shell, _cx| {
                shell.focus_web_surface(window);
            });
        })
        .capture_any_mouse_up(move |event, window, cx| {
            if event.button != MouseButton::Left {
                return;
            }
            click_entity.update(cx, |shell, cx| {
                shell.click_external_web_viewport(
                    click_tab_id.clone(),
                    click_url.clone(),
                    event.position,
                    window,
                    cx,
                );
            });
            cx.stop_propagation();
        })
        .on_mouse_move(move |event, window, cx| {
            let scale_factor = window.scale_factor();
            hover_entity.update(cx, |shell, cx| {
                shell.hover_external_web_viewport(
                    hover_tab_id.clone(),
                    event.position,
                    scale_factor,
                    cx,
                );
            });
        })
        .on_scroll_wheel(move |event, window, cx| {
            let delta = event.delta.pixel_delta(window.line_height());
            let scale_factor = window.scale_factor();
            scroll_entity.update(cx, |shell, cx| {
                shell.scroll_external_web_viewport(
                    scroll_tab_id.clone(),
                    scroll_url.clone(),
                    delta,
                    scale_factor,
                    cx,
                );
            });
            cx.stop_propagation();
        })
}

fn render_viewport_tracker(tab_id: TabId, state_entity: Entity<ElyShell>) -> impl IntoElement {
    canvas(
        move |bounds, window: &mut Window, cx: &mut App| {
            let scale_factor = window.scale_factor();
            state_entity.update(cx, |shell, cx| {
                shell.record_external_web_viewport(tab_id, bounds, scale_factor, cx);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
