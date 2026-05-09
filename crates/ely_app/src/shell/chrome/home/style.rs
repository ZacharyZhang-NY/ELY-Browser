use gpui::{BoxShadow, hsla, point, px};

pub(crate) const SEARCH_BG: u32 = 0xffffffd9;
pub(crate) const ARROW_CHIP_BG: u32 = 0x281e140a;
pub(crate) const PILL_BG: u32 = 0xffffff8c;
pub(crate) const PILL_BG_HOVER: u32 = 0xffffffd9;
pub(crate) const CARD_BG: u32 = 0xffffffc7;
pub(crate) const CARD_BG_HOVER: u32 = 0xffffffeb;
pub(crate) const ADD_TILE_BG: u32 = 0xffffff7f;

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
