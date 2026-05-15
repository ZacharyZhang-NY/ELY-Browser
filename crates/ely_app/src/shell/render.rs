use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::{BrowserTab, DEFAULT_SIDEBAR_WIDTH_PX, HIDDEN_SIDEBAR_WIDTH_PX};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px, rgb, rgba,
};

use super::chrome::command_match::visible_command_rows;
use super::chrome::{
    SANS_FAMILY, WorkspaceDisclosureAnchor, panel_bg, panel_shadow, render_command_overlay,
    render_topbar as render_topbar_chrome, render_wallpaper, render_workspace_disclosure,
    render_workspace_disclosure_backdrop,
};
use super::sidebar::collapsed_sidebar_active;
use super::{ElyShell, ShellState};

impl Render for ElyShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.state {
            ShellState::Ready(core) => match (core.snapshot(), core.active_tab().cloned()) {
                (Ok(snapshot), Ok(active_tab)) => {
                    self.render_browser(snapshot, active_tab, window, cx)
                }
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar_width = match active_sidebar_width(&snapshot) {
            Ok(sidebar_width) => sidebar_width,
            Err(message) => return render_error(message),
        };
        let sidebar_hidden = sidebar_width <= f32::from(HIDDEN_SIDEBAR_WIDTH_PX);
        let sidebar_collapsed = collapsed_sidebar_active(sidebar_width) && !sidebar_hidden;
        let hover_expanded = sidebar_hidden && self.sidebar_hover_expanded;

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
            .on_mouse_move(cx.listener(Self::on_window_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_window_mouse_up))
            .capture_key_down(cx.listener(Self::on_command_overlay_key_down))
            .font_family(SANS_FAMILY)
            .text_color(rgb(colors::INK))
            .child(render_wallpaper(snapshot.appearance.wallpaper()))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .p(px(spacing::SHELL_INSET))
                    .gap(px(spacing::SIDEBAR_MAIN_GAP))
                    .flex()
                    .child(self.render_sidebar(
                        &snapshot,
                        sidebar_width,
                        sidebar_collapsed,
                        sidebar_hidden,
                        cx,
                    ))
                    .child(self.render_main_pane(
                        &snapshot,
                        &active_tab,
                        sidebar_collapsed,
                        window,
                        cx,
                    )),
            )
            .when(hover_expanded, |el| el.child(self.render_hidden_sidebar_overlay(&snapshot, cx)))
            // Workspace popover only makes sense when the picker pill is
            // visible — i.e. the sidebar is expanded. Compact/hidden
            // modes don't render the trigger, so showing the disclosure
            // would float a stranded card with no anchor.
            .when(self.workspace_picker_open && !sidebar_collapsed && !sidebar_hidden, |el| {
                let anchor = WorkspaceDisclosureAnchor::solve(sidebar_width);
                el.child(render_workspace_disclosure_backdrop(cx))
                    .child(render_workspace_disclosure(&snapshot, anchor, cx))
            })
            .children(render_command_overlay(self, &snapshot, cx))
            .into_any_element()
    }

    fn render_hidden_sidebar_overlay(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id(SharedString::from("hidden-sidebar-backdrop"))
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|shell, _, _, cx| shell.collapse_hidden_sidebar(cx))),
            )
            .child(
                div()
                    .absolute()
                    .top(px(spacing::SHELL_INSET))
                    .left(px(spacing::SHELL_INSET))
                    .bottom(px(spacing::SHELL_INSET))
                    .child(self.render_expanded_sidebar(
                        snapshot,
                        f32::from(DEFAULT_SIDEBAR_WIDTH_PX),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_main_pane(
        &mut self,
        snapshot: &BrowserSnapshot,
        active_tab: &BrowserTab,
        sidebar_collapsed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel_color = panel_bg(snapshot);
        div()
            .flex_1()
            .h_full()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(spacing::RADIUS_CARD))
            .bg(rgba(panel_color))
            .border_1()
            .border_color(rgba(MAIN_PANE_HIGHLIGHT_BORDER))
            .shadow(panel_shadow())
            .overflow_hidden()
            .child(render_topbar_chrome(self, snapshot, active_tab, sidebar_collapsed, window, cx))
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
        sidebar_hidden: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if sidebar_hidden {
            return self.render_hidden_sidebar_rail(cx);
        }
        if sidebar_collapsed {
            return self.render_compact_sidebar(snapshot, sidebar_width, cx);
        }
        self.render_expanded_sidebar(snapshot, sidebar_width, cx)
    }

    fn on_command_overlay_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        if !matches!(key, "up" | "down" | "enter") {
            return;
        }

        let total_rows = match &self.state {
            ShellState::Ready(core) => match core.snapshot() {
                Ok(snapshot) => {
                    let Some(stripped) = snapshot.command_query.strip_prefix('>') else {
                        return;
                    };
                    let needle = stripped.trim().to_lowercase();
                    visible_command_rows(&snapshot, &needle).len()
                }
                Err(_) => return,
            },
            ShellState::StartupError(_) => return,
        };

        match key {
            "up" => self.command_select_prev(total_rows, cx),
            "down" => self.command_select_next(total_rows, cx),
            "enter" => self.activate_selected_command(window, cx),
            _ => {}
        }
        cx.stop_propagation();
    }

    fn on_window_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor_x = f32::from(event.position.x);

        if let Some((origin_x, origin_width)) = self.sidebar_resize_origin {
            let delta = cursor_x - origin_x;
            let candidate = (f32::from(origin_width) + delta)
                .clamp(MIN_RESIZE_WIDTH_PX, MAX_RESIZE_WIDTH_PX)
                .round() as u16;
            self.set_active_sidebar_width(candidate, cx);
            return;
        }

        if (REVEAL_THRESHOLD_PX..=COLLAPSE_THRESHOLD_PX).contains(&cursor_x) {
            return;
        }

        let ShellState::Ready(core) = &self.state else {
            return;
        };
        let Some(width) = core.active_space_sidebar_width() else {
            return;
        };
        if width > HIDDEN_SIDEBAR_WIDTH_PX {
            return;
        }

        if !self.sidebar_hover_expanded && cursor_x < REVEAL_THRESHOLD_PX {
            self.expand_hidden_sidebar(cx);
        } else if self.sidebar_hover_expanded && cursor_x > COLLAPSE_THRESHOLD_PX {
            self.collapse_hidden_sidebar(cx);
        }
    }

    pub(super) fn begin_sidebar_resize(&mut self, cursor_x: f32, cx: &mut Context<Self>) {
        let ShellState::Ready(core) = &self.state else {
            return;
        };
        let Some(width) = core.active_space_sidebar_width() else {
            return;
        };
        self.sidebar_resize_origin = Some((cursor_x, width));
        cx.notify();
    }

    pub(super) fn end_sidebar_resize(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_resize_origin.take().is_some() {
            cx.notify();
        }
    }

    fn on_window_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_sidebar_resize(cx);
    }

    fn render_hidden_sidebar_rail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(SharedString::from("hidden-sidebar-rail"))
            .w(px(f32::from(HIDDEN_SIDEBAR_WIDTH_PX)))
            .h_full()
            .rounded(px(spacing::RADIUS_PANE))
            .bg(rgba(0xffffff8c))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0xffffffd9)))
            .active(|style| style.opacity(0.78))
            .on_click(cx.listener(|shell, _, _, cx| shell.expand_hidden_sidebar(cx)))
            .into_any_element()
    }
}

/// 50% white inner border that traces every glass panel — the GPUI
/// substitute for the design's `box-shadow: inset 0 0 0 1px rgba(255,255,255,0.5)`.
/// Painted as the panel's own border so it stays part of the frame and
/// never participates in hit testing.
const MAIN_PANE_HIGHLIGHT_BORDER: u32 = 0xffffff80;

/// Lower clamp for the live sidebar-resize drag. Stays a touch above
/// the per-space DEFAULT so a casual flick doesn't snap the user to a
/// nearly-collapsed pane that's awkward to recover from. The toggle
/// button still owns the discrete collapse-to-rail behavior.
pub(super) const MIN_RESIZE_WIDTH_PX: f32 = 220.0;

/// Upper clamp — wider than this and the main pane disappears on
/// smaller windows. Tuned to fit comfortably on a 1280-wide window.
pub(super) const MAX_RESIZE_WIDTH_PX: f32 = 480.0;

/// Cursor x within this px from the left edge auto-reveals the hidden sidebar.
/// Sized so the user only triggers the reveal when they actually approach the
/// rail, not when they hover the page content.
const REVEAL_THRESHOLD_PX: f32 = 24.0;

/// Once revealed, cursor x past this px collapses the hidden sidebar back to
/// the rail. Sits past the right edge of the expanded sidebar plus a small
/// buffer so brief overshoots don't ping-pong the state.
const COLLAPSE_THRESHOLD_PX: f32 = spacing::SHELL_INSET + DEFAULT_SIDEBAR_WIDTH_PX as f32 + 24.0;

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

pub(super) fn tab_profile_label(tab: &BrowserTab, profiles: &[ely_domain::Profile]) -> String {
    profiles
        .iter()
        .find(|profile| profile.id() == tab.profile_id())
        .map(|profile| format!("Profile: {}", profile.name()))
        .unwrap_or_else(|| format!("Profile: {}", tab.profile_id().as_str()))
}
