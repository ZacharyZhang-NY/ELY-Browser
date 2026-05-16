use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, hsla, linear_color_stop, linear_gradient,
    prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::shell::ElyShell;
use crate::shell::chrome::animations::chrome_motion_feedback;
use crate::shell::chrome::sidebar_chrome::{
    ROW_CLOSE_SIZE, active_nav_bg, active_nav_bg_hover, close_hover_bg, highlight_border,
    hover_nav_bg, panel_bg, panel_shadow, profile_initial, render_sidebar_resize_handle,
    render_unread_badge, section_label, section_tabs_label, soft_shadow,
};
use crate::shell::chrome::{render_glyph_for, render_sidebar_header};

// `panel_bg` and `panel_shadow` are re-exported from `chrome::mod` via
// `sidebar_chrome` so external callers (`shell::render`, command overlay)
// keep their existing import paths.

impl ElyShell {
    pub(crate) fn render_expanded_sidebar(
        &mut self,
        snapshot: &BrowserSnapshot,
        sidebar_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel_color = panel_bg(snapshot);
        // Outer wrapper holds the resize handle outside the rounded
        // panel's overflow_hidden clip; inner panel keeps the rounded
        // glass treatment without swallowing the column-resize column.
        div()
            .w(px(sidebar_width))
            .h_full()
            .relative()
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(px(spacing::RADIUS_CARD))
                    .bg(rgba(panel_color))
                    .border_1()
                    .border_color(rgba(highlight_border()))
                    .shadow(panel_shadow())
                    .overflow_hidden()
                    .child(render_sidebar_header(self, snapshot, cx))
                    .child(
                        div()
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p(px(10.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(self.render_home_anchor_row(snapshot, cx))
                            .children(snapshot.favorites.iter().map(|tab| {
                                self.render_launcher_row(
                                    tab,
                                    tab.id() == &snapshot.active_tab_id,
                                    cx,
                                )
                            }))
                            .child(section_label("PINNED"))
                            .children(snapshot.pinned_tabs.iter().map(|tab| {
                                self.render_launcher_row(
                                    tab,
                                    tab.id() == &snapshot.active_tab_id,
                                    cx,
                                )
                            }))
                            .child(section_tabs_label(snapshot.tabs.len()))
                            .children(self.render_sidebar_tab_rows(snapshot, cx))
                            .child(self.render_new_tab_row(cx)),
                    )
                    .child(self.render_sidebar_footer(snapshot, cx)),
            )
            .child(render_sidebar_resize_handle(cx))
            .into_any_element()
    }

    fn render_sidebar_footer(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .px(px(10.0))
            .py(px(8.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgba(colors::divider()))
            .child(self.render_settings_row(cx))
            .child(self.render_profile_row(snapshot, cx))
            .into_any_element()
    }

    fn render_settings_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Settings is never an "active" row — only ever rest or hover —
        // so it uses hover_nav_bg() directly. Keeping it lighter than the
        // active nav card means the eye still finds the active selection
        // first when both are visible.
        let target = "nav-settings";
        let press_id = self.chrome_motion_animation_id(target);
        let element = div()
            .id(SharedString::from(target))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(colors::ink_2()))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(hover_nav_bg())).text_color(rgb(colors::ink())))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(target);
                shell.open_internal_tab("ely://settings", window, cx);
            }))
            .child(div().text_color(rgb(colors::ink_3())).child(IconName::Settings))
            .child("Settings");

        chrome_motion_feedback(press_id, "nav-settings-selection", false, element)
    }

    fn render_profile_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let profile_name = snapshot.active_profile_name.clone();

        let target = "nav-profile";
        let press_id = self.chrome_motion_animation_id(target);
        let element = div()
            .id(SharedString::from(target))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(6.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(hover_nav_bg())))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(target);
                shell.open_internal_tab("ely://settings/profiles", window, cx);
            }))
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
                            .child(profile_initial(&profile_name)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(colors::ink_2()))
                    .child(profile_name),
            )
            .child(
                // The profile chip navigates to settings/profiles —
                // it's not a popover. Use a right-chevron so the icon
                // promises "this opens a page" instead of the down
                // chevron that promises "this opens a menu inline".
                div().text_color(rgb(colors::ink_4())).child(IconName::ChevronRight),
            );

        chrome_motion_feedback(press_id, "nav-profile-selection", false, element)
    }

    fn render_home_anchor_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active_tab = snapshot.tabs.iter().find(|tab| tab.id() == &snapshot.active_tab_id);
        let active = active_tab.map(|tab| tab.url().as_str() == "ely://new-tab").unwrap_or(false);
        let palette = nav_row_palette(active);
        let target = "nav-home";
        let press_id = self.chrome_motion_animation_id(target);

        let element = div()
            .id(SharedString::from(target))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(move |style| style.bg(rgba(palette.hover_bg)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(palette.bg))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(target);
                shell.open_internal_tab("ely://new-tab", window, cx);
            }))
            .child(div().text_color(rgb(colors::ink_3())).child(IconName::Frame))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(palette.text))
                    .child("Home"),
            );

        chrome_motion_feedback(press_id, "nav-home-selection", active, element)
    }

    fn render_launcher_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let close_tab_id = tab.id().clone();
        let palette = nav_row_palette(active);
        let host = tab.url().host().map(|host| host.to_string());
        let title = tab.title().to_string();
        let initial = title.chars().next().unwrap_or('?').to_string();
        let unread = tab.unread_count();
        let row_id = format!("nav-{}", tab.id().as_str());
        let row_motion_target = SharedString::from(row_id.clone());
        let row_press_id = self.chrome_motion_animation_id(&row_id);
        let row_selection_id = SharedString::from(format!("{row_id}-selection"));
        let group_name = SharedString::from(format!("launcher-{}", tab.id().as_str()));
        let close_id = SharedString::from(format!("launcher-close-{}", tab.id().as_str()));
        let close_motion_target = close_id.clone();
        let close_press_id = self.chrome_motion_animation_id(close_id.as_str());

        let element = div()
            .id(SharedString::from(row_id))
            .group(group_name.clone())
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(move |style| style.bg(rgba(palette.hover_bg)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(palette.bg))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(row_motion_target.clone());
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(render_glyph_for(host.as_deref(), &initial, 18.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .truncate()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(palette.text))
                    .child(title),
            )
            .when(unread > 0, |el| el.child(render_unread_badge(unread)))
            .child(render_row_close_button(
                close_id,
                group_name,
                close_press_id,
                cx.listener(move |shell, _, window, cx| {
                    shell.trigger_chrome_motion(close_motion_target.clone());
                    shell.close_tab_by_id(&close_tab_id, window, cx);
                    cx.stop_propagation();
                }),
            ));

        chrome_motion_feedback(row_press_id, row_selection_id, active, element)
    }

    fn render_new_tab_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let target = "nav-new-tab";
        let press_id = self.chrome_motion_animation_id(target);
        let element = div()
            .id(SharedString::from(target))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .text_color(rgb(colors::ink_3()))
            .text_size(px(13.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(hover_nav_bg())).text_color(rgb(colors::ink())))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(target);
                shell.open_new_tab(window, cx);
            }))
            .child(div().text_color(rgb(colors::ink_4())).child(IconName::Plus))
            .child("New Tab");

        chrome_motion_feedback(press_id, "nav-new-tab-selection", false, element)
    }

    pub(crate) fn render_tab_row(
        &mut self,
        tab: &BrowserTab,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = tab.id().clone();
        let close_tab_id = tab.id().clone();
        let palette = nav_row_palette(active);
        let group_name = SharedString::from(format!("tab-{}", tab.id().as_str()));
        let close_id = SharedString::from(format!("tab-close-{}", tab.id().as_str()));
        let title = tab.title().to_string();
        let initial = title.chars().next().unwrap_or('?').to_string();
        let row_id = tab.id().as_str().to_string();
        let row_motion_target = SharedString::from(row_id.clone());
        let row_press_id = self.chrome_motion_animation_id(&row_id);
        let row_selection_id = SharedString::from(format!("tab-selection-{row_id}"));
        let close_motion_target = close_id.clone();
        let close_press_id = self.chrome_motion_animation_id(close_id.as_str());

        let element = div()
            .id(SharedString::from(row_id))
            .group(group_name.clone())
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(move |style| style.bg(rgba(palette.hover_bg)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(palette.bg))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.trigger_chrome_motion(row_motion_target.clone());
                shell.select_tab(&tab_id, window, cx);
            }))
            .child(render_tab_favicon(tab, &initial))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(500.0))
                            .text_color(rgb(palette.text))
                            .truncate()
                            .child(tab.title().to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(colors::ink_4()))
                            .truncate()
                            .child(tab.display_url()),
                    ),
            )
            .child(render_row_close_button(
                close_id,
                group_name,
                close_press_id,
                cx.listener(move |shell, _, window, cx| {
                    shell.trigger_chrome_motion(close_motion_target.clone());
                    shell.close_tab_by_id(&close_tab_id, window, cx);
                    cx.stop_propagation();
                }),
            ));

        chrome_motion_feedback(row_press_id, row_selection_id, active, element)
    }
}

/// Resolve the three colors a selectable nav row paints in based on
/// its active state. Centralising the choice means launcher row, tab
/// row, and home anchor row can never disagree on what "active" looks
/// like — and a future tweak to the contrast ladder lands in one
/// place rather than three transparent-sentinel triples.
fn nav_row_palette(active: bool) -> NavRowPalette {
    if active {
        NavRowPalette { bg: active_nav_bg(), hover_bg: active_nav_bg_hover(), text: colors::ink() }
    } else {
        NavRowPalette { bg: 0x00000000, hover_bg: hover_nav_bg(), text: colors::ink_2() }
    }
}

#[derive(Clone, Copy)]
struct NavRowPalette {
    bg: u32,
    hover_bg: u32,
    text: u32,
}

/// Per-row close (×) button shared by the launcher and tab rows.
///
/// Centralising the recipe keeps both rows obeying the same physical
/// rules: an 18 px circle (a coin, not a glyph), invisible until the
/// row is hovered, with a warm-dark wash on direct hover so the
/// button itself acknowledges the cursor before the click. The close
/// hit target stays `flex_shrink_0` so a long title can never push
/// the button into a sub-pixel sliver.
fn render_row_close_button<F>(
    close_id: SharedString,
    group_name: SharedString,
    press_id: Option<SharedString>,
    on_click: F,
) -> AnyElement
where
    F: Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    let selection_id = SharedString::from(format!("{}-selection", close_id.as_str()));
    let element = div()
        .id(close_id)
        .size(px(ROW_CLOSE_SIZE))
        .rounded_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::ink_4()))
        .opacity(0.0)
        .group_hover(group_name, |style| style.opacity(1.0))
        .hover(|style| style.bg(rgba(close_hover_bg())).text_color(rgb(colors::ink())))
        .cursor_pointer()
        .on_click(on_click)
        .child(IconName::Close);

    chrome_motion_feedback(press_id, selection_id, false, element)
}

/// Resolve the favicon glyph for a tab row. Favicon metadata stays as
/// a local key; rows render the host-derived glyph so the sidebar never
/// blocks on network image assets.
fn render_tab_favicon(tab: &BrowserTab, initial: &str) -> AnyElement {
    let host = tab.url().host();
    render_glyph_for(host.as_deref(), initial, FAVICON_SIZE)
}

const FAVICON_SIZE: f32 = 16.0;
