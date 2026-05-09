use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{ArchivedTab, BrowserTab, Profile, Space};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb, rgba,
};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
    scroll::ScrollableElement,
};

use super::sidebar::{collapsed_sidebar_active, render_command_bar_identity};
use super::{ElyShell, ShellState, archive_labels::archive_detail_label};

impl Render for ElyShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.state {
            ShellState::Ready(core) => match (core.snapshot(), core.active_tab().cloned()) {
                (Ok(snapshot), Ok(active_tab)) => self.render_browser(snapshot, active_tab, cx),
                (Err(error), _) | (_, Err(error)) => render_error(error.to_string()),
            },
            ShellState::StartupError(message) => render_error(message.clone()),
        }
    }
}

impl ElyShell {
    fn render_browser(
        &mut self,
        snapshot: BrowserSnapshot,
        active_tab: BrowserTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar_width = match active_sidebar_width(&snapshot) {
            Ok(sidebar_width) => sidebar_width,
            Err(message) => return render_error(message),
        };
        let sidebar_collapsed = collapsed_sidebar_active(sidebar_width);

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(Self::on_external_web_key_down))
            .on_action(cx.listener(Self::on_close_current_tab))
            .on_action(cx.listener(Self::on_focus_address_bar))
            .on_action(cx.listener(Self::on_focus_command_mode))
            .on_action(cx.listener(Self::on_download_current_page))
            .on_action(cx.listener(Self::on_open_downloads))
            .on_action(cx.listener(Self::on_open_history))
            .on_action(cx.listener(Self::on_open_new_tab))
            .on_action(cx.listener(Self::on_open_settings))
            .on_action(cx.listener(Self::on_open_task_manager))
            .on_action(cx.listener(Self::on_reset_zoom))
            .on_action(cx.listener(Self::on_restore_closed_tab))
            .on_action(cx.listener(Self::on_select_next_space))
            .on_action(cx.listener(Self::on_select_next_tab))
            .on_action(cx.listener(Self::on_select_previous_space))
            .on_action(cx.listener(Self::on_select_previous_tab))
            .on_action(cx.listener(Self::on_split_right))
            .on_action(cx.listener(Self::on_toggle_favorite_tab))
            .on_action(cx.listener(Self::on_toggle_pinned_tab))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_zoom_in))
            .on_action(cx.listener(Self::on_zoom_out))
            .bg(rgb(colors::CANVAS))
            .text_color(rgb(colors::INK))
            .p(px(spacing::SHELL_INSET))
            .gap(px(spacing::SIDEBAR_MAIN_GAP))
            .flex()
            .child(self.render_sidebar(&snapshot, sidebar_width, sidebar_collapsed, cx))
            .child(self.render_main_pane(&snapshot, &active_tab, sidebar_collapsed, cx))
            .into_any_element()
    }

    fn render_main_pane(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        sidebar_collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_1()
            .h_full()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(spacing::RADIUS_CARD))
            .bg(rgba(colors::GLASS))
            .overflow_hidden()
            .child(self.render_topbar(snapshot, active_tab, sidebar_collapsed, cx))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_content_area(snapshot, active_tab, cx)),
            )
            .into_any_element()
    }

    fn render_topbar(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        sidebar_collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let favorite_icon =
            if active_tab.flags().favorite { IconName::Star } else { IconName::StarOff };
        let favorite_tooltip =
            if active_tab.flags().favorite { "Remove Favorite" } else { "Add Favorite" };
        let pinned_tooltip = if active_tab.flags().pinned { "Unpin Tab" } else { "Pin Tab" };

        div()
            .h(px(spacing::TOPBAR_HEIGHT))
            .px(px(14.0))
            .gap_2()
            .flex()
            .items_center()
            .flex_shrink_0()
            .children(if sidebar_collapsed {
                Some(render_command_bar_identity(snapshot, 56.0, true))
            } else {
                None
            })
            .child(
                div()
                    .flex_1()
                    .h(px(spacing::OMNIBAR_HEIGHT))
                    .rounded(px(spacing::RADIUS_PILL))
                    .bg(rgba(colors::GLASS_2))
                    .px(px(14.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(colors::INK_3))
                            .child(IconName::Search),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(
                                Input::new(&self.command_input)
                                    .appearance(false)
                                    .cleanable(true),
                            ),
                    ),
            )
            .child(
                Button::new("toggle-pinned-tab")
                    .ghost()
                    .small()
                    .selected(active_tab.flags().pinned)
                    .icon(IconName::Asterisk)
                    .tooltip(pinned_tooltip)
                    .on_click(cx.listener(|shell, _, _, cx| shell.toggle_active_tab_pinned(cx))),
            )
            .child(
                Button::new("toggle-favorite-tab")
                    .ghost()
                    .small()
                    .selected(active_tab.flags().favorite)
                    .icon(favorite_icon)
                    .tooltip(favorite_tooltip)
                    .on_click(cx.listener(|shell, _, _, cx| shell.toggle_active_tab_favorite(cx))),
            )
            .child(
                Button::new("new-tab")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip("New Tab")
                    .on_click(cx.listener(|shell, _, window, cx| shell.open_new_tab(window, cx))),
            )
            .into_any_element()
    }

    fn render_sidebar(
        &mut self,
        snapshot: &BrowserSnapshot,
        sidebar_width: f32,
        sidebar_collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if sidebar_collapsed {
            return self.render_compact_sidebar(snapshot, sidebar_width, cx);
        }

        div()
            .w(px(sidebar_width))
            .h_full()
            .flex()
            .flex_col()
            .rounded(px(spacing::RADIUS_CARD))
            .bg(rgba(colors::GLASS))
            .overflow_hidden()
            .child(self.render_sidebar_header(snapshot))
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

    fn render_sidebar_header(&self, snapshot: &BrowserSnapshot) -> AnyElement {
        let space_name = snapshot
            .spaces
            .iter()
            .find(|s| s.id() == &snapshot.active_space_id)
            .map(|s| s.name().to_string())
            .unwrap_or_default();

        div()
            .pt(px(48.0))
            .px(px(14.0))
            .pb(px(10.0))
            .flex()
            .flex_col()
            .gap_1()
            .flex_shrink_0()
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
                    .child(space_name),
            )
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
                    .bg(rgb(colors::ACCENT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(rgb(colors::SURFACE_CARD))
                            .child(
                                snapshot
                                    .active_profile_name
                                    .chars()
                                    .next()
                                    .unwrap_or('P')
                                    .to_uppercase()
                                    .to_string(),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
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
        let bg_color = if active { colors::GLASS_3 } else { 0x00000000 };
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
            .hover(|style| style.bg(rgba(colors::GLASS_3)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
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
        let bg_color = if active { colors::GLASS_3 } else { 0x00000000 };
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
            .hover(|style| style.bg(rgba(colors::GLASS_3)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
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

    pub(super) fn render_tab_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let bg_color = if active { colors::GLASS_3 } else { 0x00000000 };
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
            .hover(|style| style.bg(rgba(colors::GLASS_3)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
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
            .hover(|style| style.bg(rgba(colors::GLASS_3)))
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

fn render_error(message: String) -> AnyElement {
    div()
        .size_full()
        .bg(rgb(colors::CANVAS))
        .text_color(rgb(colors::ERROR))
        .flex()
        .items_center()
        .justify_center()
        .child(message)
        .into_any_element()
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

fn render_workspace_tile(space: &Space) -> impl IntoElement {
    div()
        .size(px(32.0))
        .rounded(px(9.0))
        .bg(rgba(colors::GLASS_3))
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

fn active_sidebar_width(snapshot: &BrowserSnapshot) -> Result<f32, String> {
    let Some(active_space) =
        snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
    else {
        return Err("Active Space is unavailable.".to_string());
    };

    Ok(f32::from(active_space.sidebar_width_px()))
}

pub(super) fn tab_profile_label(tab: &BrowserTab, profiles: &[Profile]) -> String {
    profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map(|profile| format!("Profile: {}", profile.name()))
        .unwrap_or_else(|| format!("Profile: {}", tab.profile_id().as_str()))
}
