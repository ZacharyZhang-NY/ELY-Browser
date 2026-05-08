use ely_domain::{BrowserTab, TabId};
use gpui::{
    AnyElement, App, Entity, ImageSource, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Styled, StyledImage, Window, canvas, div, img, prelude::FluentBuilder, px, rgb,
};
use gpui_component::StyledExt;

use super::{ElyShell, web_surface_frame::WebSurfaceFrame};
use ely_design_system::{colors, spacing};

pub(super) fn render_ready_web_surface(
    frame: &WebSurfaceFrame,
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        frame.title_label(),
        frame.url_label().to_string(),
        Some(frame.detail_label()),
        img(ImageSource::Render(frame.image.clone())).size_full().object_fit(ObjectFit::Contain),
    )
}

pub(super) fn render_loading_web_surface(
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        tab.title().to_string(),
        tab.url().as_str().to_string(),
        Some("Rendering".to_string()),
        centered_status(tab.title(), tab.url().as_str(), "Rendering page with Servo", colors::BODY),
    )
}

pub(super) fn render_failed_web_surface(
    tab: &BrowserTab,
    message: &str,
    state_entity: Entity<ElyShell>,
) -> AnyElement {
    render_web_surface(
        tab,
        state_entity,
        tab.title().to_string(),
        tab.url().as_str().to_string(),
        Some("Render failed".to_string()),
        centered_status(tab.title(), tab.url().as_str(), message, colors::ERROR),
    )
}

fn render_web_surface_header(title: String, url: String, detail: Option<String>) -> AnyElement {
    div()
        .h(px(34.0))
        .px_3()
        .gap_3()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_sm()
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child(title),
        )
        .when_some(detail, |this, detail| {
            this.child(div().text_xs().text_color(rgb(colors::MUTED)).child(detail))
        })
        .child(
            div().max_w(px(420.0)).truncate().text_xs().text_color(rgb(colors::MUTED)).child(url),
        )
        .into_any_element()
}

fn centered_status(title: &str, url: &str, detail: &str, detail_color: u32) -> impl IntoElement {
    div()
        .size_full()
        .p_8()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(rgb(colors::SURFACE_CARD))
        .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child(title.to_string()))
        .child(div().text_sm().text_color(rgb(colors::MUTED)).child(url.to_string()))
        .child(div().text_sm().text_color(rgb(detail_color)).child(detail.to_string()))
}

fn render_web_surface(
    tab: &BrowserTab,
    state_entity: Entity<ElyShell>,
    title: String,
    url: String,
    detail: Option<String>,
    content: impl IntoElement,
) -> AnyElement {
    let scroll_tab_id = tab.id().clone();
    let scroll_url = tab.url().as_str().to_string();
    let scroll_entity = state_entity.clone();
    let tracker_entity = state_entity;

    div()
        .flex_1()
        .h_full()
        .p(px(spacing::SM))
        .bg(rgb(colors::CANVAS_SOFT))
        .child(
            div()
                .size_full()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(rgb(colors::HAIRLINE))
                .bg(rgb(colors::SURFACE_CARD))
                .flex()
                .flex_col()
                .child(render_web_surface_header(title, url, detail))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .bg(rgb(colors::SURFACE_CARD))
                        .on_scroll_wheel(move |event, window, cx| {
                            let delta = event.delta.pixel_delta(window.line_height());
                            scroll_entity.update(cx, |shell, cx| {
                                shell.scroll_external_web_viewport(
                                    scroll_tab_id.clone(),
                                    scroll_url.clone(),
                                    delta,
                                    cx,
                                );
                            });
                            cx.stop_propagation();
                        })
                        .child(content)
                        .child(render_viewport_tracker(tab.id().clone(), tracker_entity)),
                ),
        )
        .into_any_element()
}

fn render_viewport_tracker(tab_id: TabId, state_entity: Entity<ElyShell>) -> impl IntoElement {
    canvas(
        move |bounds, _window: &mut Window, cx: &mut App| {
            state_entity.update(cx, |shell, cx| {
                shell.record_external_web_viewport(tab_id, bounds, cx);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
