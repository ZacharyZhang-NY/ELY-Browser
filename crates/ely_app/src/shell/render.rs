use ely_browser_core::BrowserSnapshot;
use ely_design_system::{ELY_THEME, colors, spacing};
use ely_domain::{ArchiveSource, ArchivedTab, BrowserTab, Space};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};

use super::{ElyShell, ShellState};

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

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_close_current_tab))
            .on_action(cx.listener(Self::on_focus_address_bar))
            .on_action(cx.listener(Self::on_focus_command_mode))
            .on_action(cx.listener(Self::on_open_downloads))
            .on_action(cx.listener(Self::on_open_history))
            .on_action(cx.listener(Self::on_open_new_tab))
            .on_action(cx.listener(Self::on_open_settings))
            .on_action(cx.listener(Self::on_open_task_manager))
            .on_action(cx.listener(Self::on_restore_closed_tab))
            .on_action(cx.listener(Self::on_select_next_tab))
            .on_action(cx.listener(Self::on_select_previous_tab))
            .on_action(cx.listener(Self::on_split_right))
            .on_action(cx.listener(Self::on_toggle_favorite_tab))
            .on_action(cx.listener(Self::on_toggle_pinned_tab))
            .bg(rgb(ELY_THEME.canvas))
            .text_color(rgb(ELY_THEME.ink))
            .flex()
            .flex_col()
            .child(self.render_command_bar(&snapshot, &active_tab, sidebar_width, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(&snapshot, sidebar_width, cx))
                    .child(self.render_content_area(&snapshot, &active_tab, cx)),
            )
            .into_any_element()
    }

    fn render_command_bar(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        sidebar_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let favorite_icon =
            if active_tab.flags().favorite { IconName::Star } else { IconName::StarOff };
        let favorite_tooltip =
            if active_tab.flags().favorite { "Remove Favorite" } else { "Add Favorite" };
        let pinned_tooltip = if active_tab.flags().pinned { "Unpin Tab" } else { "Pin Tab" };

        div()
            .h(px(spacing::COMMAND_BAR_HEIGHT))
            .px_4()
            .gap_3()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(colors::HAIRLINE))
            .child(
                div()
                    .w(px(sidebar_width - spacing::XL))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(18.0)).font_semibold().child("ELY Browser"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors::MUTED))
                            .child(snapshot.active_space_name.clone()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(40.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(colors::HAIRLINE_STRONG))
                    .bg(rgb(colors::SURFACE_CARD))
                    .px_3()
                    .child(Input::new(&self.command_input).appearance(false).cleanable(true)),
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
                    .primary()
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(sidebar_width))
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_r_1()
            .border_color(rgb(colors::HAIRLINE))
            .bg(rgb(colors::CANVAS))
            .child(section_label("Favorites"))
            .children(
                snapshot.favorites.iter().map(|tab| {
                    self.render_favorite_row(tab, tab.id() == &snapshot.active_tab_id, cx)
                }),
            )
            .child(section_label("Pinned"))
            .children(
                snapshot.pinned_tabs.iter().map(|tab| {
                    self.render_pinned_row(tab, tab.id() == &snapshot.active_tab_id, cx)
                }),
            )
            .child(section_label("Space"))
            .children(snapshot.spaces.iter().map(|space| {
                self.render_space_row(space, space.id() == &snapshot.active_space_id, cx)
            }))
            .child(section_label("Tabs"))
            .children(self.render_sidebar_tab_rows(snapshot, cx))
            .child(section_label("Archive"))
            .children(
                snapshot
                    .archived_tabs
                    .iter()
                    .rev()
                    .map(|archived_tab| self.render_archived_row(archived_tab, cx)),
            )
            .child(div().flex_1())
            .child(section_label("Profile"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(colors::BODY))
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
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::HAIRLINE_STRONG } else { colors::HAIRLINE };

        div()
            .id(SharedString::from(format!("space-{}", space.id().as_str())))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_space(&space_id, window, cx);
            }))
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(rgb(colors::PRIMARY))
                    .child(space.icon().to_string()),
            )
            .child(div().text_sm().font_semibold().child(space.name().to_string()))
            .into_any_element()
    }

    fn render_favorite_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::HAIRLINE_STRONG } else { colors::HAIRLINE };

        div()
            .id(SharedString::from(format!("favorite-{}", tab.id().as_str())))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(div().text_color(rgb(colors::PRIMARY)).child(IconName::Star))
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
                            .child(tab.title().to_string()),
                    )
                    .child(div().text_xs().text_color(rgb(colors::MUTED)).child(tab.display_url())),
            )
            .into_any_element()
    }

    fn render_pinned_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::HAIRLINE_STRONG } else { colors::HAIRLINE };

        div()
            .id(SharedString::from(format!("pinned-{}", tab.id().as_str())))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(div().text_color(rgb(colors::MUTED)).child(IconName::Asterisk))
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
                            .child(tab.title().to_string()),
                    )
                    .child(div().text_xs().text_color(rgb(colors::MUTED)).child(tab.display_url())),
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
        let background = if active { colors::SURFACE_CARD } else { colors::CANVAS };
        let border = if active { colors::HAIRLINE_STRONG } else { colors::HAIRLINE };

        div()
            .id(SharedString::from(tab.id().as_str().to_string()))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .px_3()
            .py_2()
            .gap_1()
            .flex()
            .flex_col()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(colors::INK))
                    .child(tab.title().to_string()),
            )
            .child(div().text_xs().text_color(rgb(colors::MUTED)).child(tab.display_url()))
            .into_any_element()
    }

    fn render_archived_row(
        &mut self,
        archived_tab: &ArchivedTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = archived_tab.tab();
        let tab_id = tab.id().clone();

        div()
            .id(SharedString::from(format!("archived-{}", tab.id().as_str())))
            .rounded_md()
            .border_1()
            .border_color(rgb(colors::HAIRLINE))
            .bg(rgb(colors::CANVAS))
            .px_3()
            .py_2()
            .gap_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(colors::SURFACE_CARD)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.restore_archived_tab(&tab_id, window, cx);
            }))
            .child(div().text_color(rgb(colors::MUTED)).child(IconName::Undo2))
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
                            .child(tab.title().to_string()),
                    )
                    .child(div().text_xs().text_color(rgb(colors::MUTED)).child(format!(
                        "{} - {}",
                        tab.display_url(),
                        archive_source_label(archived_tab.source())
                    ))),
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
    div().text_xs().font_semibold().text_color(rgb(colors::MUTED)).child(label)
}

fn active_sidebar_width(snapshot: &BrowserSnapshot) -> Result<f32, String> {
    let Some(active_space) =
        snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
    else {
        return Err("Active Space is unavailable.".to_string());
    };

    Ok(f32::from(active_space.sidebar_width_px()))
}

fn archive_source_label(source: &ArchiveSource) -> &'static str {
    match source {
        ArchiveSource::ManualClose => "Closed",
        ArchiveSource::AutoArchive => "Auto archived",
    }
}
