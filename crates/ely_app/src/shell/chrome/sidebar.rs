use ely_browser_core::BrowserSnapshot;
use ely_design_system::{colors, spacing};
use ely_domain::BrowserTab;
use gpui::{
    AnyElement, BoxShadow, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, hsla,
    linear_color_stop, linear_gradient, point, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, StyledExt, scroll::ScrollableElement};

use crate::shell::ElyShell;
use crate::shell::chrome::{render_glyph_for, render_sidebar_header};

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
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)).text_color(rgb(colors::INK)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_internal_tab("ely://settings", window, cx);
            }))
            .child(
                div()
                    .text_color(rgb(colors::INK_3))
                    .child(IconName::Settings),
            )
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
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
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
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(colors::INK_2))
                    .child(profile_name),
            )
            .child(
                // The profile chip navigates to settings/profiles —
                // it's not a popover. Use a right-chevron so the icon
                // promises "this opens a page" instead of the down
                // chevron that promises "this opens a menu inline".
                div()
                    .text_color(rgb(colors::INK_4))
                    .child(IconName::ChevronRight),
            )
            .into_any_element()
    }

    fn render_home_anchor_row(
        &mut self,
        snapshot: &BrowserSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active_tab = snapshot
            .tabs
            .iter()
            .find(|tab| tab.id() == &snapshot.active_tab_id);
        let active = active_tab
            .map(|tab| tab.url().as_str() == "ely://new-tab")
            .unwrap_or(false);
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };

        div()
            .id(SharedString::from("nav-home"))
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
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_internal_tab("ely://new-tab", window, cx);
            }))
            .child(div().text_color(rgb(colors::INK_3)).child(IconName::Frame))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(text_color))
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
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };
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
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
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
                    .font_weight(gpui::FontWeight(500.0))
                    .text_color(rgb(text_color))
                    .child(title),
            )
            .when(unread > 0, |el| el.child(render_unread_badge(unread)))
            .child(
                div()
                    .id(close_id)
                    .size(px(16.0))
                    .rounded(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(colors::INK_4))
                    .opacity(0.0)
                    .group_hover(group_name, |style| style.opacity(1.0))
                    .hover(|style| style.bg(rgba(CLOSE_HOVER_BG)).text_color(rgb(colors::INK)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.close_tab_by_id(&close_tab_id, window, cx);
                        cx.stop_propagation();
                    }))
                    .child(IconName::Close),
            )
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
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)).text_color(rgb(colors::INK)))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.open_new_tab(window, cx);
            }))
            .child(
                div()
                    .text_color(rgb(colors::INK_4))
                    .child(IconName::Plus),
            )
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
        let bg_color = if active { ACTIVE_NAV_BG } else { 0x00000000 };
        let text_color = if active { colors::INK } else { colors::INK_2 };
        let group_name = SharedString::from(format!("tab-{}", tab.id().as_str()));
        let close_id = SharedString::from(format!("tab-close-{}", tab.id().as_str()));

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
            .hover(|style| style.bg(rgba(ACTIVE_NAV_BG)))
            .active(|style| style.opacity(0.82))
            .bg(rgba(bg_color))
            .when(active, |el| el.shadow(soft_shadow()))
            .on_click(cx.listener(move |shell, _, window, cx| {
                shell.select_tab(&tab_id, window, cx);
            }))
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
                            .font_weight(gpui::FontWeight(500.0))
                            .text_color(rgb(text_color))
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
            .child(
                div()
                    .id(close_id)
                    .size(px(16.0))
                    .rounded(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(colors::INK_4))
                    .opacity(0.0)
                    .group_hover(group_name, |style| style.opacity(1.0))
                    .hover(|style| style.bg(rgba(CLOSE_HOVER_BG)).text_color(rgb(colors::INK)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |shell, _, window, cx| {
                        shell.close_tab_by_id(&close_tab_id, window, cx);
                        cx.stop_propagation();
                    }))
                    .child(IconName::Close),
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

fn render_unread_badge(count: u32) -> impl IntoElement {
    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };

    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(999.0))
        .bg(rgba(UNREAD_BADGE_BG))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight(500.0))
        .text_color(rgb(colors::INK_3))
        .child(label)
}

fn section_tabs_label(count: usize) -> impl IntoElement {
    div()
        .pt(px(12.0))
        .pb(px(4.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_color(rgb(colors::INK_4))
                .child(IconName::Frame),
        )
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(gpui::FontWeight(500.0))
                .text_color(rgb(colors::INK_4))
                .child(format!("TABS · {count}")),
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

/// Hover tint behind the per-row close (×) button. Was 8% alpha, which
/// was visually indistinguishable from the panel background and made
/// the click target read as inert. Brought to ~30% alpha so the
/// hover registers as a real "press here" surface, matching the
/// confidence of close buttons in Arc/Dia/Zen.
const CLOSE_HOVER_BG: u32 = 0x281e144d;
const UNREAD_BADGE_BG: u32 = 0x281e140f;

/// 50% white inner border that traces every glass panel — the GPUI
/// substitute for the design's `box-shadow: inset 0 0 0 1px rgba(255,255,255,0.5)`.
/// Painted as the panel's own border so it stays part of the frame and
/// never participates in hit testing.
const HIGHLIGHT_BORDER: u32 = 0xffffff80;

/// Sidebar resize affordance straddling the panel's right edge. The
/// strip lives in the outer wrapper (not the rounded inner panel) so
/// it isn't swallowed by `overflow_hidden`; it spans 4 px outside the
/// panel and 4 px inside, giving the cursor a comfortably wide hit
/// surface that still reads as "edge of the panel" rather than
/// "stripe across the layout".
fn render_sidebar_resize_handle(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .id(SharedString::from("sidebar-resize-handle"))
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(-4.0))
        .w(px(8.0))
        .cursor_col_resize()
        .hover(|style| style.bg(rgba(RESIZE_HANDLE_HOVER_BG)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|shell, event: &gpui::MouseDownEvent, _, cx| {
                shell.begin_sidebar_resize(f32::from(event.position.x), cx);
            }),
        )
        .into_any_element()
}

const RESIZE_HANDLE_HOVER_BG: u32 = 0xffaa7733;

/// Maps appearance translucency_pct to a panel rgba u32 tinted by the
/// active wallpaper theme.
///
/// GPUI 0.2.2 has no backdrop-filter, so a true sample-and-blur is
/// impossible. Instead the panel base RGB is pre-tinted with the
/// wallpaper's character (warm cream for Dawn, lavender for Violet, etc.)
/// so the wallpaper's mood reads through every translucent panel without
/// any extra layer or shader. Combined with the user-controlled
/// translucency_pct it gives the design's "frosted glass tied to the
/// wallpaper" effect entirely from real GPUI primitives.
pub(crate) fn panel_bg(snapshot: &BrowserSnapshot) -> u32 {
    let pct = snapshot.appearance.translucency_pct().min(100) as u32;
    let max_alpha: u32 = 0xff;
    let min_alpha: u32 = 0xb3;
    let alpha = max_alpha - (pct * (max_alpha - min_alpha)) / 100;
    let rgb = wallpaper_panel_rgb(snapshot.appearance.wallpaper());
    (rgb << 8) | alpha
}

fn wallpaper_panel_rgb(theme: ely_domain::WallpaperTheme) -> u32 {
    match theme {
        ely_domain::WallpaperTheme::Dawn => 0xfaf6f0,
        ely_domain::WallpaperTheme::Violet => 0xf6f3f8,
        ely_domain::WallpaperTheme::Mint => 0xf3f6f1,
        ely_domain::WallpaperTheme::Slate => 0xeff1f4,
    }
}

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
