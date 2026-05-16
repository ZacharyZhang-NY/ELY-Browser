use ely_browser_core::BrowserSnapshot;
use ely_design_system::colors;
use ely_domain::{ThemeMode, WallpaperTheme};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, hsla, linear_color_stop,
    linear_gradient, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::{IconName, scroll::ScrollableElement, slider::Slider};

use crate::shell::ElyShell;
use crate::shell::chrome::SERIF_FAMILY;

pub(crate) fn render_appearance_form(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .overflow_y_scrollbar()
        .pt(px(28.0))
        .px(px(40.0))
        .pb(px(32.0))
        .child(
            div()
                .max_w(px(780.0))
                .mx_auto()
                .flex()
                .flex_col()
                .gap(px(28.0))
                .child(render_header())
                .child(render_wallpaper_grid(snapshot, cx))
                .child(render_appearance_rows(shell, snapshot, cx))
                .child(render_translucency_presets(cx)),
        )
        .into_any_element()
}

fn render_header() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(div().text_size(px(11.0)).text_color(rgb(colors::ink_4())).child("GENERAL"))
        .child(
            div()
                .font_family(SERIF_FAMILY)
                .text_size(px(34.0))
                .font_weight(FontWeight(400.0))
                .text_color(rgb(colors::ink()))
                .child("Appearance"),
        )
        .child(div().max_w(px(520.0)).text_size(px(13.0)).text_color(rgb(colors::ink_3())).child(
            "Tune the atmosphere of your browser. ELY's wallpaper sets the ambient \
                     palette of every surface; pick one and let it breathe.",
        ))
        .into_any_element()
}

fn render_wallpaper_grid(snapshot: &BrowserSnapshot, cx: &mut Context<ElyShell>) -> AnyElement {
    let active = snapshot.appearance.wallpaper();
    div()
        .grid()
        .grid_cols(4)
        .gap(px(10.0))
        .child(render_wallpaper_swatch(WallpaperTheme::Dawn, active, cx))
        .child(render_wallpaper_swatch(WallpaperTheme::Violet, active, cx))
        .child(render_wallpaper_swatch(WallpaperTheme::Mint, active, cx))
        .child(render_wallpaper_swatch(WallpaperTheme::Slate, active, cx))
        .into_any_element()
}

fn render_wallpaper_swatch(
    theme: WallpaperTheme,
    active: WallpaperTheme,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let label = wallpaper_label(theme);
    let id = SharedString::from(format!("wallpaper-{}", label.to_ascii_lowercase()));
    let selected = theme == active;
    let (upper, lower, base) = swatch_colors(theme);

    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|style| style.opacity(0.94))
        .active(|style| style.opacity(0.85))
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_wallpaper_theme(theme, cx);
        }))
        .child(
            div()
                .h(px(96.0))
                .rounded(px(10.0))
                .when(selected, |el| el.border_2().border_color(rgb(colors::accent())))
                .when(!selected, |el| el.border_1().border_color(rgba(colors::stroke())))
                .relative()
                .overflow_hidden()
                .bg(rgb(base))
                .child(div().absolute().inset_0().bg(linear_gradient(
                    225.0,
                    linear_color_stop(upper, 0.0),
                    linear_color_stop(transparent_like(upper), 0.6),
                )))
                .child(div().absolute().inset_0().bg(linear_gradient(
                    45.0,
                    linear_color_stop(lower, 0.0),
                    linear_color_stop(transparent_like(lower), 0.6),
                ))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child(label),
                )
                .when(selected, |el| {
                    el.child(div().text_color(rgb(colors::accent())).child(IconName::Check))
                }),
        )
        .into_any_element()
}

fn render_appearance_rows(
    shell: &mut ElyShell,
    snapshot: &BrowserSnapshot,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let appearance = snapshot.appearance;

    div()
        .flex()
        .flex_col()
        .child(render_theme_mode_row(appearance.theme_mode(), cx))
        .child(render_accent_row())
        .child(render_translucency_row(shell, appearance.translucency_pct()))
        .child(render_reduce_motion_row(appearance.reduce_motion(), cx))
        .child(render_reset_row(cx))
        .into_any_element()
}

fn render_translucency_row(shell: &mut ElyShell, pct: u8) -> AnyElement {
    let slider_state = shell.translucency_slider.clone();
    settings_row(
        "Translucency",
        "Glass density of sidebar & panels.",
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(div().w(px(160.0)).child(Slider::new(&slider_state)))
            .child(
                div()
                    .min_w(px(36.0))
                    .text_size(px(11.0))
                    .text_color(rgb(colors::ink_3()))
                    .child(format!("{pct}%")),
            )
            .into_any_element(),
    )
}

fn render_translucency_presets(cx: &mut Context<ElyShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .pt(px(2.0))
        .child(div().text_size(px(10.5)).text_color(rgb(colors::ink_4())).child("Presets"))
        .child(translucency_preset("Solid", 0, cx))
        .child(translucency_preset("Default", 40, cx))
        .child(translucency_preset("Glassy", 75, cx))
        .into_any_element()
}

fn translucency_preset(
    label: &'static str,
    target_pct: u8,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let id = SharedString::from(format!("translucency-{}", label.to_ascii_lowercase()));

    div()
        .id(id)
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .bg(rgba(segment_bg()))
        .text_size(px(11.5))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_2()))
        .cursor_pointer()
        .hover(|style| style.opacity(0.92))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.set_translucency_pct_from_preset(target_pct, window, cx);
        }))
        .child(label)
        .into_any_element()
}

fn render_theme_mode_row(active: ThemeMode, cx: &mut Context<ElyShell>) -> AnyElement {
    settings_row(
        "Theme mode",
        "Match system appearance, or pick one to lock.",
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded(px(8.0))
            .bg(rgba(segment_bg()))
            .child(theme_segment("System", ThemeMode::System, active, cx))
            .child(theme_segment("Light", ThemeMode::Light, active, cx))
            .child(theme_segment("Dark", ThemeMode::Dark, active, cx))
            .into_any_element(),
    )
}

fn theme_segment(
    label: &'static str,
    mode: ThemeMode,
    active: ThemeMode,
    cx: &mut Context<ElyShell>,
) -> AnyElement {
    let selected = mode == active;
    let bg = if selected { 0xffffffff } else { 0x00000000 };
    let id = SharedString::from(format!("theme-mode-{}", label.to_ascii_lowercase()));

    div()
        .id(id)
        .px(px(12.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .text_size(px(12.0))
        .font_weight(FontWeight(500.0))
        .text_color(rgb(colors::ink_2()))
        .bg(rgba(bg))
        .cursor_pointer()
        .hover(|style| style.opacity(0.92))
        .active(|style| style.opacity(0.82))
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_theme_mode(mode, cx);
        }))
        .child(label)
        .into_any_element()
}

fn render_accent_row() -> AnyElement {
    settings_row(
        "Accent color",
        "Used across selection, focus rings & links.",
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(swatch(colors::accent(), true))
            .child(swatch(0xec6a8e, false))
            .child(swatch(0x7c6cf7, false))
            .child(swatch(0x4fb59a, false))
            .child(swatch(colors::ink(), false))
            .into_any_element(),
    )
}

fn swatch(color: u32, selected: bool) -> AnyElement {
    let mut element = div()
        .size(px(22.0))
        .rounded_full()
        .bg(rgb(color))
        .border_1()
        .border_color(rgba(0x0000000d));
    if selected {
        element = element.border_2().border_color(rgb(colors::accent()));
    }
    element.into_any_element()
}

fn render_reduce_motion_row(reduce: bool, cx: &mut Context<ElyShell>) -> AnyElement {
    settings_row(
        "Reduce motion",
        "Mute transitions and ambient effects.",
        div()
            .id(SharedString::from("toggle-reduce-motion"))
            .w(px(34.0))
            .h(px(20.0))
            .rounded_full()
            .bg(if reduce { rgb(colors::accent()) } else { rgba(0x281e1426) })
            .p(px(2.0))
            .cursor_pointer()
            .hover(|style| style.opacity(0.9))
            .active(|style| style.opacity(0.78))
            .on_click(cx.listener(|shell, _, _, cx| shell.toggle_reduce_motion(cx)))
            .child(
                div()
                    .size(px(16.0))
                    .rounded_full()
                    .bg(rgb(0xffffff))
                    .when(reduce, |el| el.ml(px(14.0))),
            )
            .into_any_element(),
    )
}

fn render_reset_row(cx: &mut Context<ElyShell>) -> AnyElement {
    settings_row(
        "Restore defaults",
        "Reset wallpaper, theme mode, and motion to defaults.",
        div()
            .id(SharedString::from("appearance-reset"))
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(8.0))
            .bg(rgba(segment_bg()))
            .text_size(px(12.0))
            .font_weight(FontWeight(500.0))
            .text_color(rgb(colors::ink_2()))
            .cursor_pointer()
            .hover(|style| style.opacity(0.92))
            .active(|style| style.opacity(0.82))
            .on_click(cx.listener(|shell, _, window, cx| {
                shell.reset_appearance(window, cx);
            }))
            .child("Reset")
            .into_any_element(),
    )
}

fn settings_row(title: &'static str, detail: &'static str, control: AnyElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.0))
        .py(px(14.0))
        .border_b_1()
        .border_color(rgba(colors::divider()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight(500.0))
                        .text_color(rgb(colors::ink()))
                        .child(title),
                )
                .child(div().text_size(px(11.5)).text_color(rgb(colors::ink_3())).child(detail)),
        )
        .child(control)
        .into_any_element()
}

fn wallpaper_label(theme: WallpaperTheme) -> &'static str {
    match theme {
        WallpaperTheme::Dawn => "Dawn",
        WallpaperTheme::Violet => "Violet",
        WallpaperTheme::Mint => "Mint",
        WallpaperTheme::Slate => "Slate",
    }
}

fn swatch_colors(theme: WallpaperTheme) -> (Hsla, Hsla, u32) {
    match theme {
        WallpaperTheme::Dawn => {
            (hsla(351.0 / 360.0, 1.0, 0.91, 0.95), hsla(228.0 / 360.0, 1.0, 0.86, 0.85), 0xefe8e1)
        }
        WallpaperTheme::Violet => {
            (hsla(263.0 / 360.0, 1.0, 0.88, 0.95), hsla(33.0 / 360.0, 1.0, 0.87, 0.85), 0xe9e2ee)
        }
        WallpaperTheme::Mint => {
            (hsla(146.0 / 360.0, 0.69, 0.85, 0.95), hsla(40.0 / 360.0, 1.0, 0.86, 0.85), 0xe6ece2)
        }
        WallpaperTheme::Slate => {
            (hsla(218.0 / 360.0, 0.20, 0.85, 0.95), hsla(20.0 / 360.0, 0.0, 0.10, 0.40), 0xd6dae3)
        }
    }
}

fn transparent_like(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.0)
}

fn segment_bg() -> u32 {
    colors::pick(0x281e140d, 0xf2efe90d)
}
