use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::NewTabDestination;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgb};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ElyShell, render_canvas_surface};

impl ElyShell {
    pub(super) fn render_general_page(
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
                .child(render_general_header(snapshot))
                .child(render_general_summary(snapshot.new_tab_destination))
                .child(render_new_tab_destinations(snapshot.new_tab_destination, cx)),
        )
    }
}

fn render_general_header(snapshot: &BrowserSnapshot) -> AnyElement {
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
                .child(div().text_size(px(26.0)).text_color(rgb(colors::INK)).child("General"))
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(rgb(colors::MUTED))
                        .child(format!("Profile: {}", snapshot.active_profile_name)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_semibold()
                .text_color(rgb(colors::MUTED))
                .child(IconName::Settings2)
                .child(snapshot.new_tab_destination.name()),
        )
        .into_any_element()
}

fn render_general_summary(destination: NewTabDestination) -> AnyElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(colors::HAIRLINE))
        .bg(rgb(colors::CANVAS_SOFT))
        .px_4()
        .py_3()
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
                .child(div().text_color(rgb(colors::PRIMARY)).child(destination_icon(destination)))
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
                                .text_color(rgb(colors::INK))
                                .child(format!("New Tab opens {}", destination.name())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(destination.detail()),
                        ),
                ),
        )
        .child(
            div().text_xs().font_semibold().text_color(rgb(colors::SUCCESS)).child("Saved locally"),
        )
        .into_any_element()
}

fn render_new_tab_destinations(
    active_destination: NewTabDestination,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scrollbar()
        .border_t_1()
        .border_color(rgb(colors::HAIRLINE))
        .children(NewTabDestination::ALL.iter().copied().enumerate().map(|(index, destination)| {
            render_new_tab_destination_row(index, destination, active_destination, cx)
        }))
        .into_any_element()
}

fn render_new_tab_destination_row(
    index: usize,
    destination: NewTabDestination,
    active_destination: NewTabDestination,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = destination == active_destination;

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
                .child(
                    div()
                        .text_color(rgb(destination_icon_color(selected)))
                        .child(destination_status_icon(destination, selected)),
                )
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
                                .child(destination.name()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(rgb(colors::MUTED))
                                .child(destination.detail()),
                        ),
                ),
        )
        .child(
            Button::new(("new-tab-destination", index))
                .ghost()
                .xsmall()
                .selected(selected)
                .label(destination_button_label(selected))
                .tooltip(destination.name())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.set_new_tab_destination(destination, cx);
                })),
        )
        .into_any_element()
}

fn destination_icon(destination: NewTabDestination) -> IconName {
    match destination {
        NewTabDestination::ElyNewTab => IconName::Plus,
        NewTabDestination::Bookmarks => IconName::BookOpen,
        NewTabDestination::ReadingList => IconName::Inbox,
    }
}

fn destination_status_icon(destination: NewTabDestination, selected: bool) -> IconName {
    if selected { IconName::CircleCheck } else { destination_icon(destination) }
}

fn destination_icon_color(selected: bool) -> u32 {
    if selected { colors::PRIMARY } else { colors::MUTED_SOFT }
}

fn destination_button_label(selected: bool) -> &'static str {
    if selected { "Active" } else { "Select" }
}
