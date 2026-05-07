pub mod colors;
pub mod motion;
pub mod spacing;
pub mod typography;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub canvas: u32,
    pub canvas_soft: u32,
    pub surface: u32,
    pub ink: u32,
    pub body: u32,
    pub muted: u32,
    pub hairline: u32,
    pub accent: u32,
    pub success: u32,
    pub error: u32,
}

pub const ELY_THEME: Theme = Theme {
    canvas: colors::CANVAS,
    canvas_soft: colors::CANVAS_SOFT,
    surface: colors::SURFACE_CARD,
    ink: colors::INK,
    body: colors::BODY,
    muted: colors::MUTED,
    hairline: colors::HAIRLINE,
    accent: colors::PRIMARY,
    success: colors::SUCCESS,
    error: colors::ERROR,
};
