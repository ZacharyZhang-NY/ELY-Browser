#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotionRegister {
    Productive,
    Expressive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionToken {
    pub register: MotionRegister,
    pub duration_ms: u16,
}

pub const HOVER_FEEDBACK: MotionToken =
    MotionToken { register: MotionRegister::Productive, duration_ms: 120 };

pub const PANEL_TRANSITION: MotionToken =
    MotionToken { register: MotionRegister::Productive, duration_ms: 180 };
