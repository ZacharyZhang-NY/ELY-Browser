use ely_design_system::colors;
use gpui::{
    AnyElement, IntoElement, ParentElement, Styled, div, px, rgb, rgba,
};
use gpui_component::IconName;

pub(crate) fn render_command_footer() -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(14.0))
        .px(px(16.0))
        .py(px(10.0))
        .border_t_1()
        .border_color(rgba(colors::DIVIDER))
        .text_size(px(10.5))
        .text_color(rgb(colors::INK_3))
        .bg(rgba(FOOTER_BG))
        .child(footer_chunk("↑↓", "navigate"))
        .child(footer_chunk("↵", "open"))
        .child(footer_chunk("⌘↵", "open in split"))
        .child(footer_chunk("⇥", "filter"))
        .child(
            div()
                .ml_auto()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_color(rgb(colors::ACCENT))
                        .child(IconName::Asterisk),
                )
                .child("Powered by ELY"),
        )
        .into_any_element()
}

fn footer_chunk(keys: &'static str, label: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(render_kbd(keys))
        .child(label)
        .into_any_element()
}

pub(crate) fn render_kbd(label: &'static str) -> AnyElement {
    div()
        .px(px(5.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .bg(rgba(KBD_BG))
        .text_size(px(10.0))
        .text_color(rgb(colors::INK_3))
        .child(label)
        .into_any_element()
}

const KBD_BG: u32 = 0xffffffd9;
const FOOTER_BG: u32 = 0xffffff8c;
