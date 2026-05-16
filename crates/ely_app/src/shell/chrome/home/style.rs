use ely_design_system::colors;
use gpui::{BoxShadow, hsla, point, px};

pub(crate) fn search_bg() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
pub(crate) fn arrow_chip_bg() -> u32 {
    colors::pick(0x281e140a, 0xf2efe90a)
}
pub(crate) fn pill_bg() -> u32 {
    colors::pick(0xffffff8c, 0x1f1d1b8c)
}
pub(crate) fn pill_bg_hover() -> u32 {
    colors::pick(0xffffffd9, 0x1f1d1bd9)
}
pub(crate) fn card_bg() -> u32 {
    colors::pick(0xffffffc7, 0x1f1d1bc7)
}
pub(crate) fn card_bg_hover() -> u32 {
    colors::pick(0xffffffeb, 0x1f1d1beb)
}
pub(crate) fn add_tile_bg() -> u32 {
    colors::pick(0xffffff7f, 0x1f1d1b7f)
}

pub(crate) fn card_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(25.0 / 360.0, 0.33, 0.12, 0.18),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(-10.0),
    }]
}

pub(crate) fn soft_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: hsla(0.0, 0.0, 1.0, 0.7),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
        },
        BoxShadow {
            color: hsla(25.0 / 360.0, 0.33, 0.12, 0.08),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
        },
    ]
}
