use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{ArchivedTab, BrowserTab, Profile, Space};
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::shell::ElyShell;
use crate::shell::archive_labels::archive_detail_label;
use crate::shell::chrome::render_sidebar_header;

impl ElyShell {
    pub(crate) fn render_expanded_sidebar(
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
            .rounded(px(spacing::RADIUS_CARD))
            .bg(rgba(PANEL_BG))
            .shadow(panel_shadow())
            .overflow_hidden()
            .child(render_sidebar_header(snapshot, cx))
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_label("FAVORITES"))
                    .children(snapshot.favorites.iter().map(|tab| {
                        self.render_favorite_row(
                            tab,
                            &snapshot.profiles,
                            tab.id() == &snapshot.active_tab_id,
                            cx,
                        )
                    }))
                    .child(section_label("PINNED"))
                    .children(snapshot.pinned_tabs.iter().map(|tab| {
                        self.render_pinned_row(
                            tab,
                            &snapshot.profiles,
                            tab.id() == &snapshot.active_tab_id,
                            cx,
                        )
                    }))
                    .child(section_label("SPACES"))
                    .children(snapshot.spaces.iter().map(|space| {
                        self.render_space_row(
                            space,
                            space.id() == &snapshot.active_space_id,
                            cx,
                        )
                    }))
                    .child(section_label("TABS"))
                    .children(self.render_sidebar_tab_rows(snapshot, cx))
                    .child(section_label("ARCHIVE"))
                    .children(
                        snapshot
                            .archived_tabs
                            .iter()
                            .rev()
                            .map(|archived_tab| {
                                self.render_archived_row(archived_tab, snapshot, cx)
                            }),
                    ),
            )
            .child(self.render_sidebar_footer(snapshot))
            .into_any_element()
    }

    fn render_sidebar_footer(&self, snapshot: &BrowserSnapshot) -> AnyElement {
        div()
            .px(px(14.0))
            .py(px(12.0))
            .flex()
            .items_center()
            .gap_2()
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgba(colors::DIVIDER))
            .child(
                div()
                    .size(px(26.0))
                    .rounded_full()
                    .bg(linear_gradient(
                        135.0,
                        linear_color_stop(hsla(20.0 / 360.0, 0.6, 0.84, 1.0), 0.0),
                        linear_color_stop(hsla(15.0 / 360.0, 0.55, 0.53, 1.0), 1.0),
                    ))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(rgb(0xffffff))
                            .child(profile_initial(&snapshot.active_profile_name)),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(colors::INK_2))
                    .child(snapshot.active_profile_name.clone()),
            )
            .into_any_element()
    }

    fn render_space_row(
        &mut self,
        space: &Space,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let space_id = space.id().clone();
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };

        div()
            .id(SharedString::from(format!("space-{}", space.id().as_str())))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_space(&space_id, window, cx);
            }))
            .child(render_workspace_tile(space))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(text_color))
                    .child(space.name().to_string()),
            )
            .into_any_element()
    }

    fn render_favorite_row(
        &mut self,
        tab: &BrowserTab,
        profiles: &[Profile],
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_nav_row(tab, profiles, active, IconName::Star, colors::ACCENT, cx)
    }

    fn render_pinned_row(
        &mut self,
        tab: &BrowserTab,
        profiles: &[Profile],
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_nav_row(tab, profiles, active, IconName::Asterisk, colors::INK_3, cx)
    }

    fn render_nav_row(
        &mut self,
        tab: &BrowserTab,
        _profiles: &[Profile],
        active: bool,
        icon: IconName,
        icon_color: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };

        div()
            .id(SharedString::from(format!("nav-{}", tab.id().as_str())))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(div().text_color(rgb(icon_color)).child(icon))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(text_color))
                    .child(tab.title().to_string()),
            )
            .into_any_element()
    }

    pub(crate) fn render_tab_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };

        div()
            .id(SharedString::from(tab.id().as_str().to_string()))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(6.0))
            .flex()
            .flex_col()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(text_color))
                    .overflow_hidden()
                    .child(tab.title().to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(colors::INK_4))
                    .overflow_hidden()
                    .child(tab.display_url()),
            )
            .into_any_element()
    }

    fn render_archived_row(
        &mut self,
        archived_tab: &ArchivedTab,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = archived_tab.tab();
        let tab_id = tab.id().clone();
        let detail = archive_detail_label(archived_tab, &snapshot.spaces, &snapshot.profiles);

        div()
            .id(SharedString::from(format!("archived-{}", tab.id().as_str())))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.restore_archived_tab(&tab_id, window, cx);
            }))
            .child(div().text_color(rgb(colors::INK_4)).child(IconName::Undo2))
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight(500.0))
                            .text_color(rgb(colors::INK_2))
                            .overflow_hidden()
                            .child(tab.title().to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(colors::INK_4))
                            .overflow_hidden()
                            .child(detail),
                    ),
            )
            .into_any_element()
    }
}

fn profile_initial(name: &str) -> String {
    name.chars()
        .next()
        .unwrap_or('P')
        .to_uppercase()
        .to_string()
}

fn render_workspace_tile(space: &Space) -> impl IntoElement {
    div()
        .size(px(32.0))
        .rounded(px(9.0))
        .bg(linear_gradient(
            135.0,
            linear_color_stop(hsla(0., 0., 1., 1.0), 0.0),
            linear_color_stop(hsla(20.0 / 360.0, 0.6, 0.94, 1.0), 1.0),
        ))
        .shadow(soft_shadow())
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgb(colors::ACCENT))
                .child(space.icon().to_string()),
        )
}

fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .pt(px(8.0))
        .pb(px(4.0))
        .px(px(10.0))
        .text_size(px(10.5))
        .font_weight(gpui::FontWeight(500.0))
        .text_color(rgb(colors::INK_4))
        .child(label)
}

pub(crate) const ACTIVE_NAV_BG: u32 = 0xffffffd9;
pub(crate) const PANEL_BG: u32 = 0xffffffe0;

pub(crate) fn panel_shadow() -> Vec<BoxShadow> {
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

pub(crate) fn soft_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(0., 0., 1., 0.7),
            offset: point(px(0.), px(1.)),
            blur_radius: px(0.),
            spread_radius: px(0.),
        },
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.08),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(1.),
        },
    ]
}
