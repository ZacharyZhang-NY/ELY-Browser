use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, Styled, div, hsla, point, px, rgb, rgba,
};
use gpui_component::IconName;

use crate::shell::ElyShell;

/// Hover/active palette for the vertical sidebar rows.
///
/// The original code reused `ACTIVE_NAV_BG` for both `active` and
/// `hover`, which made an already-active row visually inert under the
/// cursor — the object refused to acknowledge the touch. The four
/// tokens below split that single value into a four-step ladder:
///
///   rest (transparent) → HOVER_NAV_BG → ACTIVE_NAV_BG → ACTIVE_NAV_BG_HOVER
///
/// so every state transition produces a real, perceptible change.
///
/// Values were picked by eye against the warm panel tint, then nudged
/// until the wash on a Dawn-themed panel reads as "you touched this"
/// without competing with the active selection's authority.
pub(crate) const HOVER_NAV_BG: u32 = 0xffffff66; // 40% white wash
pub(crate) const ACTIVE_NAV_BG: u32 = 0xffffffd9; // 85% white card
pub(crate) const ACTIVE_NAV_BG_HOVER: u32 = 0xfffffff2; // 95% white — active + hover

/// Hover tint behind the per-row close (×) button. Was 8% alpha, which
/// was visually indistinguishable from the panel background and made
/// the click target read as inert. Brought to ~30% alpha so the
/// hover registers as a real "press here" surface, matching the
/// confidence of close buttons in Arc/Dia/Zen.
pub(crate) const CLOSE_HOVER_BG: u32 = 0x281e144d;

pub(crate) const UNREAD_BADGE_BG: u32 = 0x281e140f;

/// 50% white inner border that traces every glass panel — the GPUI
/// substitute for the design's `box-shadow: inset 0 0 0 1px rgba(255,255,255,0.5)`.
/// Painted as the panel's own border so it stays part of the frame and
/// never participates in hit testing.
pub(crate) const HIGHLIGHT_BORDER: u32 = 0xffffff80;

pub(crate) const RESIZE_HANDLE_HOVER_BG: u32 = 0xffaa7733;

/// Diameter of the row-level close (×) button.
///
/// 16 px read as a glyph squeezed into a square; 18 px paired with
/// `rounded_full()` reads as a small physical coin you can press.
/// The extra two pixels also give the hover wash room to feel like
/// a halo around the icon instead of a tight collar.
pub(crate) const ROW_CLOSE_SIZE: f32 = 18.0;

pub(crate) fn profile_initial(name: &str) -> String {
    name.chars().next().unwrap_or('P').to_uppercase().to_string()
}

pub(crate) fn render_unread_badge(count: u32) -> impl IntoElement {
    let label = if count > 99 { "99+".to_string() } else { count.to_string() };

    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(999.0))
        .bg(rgba(UNREAD_BADGE_BG))
        .text_size(px(10.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_3()))
        .child(label)
}

pub(crate) fn section_tabs_label(count: usize) -> impl IntoElement {
    div()
        .pt(px(12.0))
        .pb(px(4.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().text_color(rgb(colors::ink_4())).child(IconName::Frame))
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight(500.0))
                .text_color(rgb(colors::ink_4()))
                .child(format!("TABS · {count}")),
        )
}

pub(crate) fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .pt(px(8.0))
        .pb(px(4.0))
        .px(px(10.0))
        .text_size(px(10.5))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_4()))
        .child(label)
}

/// Sidebar resize affordance straddling the panel's right edge. The
/// strip lives in the outer wrapper (not the rounded inner panel) so
/// it isn't swallowed by `overflow_hidden`; it spans 4 px outside the
/// panel and 4 px inside, giving the cursor a comfortably wide hit
/// surface that still reads as "edge of the panel" rather than
/// "stripe across the layout".
pub(crate) fn render_sidebar_resize_handle(cx: &mut Context<ElyShell>) -> AnyElement {
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
    let rgb = wallpaper_panel_rgb(snapshot.appearance.wallpaper(), colors::mode());
    (rgb << 8) | alpha
}

fn wallpaper_panel_rgb(theme: ely_domain::WallpaperTheme, mode: colors::Mode) -> u32 {
    match (theme, mode) {
        (ely_domain::WallpaperTheme::Dawn, colors::Mode::Light) => 0xfaf6f0,
        (ely_domain::WallpaperTheme::Violet, colors::Mode::Light) => 0xf6f3f8,
        (ely_domain::WallpaperTheme::Mint, colors::Mode::Light) => 0xf3f6f1,
        (ely_domain::WallpaperTheme::Slate, colors::Mode::Light) => 0xeff1f4,
        (ely_domain::WallpaperTheme::Dawn, colors::Mode::Dark) => 0x24211f,
        (ely_domain::WallpaperTheme::Violet, colors::Mode::Dark) => 0x232029,
        (ely_domain::WallpaperTheme::Mint, colors::Mode::Dark) => 0x1f2421,
        (ely_domain::WallpaperTheme::Slate, colors::Mode::Dark) => 0x22262e,
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
