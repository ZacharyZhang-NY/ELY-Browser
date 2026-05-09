use gpui::{
    Hsla, IntoElement, ParentElement, Styled, div, hsla, linear_color_stop, linear_gradient, rgb,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WallpaperTheme {
    Dawn,
}

impl WallpaperTheme {
    fn base(self) -> u32 {
        match self {
            Self::Dawn => 0xefe8e1,
        }
    }

    fn upper_blob(self) -> Hsla {
        match self {
            Self::Dawn => hsla(351.0 / 360.0, 1.0, 0.91, 0.85),
        }
    }

    fn lower_blob(self) -> Hsla {
        match self {
            Self::Dawn => hsla(228.0 / 360.0, 1.0, 0.86, 0.65),
        }
    }
}

pub(crate) fn render_wallpaper(theme: WallpaperTheme) -> impl IntoElement {
    let upper = theme.upper_blob();
    let lower = theme.lower_blob();

    div()
        .absolute()
        .inset_0()
        .bg(rgb(theme.base()))
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(linear_gradient(
                    225.0,
                    linear_color_stop(upper, 0.0),
                    linear_color_stop(transparent_like(upper), 0.55),
                )),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(linear_gradient(
                    45.0,
                    linear_color_stop(lower, 0.0),
                    linear_color_stop(transparent_like(lower), 0.62),
                )),
        )
}

fn transparent_like(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.0)
}
