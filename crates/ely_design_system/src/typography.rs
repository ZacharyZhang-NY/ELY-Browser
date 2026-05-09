#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeToken {
    pub size_px: f32,
    pub line_height: f32,
    pub weight: u16,
}

pub const HERO: TypeToken = TypeToken { size_px: 64.0, line_height: 1.05, weight: 400 };
pub const DISPLAY_LG: TypeToken = TypeToken { size_px: 48.0, line_height: 1.05, weight: 400 };
pub const DISPLAY_MD: TypeToken = TypeToken { size_px: 34.0, line_height: 1.1, weight: 400 };
pub const TITLE_LG: TypeToken = TypeToken { size_px: 22.0, line_height: 1.3, weight: 400 };
pub const TITLE_MD: TypeToken = TypeToken { size_px: 18.0, line_height: 1.4, weight: 500 };
pub const BODY_LG: TypeToken = TypeToken { size_px: 16.0, line_height: 1.5, weight: 400 };
pub const BODY_MD: TypeToken = TypeToken { size_px: 14.0, line_height: 1.5, weight: 400 };
pub const LABEL: TypeToken = TypeToken { size_px: 13.0, line_height: 1.4, weight: 500 };
pub const CAPTION: TypeToken = TypeToken { size_px: 12.0, line_height: 1.4, weight: 500 };
pub const SECTION_LABEL: TypeToken = TypeToken { size_px: 10.5, line_height: 1.4, weight: 500 };
pub const META: TypeToken = TypeToken { size_px: 10.0, line_height: 1.4, weight: 500 };
