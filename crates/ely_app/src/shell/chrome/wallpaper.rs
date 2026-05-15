use ely_design_system::colors;
use ely_domain::WallpaperTheme;
use gpui::{
    Hsla, IntoElement, ParentElement, Styled, div, hsla, linear_color_stop, linear_gradient, rgb,
};

pub(crate) fn render_wallpaper(theme: WallpaperTheme) -> impl IntoElement {
    let palette = palette_for(theme, colors::mode());

    div()
        .absolute()
        .inset_0()
        .bg(rgb(palette.base))
        .child(div().absolute().inset_0().bg(linear_gradient(
            225.0,
            linear_color_stop(palette.upper, 0.0),
            linear_color_stop(transparent_like(palette.upper), 0.55),
        )))
        .child(div().absolute().inset_0().bg(linear_gradient(
            45.0,
            linear_color_stop(palette.lower, 0.0),
            linear_color_stop(transparent_like(palette.lower), 0.62),
        )))
}

struct WallpaperPalette {
    base: u32,
    upper: Hsla,
    lower: Hsla,
}

fn palette_for(theme: WallpaperTheme, mode: colors::Mode) -> WallpaperPalette {
    match (theme, mode) {
        (WallpaperTheme::Dawn, colors::Mode::Light) => WallpaperPalette {
            base: 0xefe8e1,
            upper: hsla(351.0 / 360.0, 1.0, 0.91, 0.85),
            lower: hsla(228.0 / 360.0, 1.0, 0.86, 0.65),
        },
        (WallpaperTheme::Dawn, colors::Mode::Dark) => WallpaperPalette {
            base: 0x141312,
            upper: hsla(351.0 / 360.0, 0.45, 0.18, 0.85),
            lower: hsla(228.0 / 360.0, 0.45, 0.16, 0.65),
        },
        (WallpaperTheme::Violet, colors::Mode::Light) => WallpaperPalette {
            base: 0xe9e2ee,
            upper: hsla(263.0 / 360.0, 1.0, 0.88, 0.78),
            lower: hsla(33.0 / 360.0, 1.0, 0.87, 0.60),
        },
        (WallpaperTheme::Violet, colors::Mode::Dark) => WallpaperPalette {
            base: 0x161320,
            upper: hsla(263.0 / 360.0, 0.55, 0.20, 0.85),
            lower: hsla(33.0 / 360.0, 0.50, 0.18, 0.65),
        },
        (WallpaperTheme::Mint, colors::Mode::Light) => WallpaperPalette {
            base: 0xe6ece2,
            upper: hsla(146.0 / 360.0, 0.69, 0.85, 0.82),
            lower: hsla(40.0 / 360.0, 1.0, 0.86, 0.65),
        },
        (WallpaperTheme::Mint, colors::Mode::Dark) => WallpaperPalette {
            base: 0x121814,
            upper: hsla(146.0 / 360.0, 0.45, 0.18, 0.82),
            lower: hsla(40.0 / 360.0, 0.45, 0.16, 0.65),
        },
        (WallpaperTheme::Slate, colors::Mode::Light) => WallpaperPalette {
            base: 0xd6dae3,
            upper: hsla(218.0 / 360.0, 0.20, 0.85, 0.75),
            lower: hsla(20.0 / 360.0, 0.0, 0.10, 0.18),
        },
        (WallpaperTheme::Slate, colors::Mode::Dark) => WallpaperPalette {
            base: 0x141620,
            upper: hsla(218.0 / 360.0, 0.30, 0.20, 0.80),
            lower: hsla(20.0 / 360.0, 0.0, 0.12, 0.55),
        },
    }
}

fn transparent_like(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.0)
}
