use gpui::{Bounds, Pixels, Point};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceSize {
    pub(super) width: u32,
    pub(super) height: u32,
}

impl WebSurfaceSize {
    pub(super) fn from_bounds(bounds: Bounds<Pixels>) -> Option<Self> {
        Some(Self {
            width: viewport_dimension(bounds.size.width)?,
            height: viewport_dimension(bounds.size.height)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct WebSurfaceScrollOffset {
    x: i32,
    y: i32,
}

impl WebSurfaceScrollOffset {
    pub(super) fn scrolled_by(self, delta: WebSurfaceScrollDelta) -> Self {
        Self {
            x: positive_scroll_component(self.x, delta.x),
            y: positive_scroll_component(self.y, delta.y),
        }
    }

    pub(super) fn detail_label(self, size: WebSurfaceSize) -> String {
        match (self.x, self.y) {
            (0, 0) => format!("{}x{}", size.width, size.height),
            (0, y) => format!("{}x{} y={y}", size.width, size.height),
            (x, y) => format!("{}x{} x={x} y={y}", size.width, size.height),
        }
    }

    pub(super) fn x(self) -> i32 {
        self.x
    }

    pub(super) fn y(self) -> i32 {
        self.y
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceScrollDelta {
    x: i32,
    y: i32,
}

impl WebSurfaceScrollDelta {
    pub(super) fn from_point(delta: Point<Pixels>) -> Option<Self> {
        let x = scroll_dimension(delta.x)?;
        let y = scroll_dimension(delta.y)?;
        if x == 0 && y == 0 {
            return None;
        }

        Some(Self { x, y })
    }
}

fn viewport_dimension(pixels: Pixels) -> Option<u32> {
    let value = f32::from(pixels.round());
    if !value.is_finite() || value < 1.0 || value > u32::MAX as f32 {
        return None;
    }

    Some(value as u32)
}

fn scroll_dimension(pixels: Pixels) -> Option<i32> {
    let value = f32::from(pixels.round());
    if !value.is_finite() {
        return None;
    }
    if value > i32::MAX as f32 {
        return Some(i32::MAX);
    }
    if value < i32::MIN as f32 {
        return Some(i32::MIN);
    }

    Some(value as i32)
}

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    let clamped = value.clamp(0, i64::from(i32::MAX));
    clamped as i32
}
