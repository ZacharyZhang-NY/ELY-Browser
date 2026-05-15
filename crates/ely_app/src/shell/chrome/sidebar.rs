use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, Context, FontWeight, ImageSource, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, StyledImage, div, hsla, img,
    linear_color_stop, linear_gradient, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::shell::ElyShell;
use crate::shell::chrome::sidebar_chrome::{
    ACTIVE_NAV_BG, ACTIVE_NAV_BG_HOVER, CLOSE_HOVER_BG, HIGHLIGHT_BORDER, HOVER_NAV_BG,
    ROW_CLOSE_SIZE, panel_bg, panel_shadow, profile_initial, render_sidebar_resize_handle,
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
                    .border_color(rgba(HIGHLIGHT_BORDER))
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
            .border_color(rgba(colors::DIVIDER))
            .child(self.render_settings_row(cx))
            .child(self.render_profile_row(snapshot, cx))
            .into_any_element()
    }

    fn render_settings_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Settings is never an "active" row — only ever rest or hover —
        // so it uses HOVER_NAV_BG directly. Keeping it lighter than the
        // active nav card means the eye still finds the active selection
        // first when both are visible.
        div()
            .id(SharedString::from("nav-settings"))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(colors::INK_2))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(HOVER_NAV_BG)).text_color(rgb(colors::INK)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_internal_tab("ely://settings", window, cx);
            }))
            .child(div().text_color(rgb(colors::INK_3)).child(IconName::Settings))
            .child("Settings")
            .into_any_element()
    }

    fn render_profile_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let profile_name = snapshot.active_profile_name.clone();

        div()
            .id(SharedString::from("nav-profile"))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(6.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(HOVER_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
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
                    .text_color(rgb(colors::INK_2))
                    .child(profile_name),
            )
            .child(
                // The profile chip navigates to settings/profiles —
                // it's not a popover. Use a right-chevron so the icon
                // promises "this opens a page" instead of the down
                // chevron that promises "this opens a menu inline".
                div().text_color(rgb(colors::INK_4)).child(IconName::ChevronRight),
            )
            .into_any_element()
    }

    fn render_home_anchor_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active_tab = snapshot.tabs.iter().find(|tab| tab.id() == &snapshot.active_tab_id);
        let active = active_tab.map(|tab| tab.url().as_str() == "ely://new-tab").unwrap_or(false);
        let palette = nav_row_palette(active);

        div()
            .id(SharedString::from("nav-home"))
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
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_internal_tab("ely://new-tab", window, cx);
            }))
            .child(div().text_color(rgb(colors::INK_3)).child(IconName::Frame))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(palette.text))
                    .child("Home"),
            )
            .into_any_element()
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
        let group_name = SharedString::from(format!("launcher-{}", tab.id().as_str()));
        let close_id = SharedString::from(format!("launcher-close-{}", tab.id().as_str()));

        div()
            .id(SharedString::from(format!("nav-{}", tab.id().as_str())))
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
                cx.listener(move |shell, _, window, cx| {
                    shell.close_tab_by_id(&close_tab_id, window, cx);
                    cx.stop_propagation();
                }),
            ))
            .into_any_element()
    }

    fn render_new_tab_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(SharedString::from("nav-new-tab"))
            .rounded(px(spacing::RADIUS_NAV))
            .px(px(10.0))
            .py(px(7.0))
            .gap(px(10.0))
            .flex()
            .items_center()
            .text_color(rgb(colors::INK_3))
            .text_size(px(13.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(HOVER_NAV_BG)).text_color(rgb(colors::INK)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_new_tab(window, cx);
            }))
            .child(div().text_color(rgb(colors::INK_4)).child(IconName::Plus))
            .child("New Tab")
            .into_any_element()
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

        div()
            .id(SharedString::from(tab.id().as_str().to_string()))
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
                            .text_color(rgb(colors::INK_4))
                            .truncate()
                            .child(tab.display_url()),
                    ),
            )
            .child(render_row_close_button(
                close_id,
                group_name,
                cx.listener(move |shell, _, window, cx| {
                    shell.close_tab_by_id(&close_tab_id, window, cx);
                    cx.stop_propagation();
                }),
            ))
            .into_any_element()
    }
}

/// Resolve the three colors a selectable nav row paints in based on
/// its active state. Centralising the choice means launcher row, tab
/// row, and home anchor row can never disagree on what "active" looks
/// like — and a future tweak to the contrast ladder lands in one
/// place rather than three transparent-sentinel triples.
fn nav_row_palette(active: bool) -> NavRowPalette {
    if active {
        NavRowPalette { bg: ACTIVE_NAV_BG, hover_bg: ACTIVE_NAV_BG_HOVER, text: colors::INK }
    } else {
        NavRowPalette { bg: 0x00000000, hover_bg: HOVER_NAV_BG, text: colors::INK_2 }
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
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    div()
        .id(close_id)
        .size(px(ROW_CLOSE_SIZE))
        .rounded_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors::INK_4))
        .opacity(0.0)
        .group_hover(group_name, |style| style.opacity(1.0))
        .hover(|style| style.bg(rgba(CLOSE_HOVER_BG)).text_color(rgb(colors::INK)))
        .cursor_pointer()
        .on_click(on_click)
        .child(IconName::Close)
}

/// Resolve the favicon glyph for a tab row. Prefers the favicon URL
/// the Servo runtime derived from the loaded URL; falls back to the
/// initial-letter chip used everywhere else when the tab has no live
/// favicon (yet to load, internal page, file URL, etc.).
fn render_tab_favicon(tab: &BrowserTab, initial: &str) -> AnyElement {
    if let Some(favicon_url) = tab.favicon_key()
        && favicon_url.starts_with("http")
    {
        return div()
            .size(px(FAVICON_SIZE))
            .flex_shrink_0()
            .rounded(px(FAVICON_RADIUS))
            .overflow_hidden()
            .child(
                img(ImageSource::from(favicon_url.to_string()))
                    .size(px(FAVICON_SIZE))
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element();
    }

    let host = tab.url().host();
    render_glyph_for(host.as_deref(), initial, FAVICON_SIZE)
}

const FAVICON_SIZE: f32 = 16.0;
const FAVICON_RADIUS: f32 = 4.0;
