use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{BrowserTab, SplitAxis, SplitLayout};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use gpui_component::{IconName, StyledExt};

use super::{ElyShell, ShellState};
use crate::SplitRight;

impl ElyShell {
    pub(super) fn render_content_area(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(layout) = active_split_layout(snapshot, active_tab) else {
            return self.render_web_canvas(active_tab, snapshot, cx);
        };

        if layout.pane_count() < 2 {
            return self.render_web_canvas(active_tab, snapshot, cx);
        }

        self.render_split_canvas(snapshot, active_tab, layout, cx)
    }

    pub(super) fn split_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let ShellState::Ready(core) = &mut self.state
            && core.split_active_tab_right().is_ok()
        {
            self.sync_address_input(window, cx);
            self.focus_address_bar(window, cx);
            cx.notify();
        }
    }

    pub(super) fn on_split_right(
        &mut self,
        _: &SplitRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_right(window, cx);
    }

    fn render_split_canvas(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        layout: &SplitLayout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panes = layout
            .panes()
            .iter()
            .filter_map(|pane| snapshot.tabs.iter().find(|tab| tab.id() == pane.tab_id()))
            .collect::<Vec<_>>();

        if panes.len() < 2 {
            return self.render_web_canvas(active_tab, snapshot, cx);
        }

        let body = div()
            .flex_1()
            .h_full()
            .min_w_0()
            .p_3()
            .gap_3()
            .overflow_hidden()
            .bg(rgb(colors::CANVAS));

        let body = match layout.axis() {
            SplitAxis::Vertical => body.flex().flex_col(),
            SplitAxis::Horizontal | SplitAxis::Grid => body.flex(),
        };

        body.children(
            panes
                .into_iter()
                .enumerate()
                .map(|(index, tab)| self.render_split_pane(index, tab, snapshot, cx)),
        )
        .into_any_element()
    }

    fn render_split_pane(
        &mut self,
        index: usize,
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = tab.id() == &snapshot.active_tab_id;
        let tab_id = tab.id().clone();
        let pane_number = index + 1;
        let border = if active { colors::PRIMARY } else { colors::HAIRLINE_STRONG };
        let title_color = if active { colors::INK } else { colors::BODY };

        div()
            .id(SharedString::from(format!("split-pane-{}", tab.id().as_str())))
            .flex_1()
            .h_full()
            .min_w(px(240.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(colors::SURFACE_CARD))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::CANVAS_SOFT)))
            .active(|style| style.opacity(0.92))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(
                div()
                    .h(px(spacing::COMMAND_BAR_HEIGHT - spacing::MD))
                    .px_3()
                    .gap_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(colors::HAIRLINE))
                    .bg(rgb(colors::CANVAS_SOFT))
                    .child(div().text_color(rgb(colors::MUTED)).child(IconName::Frame))
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(rgb(colors::MUTED))
                            .child(format!("Pane {pane_number}")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_semibold()
                            .text_color(rgb(title_color))
                            .child(tab.title().to_string()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_web_canvas(tab, snapshot, cx)),
            )
            .into_any_element()
    }
}

fn active_split_layout<'a>(
    snapshot: &'a BrowserSnapshot,
    active_tab: &BrowserTab,
) -> Option<&'a SplitLayout> {
    let split_id = active_tab.split_id()?;
    snapshot.split_layouts.iter().find(|layout| layout.id() == split_id)
}
