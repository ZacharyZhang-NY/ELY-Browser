use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, px, rgb, rgba,
};

use super::chrome::{
    PANEL_BG, WallpaperTheme, panel_shadow, render_topbar as render_topbar_chrome,
    render_wallpaper,
};
use super::sidebar::collapsed_sidebar_active;
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
            .text_color(rgb(colors::INK))
            .child(render_wallpaper(WallpaperTheme::Dawn))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .p(px(spacing::SHELL_INSET))
                    .gap(px(spacing::SIDEBAR_MAIN_GAP))
                    .flex()
                    .child(self.render_sidebar(&snapshot, sidebar_width, sidebar_collapsed, cx))
                    .child(self.render_main_pane(&snapshot, &active_tab, sidebar_collapsed, cx)),
            )
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
            .bg(rgba(PANEL_BG))
            .shadow(panel_shadow())
            .overflow_hidden()
            .child(render_topbar_chrome(self, snapshot, active_tab, sidebar_collapsed, cx))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_content_area(snapshot, active_tab, cx)),
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
        self.render_expanded_sidebar(snapshot, sidebar_width, cx)
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

fn active_sidebar_width(snapshot: &BrowserSnapshot) -> Result<f32, String> {
    let Some(active_space) =
        snapshot.spaces.iter().find(|space| space.id() == &snapshot.active_space_id)
    else {
        return Err("Active Space is unavailable.".to_string());
    };

    Ok(f32::from(active_space.sidebar_width_px()))
}

pub(super) fn tab_profile_label(
    tab: &BrowserTab,
    profiles: &[ely_domain::Profile],
) -> String {
    profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map(|profile| format!("Profile: {}", profile.name()))
        .unwrap_or_else(|| format!("Profile: {}", tab.profile_id().as_str()))
}
