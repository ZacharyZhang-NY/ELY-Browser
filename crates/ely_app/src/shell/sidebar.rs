use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{
    ArchivedTab, BrowserTab, COLLAPSED_SIDEBAR_WIDTH_PX, DEFAULT_SIDEBAR_WIDTH_PX, Space,
};
use gpui::{
    AnyElement, BoxShadow, Context, IntoElement, ParentElement, Styled, Window, div, hsla, point,
    px, rgb, rgba,
};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};

use super::{ElyShell, ShellState, render::tab_profile_label};
use crate::ToggleSidebar;

impl ElyShell {
    pub(super) fn on_toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_sidebar_width(cx);
    }

    pub(super) fn render_compact_sidebar(
        &mut self,
        snapshot: &BrowserSnapshot,
        sidebar_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(sidebar_width))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .p_2()
            .rounded(px(spacing::RADIUS_CARD))
            .bg(rgba(PANEL_BG))
            .shadow(panel_shadow())
            .children(snapshot.favorites.iter().enumerate().map(|(index, tab)| {
                self.render_compact_tab_button(
                    ("compact-favorite", index),
                    tab,
                    snapshot,
                    tab.id() == &snapshot.active_tab_id,
                    IconName::Star,
                    cx,
                )
            }))
            .children(snapshot.pinned_tabs.iter().enumerate().map(|(index, tab)| {
                self.render_compact_tab_button(
                    ("compact-pinned", index),
                    tab,
                    snapshot,
                    tab.id() == &snapshot.active_tab_id,
                    IconName::Asterisk,
                    cx,
                )
            }))
            .children(snapshot.spaces.iter().enumerate().map(|(index, space)| {
                self.render_compact_space_button(
                    index,
                    space,
                    space.id() == &snapshot.active_space_id,
                    cx,
                )
            }))
            .children(snapshot.tabs.iter().enumerate().map(|(index, tab)| {
                self.render_compact_tab_button(
                    ("compact-tab", index),
                    tab,
                    snapshot,
                    tab.id() == &snapshot.active_tab_id,
                    IconName::Globe,
                    cx,
                )
            }))
            .children(snapshot.archived_tabs.iter().rev().enumerate().map(
                |(index, archived_tab)| {
                    self.render_compact_archived_button(index, archived_tab, cx)
                },
            ))
            .into_any_element()
    }

    fn toggle_sidebar_width(&mut self, cx: &mut Context<Self>) {
        let ShellState::Ready(core) = &mut self.state else {
            return;
        };
        let Ok(snapshot) = core.snapshot() else {
            return;
        };
        let Some(active_space) =
            snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
        else {
            return;
        };

        let next_width = if active_space.sidebar_width_px() <= COLLAPSED_SIDEBAR_WIDTH_PX {
            DEFAULT_SIDEBAR_WIDTH_PX
        } else {
            COLLAPSED_SIDEBAR_WIDTH_PX
        };

        if core.set_space_sidebar_width(&snapshot.active_space_id, next_width).is_ok() {
            cx.notify();
        }
    }

    fn render_compact_space_button(
        &mut self,
        index: usize,
        space: &Space,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let space_id = space.id().clone();
        Button::new(("compact-space", index))
            .ghost()
            .small()
            .selected(active)
            .label(space.icon().to_string())
            .tooltip(space.name().to_string())
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_space(&space_id, window, cx);
            }))
            .into_any_element()
    }

    fn render_compact_tab_button(
        &mut self,
        id: (&'static str, usize),
        tab: &BrowserTab,
        snapshot: &BrowserSnapshot,
        active: bool,
        icon: IconName,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let tooltip = format!("{} - {}", tab.title(), tab_profile_label(tab, &snapshot.profiles));
        Button::new(id)
            .ghost()
            .small()
            .selected(active)
            .icon(icon)
            .tooltip(tooltip)
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .into_any_element()
    }

    fn render_compact_archived_button(
        &mut self,
        index: usize,
        archived_tab: &ArchivedTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = archived_tab.tab();
        let tab_id = tab.id().clone();
        Button::new(("compact-archive", index))
            .ghost()
            .small()
            .icon(IconName::Undo2)
            .tooltip(tab.title().to_string())
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.restore_archived_tab(&tab_id, window, cx);
            }))
            .into_any_element()
    }
}

pub(super) fn render_command_bar_identity(
    snapshot: &BrowserSnapshot,
    _sidebar_width: f32,
    sidebar_collapsed: bool,
) -> AnyElement {
    if sidebar_collapsed {
        return div()
            .flex()
            .items_center()
            .child(
                div()
                    .size(px(28.0))
                    .rounded(px(spacing::RADIUS_NAV))
                    .bg(rgb(colors::ACCENT))
                    .text_color(rgb(colors::SURFACE_CARD))
                    .text_size(px(10.0))
                    .font_semibold()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("ELY"),
            )
            .into_any_element();
    }

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_size(px(15.0))
                .font_semibold()
                .text_color(rgb(colors::INK))
                .child("ELY Browser"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(colors::INK_3))
                .child(snapshot.active_space_name.clone()),
        )
        .into_any_element()
}

pub(super) fn collapsed_sidebar_active(sidebar_width: f32) -> bool {
    sidebar_width <= f32::from(COLLAPSED_SIDEBAR_WIDTH_PX)
}

const PANEL_BG: u32 = 0xffffffe0;

fn panel_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.30),
            offset: point(px(0.), px(20.)),
            blur_radius: px(50.),
            spread_radius: px(-15.),
        },
        BoxShadow {
            color: hsla(0., 0., 1., 0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(1.),
        },
    ]
}
