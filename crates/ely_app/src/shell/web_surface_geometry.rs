use gpui::{Bounds, Pixels, Point};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct WebSurfaceSize {
    pub(super) width: u32,
    pub(super) height: u32,
}

impl WebSurfaceSize {
    pub(super) fn from_bounds(bounds: Bounds<Pixels>, scale_factor: f32) -> Option<Self> {
        Some(Self {
            width: viewport_dimension(bounds.size.width, scale_factor)?,
            height: viewport_dimension(bounds.size.height, scale_factor)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceClickPoint {
    x: u32,
    y: u32,
}

impl WebSurfaceClickPoint {
    pub(super) fn from_window_position(
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        scale_factor: f32,
    ) -> Option<Self> {
        Some(Self {
            x: click_coordinate(position.x, bounds.origin.x, bounds.size.width, scale_factor)?,
            y: click_coordinate(position.y, bounds.origin.y, bounds.size.height, scale_factor)?,
        })
    }

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn detail_label(self) -> String {
        format!("click={},{}", self.x, self.y)
    }

    pub(super) fn x(self) -> u32 {
        self.x
    }

    pub(super) fn y(self) -> u32 {
        self.y
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

    #[cfg(all(test, feature = "live-site-smoke"))]
    pub(super) fn detail_label(self, size: WebSurfaceSize) -> String {
        match (self.x, self.y) {
            (0, 0) => format!("{}x{}", size.width, size.height),
            (0, y) => format!("{}x{} y={y}", size.width, size.height),
            (x, y) => format!("{}x{} x={x} y={y}", size.width, size.height),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
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
    pub(super) fn from_point(delta: Point<Pixels>, scale_factor: f32) -> Option<Self> {
        let x = scroll_dimension(delta.x, scale_factor)?;
        let y = scroll_dimension(delta.y, scale_factor)?;
        if x == 0 && y == 0 {
            return None;
        }

        Some(Self { x, y })
    }

    pub(super) fn x(self) -> i32 {
        self.x
    }

    pub(super) fn y(self) -> i32 {
        self.y
    }

    pub(super) fn combined_with(self, next: Self) -> Self {
        Self { x: combined_scroll_delta(self.x, next.x), y: combined_scroll_delta(self.y, next.y) }
    }
}

fn viewport_dimension(pixels: Pixels, scale_factor: f32) -> Option<u32> {
    let value = (f32::from(pixels) * positive_scale_or_one(scale_factor)).round();
    if !value.is_finite() || value < 1.0 || value > u32::MAX as f32 {
        return None;
    }

    Some(value as u32)
}

fn scroll_dimension(pixels: Pixels, scale_factor: f32) -> Option<i32> {
    let value = (f32::from(pixels) * positive_scale_or_one(scale_factor)).round();
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

fn click_coordinate(
    position: Pixels,
    origin: Pixels,
    size: Pixels,
    scale_factor: f32,
) -> Option<u32> {
    let relative = f32::from(position) - f32::from(origin);
    let size = f32::from(size);
    if !relative.is_finite() || !size.is_finite() || relative < 0.0 || relative >= size {
        return None;
    }

    let scaled = (relative * positive_scale_or_one(scale_factor)).floor();
    if !scaled.is_finite() || scaled < 0.0 || scaled > u32::MAX as f32 {
        return None;
    }

    Some(scaled as u32)
}

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    let clamped = value.clamp(0, i64::from(i32::MAX));
    clamped as i32
}

fn combined_scroll_delta(current: i32, next: i32) -> i32 {
    let value = i64::from(current) + i64::from(next);
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Guard against a zero/negative/NaN scale factor reaching the
/// arithmetic above. GPUI's `Window::scale_factor` returns the real
/// device pixel ratio (1.0 on standard displays, 2.0 on Retina), so a
/// non-positive value would mean the platform reported nonsense; we
/// fall back to 1.0 rather than collapsing every coordinate to zero.
fn positive_scale_or_one(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 { scale_factor } else { 1.0 }
}
