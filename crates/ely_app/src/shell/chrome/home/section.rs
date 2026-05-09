use ely_design_system::colors;
use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::IconName;

pub(crate) fn render_section_chevron_label(label: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_color(rgb(colors::INK_3))
        .text_size(px(12.5))
        .font_weight(FontWeight(500.0))
        .child(label)
        .child(
            div()
                .text_color(rgb(colors::INK_4))
                .child(IconName::ChevronDown),
        )
        .into_any_element()
}
