#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeToken {
    pub size_px: f32,
    pub line_height: f32,
    pub weight: u16,
}

pub const DISPLAY_MD: TypeToken = TypeToken { size_px: 26.0, line_height: 1.25, weight: 400 };

pub const TITLE_MD: TypeToken = TypeToken { size_px: 18.0, line_height: 1.4, weight: 600 };

pub const BODY_MD: TypeToken = TypeToken { size_px: 16.0, line_height: 1.5, weight: 400 };

pub const CAPTION: TypeToken = TypeToken { size_px: 13.0, line_height: 1.4, weight: 400 };
