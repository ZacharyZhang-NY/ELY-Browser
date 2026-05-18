use std::{
    env,
    rc::Rc,
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

use dpi::PhysicalSize;
use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePixel,
    RenderingContext,
};

use super::SoftwareServoHost;
use crate::{RenderedFrame, ServoHostError};

const DEFAULT_PAINT_BARRIER_MS: u64 = 32;
const PAINT_BARRIER_POLL_INTERVAL: Duration = Duration::from_millis(2);

/// Wrap an `f32` scale factor in Servo's typed `Scale<f32, DeviceIndependentPixel,
/// DevicePixel>`. The clamp guards against `NaN`/`inf` reaching Servo's
/// layout (which assumes a positive finite scale).
pub(super) fn hidpi_scale_from_factor(
    scale_factor: f32,
) -> Scale<f32, DeviceIndependentPixel, DevicePixel> {
    let safe = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor.clamp(0.5, 5.0)
    } else {
        1.0
    };
    Scale::new(safe)
}

fn paint_barrier_budget() -> Duration {
    static BUDGET: OnceLock<Duration> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let ms = env::var("ELY_PAINT_BARRIER_MS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PAINT_BARRIER_MS);
        Duration::from_millis(ms)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServoSurfaceSize {
    width: u32,
    height: u32,
}

impl ServoSurfaceSize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width: width.max(1), height: height.max(1) }
    }

    pub(super) fn physical(self) -> PhysicalSize<u32> {
        PhysicalSize { width: self.width, height: self.height }
    }
}

/// Selects the `RenderingContext` implementation each webview gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderingContextKind {
    #[default]
    Software,
}

/// Pair of rendering-context handles produced by
/// [`SoftwareServoHost::new_rendering_context`].
pub(super) struct RenderingContextHandles {
    pub(super) rendering_context: Rc<dyn RenderingContext>,
}

impl SoftwareServoHost {
    pub(super) fn new_rendering_context(
        &self,
        size: ServoSurfaceSize,
    ) -> Result<RenderingContextHandles, ServoHostError> {
        match self.rendering_context_kind {
            RenderingContextKind::Software => {
                let rendering_context = Rc::new(
                    servo::SoftwareRenderingContext::new(size.physical())
                        .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
                );
                rendering_context
                    .make_current()
                    .map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
                Ok(RenderingContextHandles { rendering_context })
            }
        }
    }

    pub(super) fn new_rendering_context_for_native_surface<S>(
        &self,
        size: ServoSurfaceSize,
        native_surface: &S,
    ) -> Result<RenderingContextHandles, ServoHostError>
    where
        S: HasDisplayHandle + HasWindowHandle + ?Sized,
    {
        let display_handle = native_surface
            .display_handle()
            .map_err(|_| ServoHostError::RenderingContextUnavailable)?;
        let window_handle = native_surface
            .window_handle()
            .map_err(|_| ServoHostError::RenderingContextUnavailable)?;
        let rendering_context = Rc::new(
            servo::WindowRenderingContext::new(display_handle, window_handle, size.physical())
                .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
        );
        rendering_context.make_current().map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
        Ok(RenderingContextHandles { rendering_context })
    }

    /// Spin Servo's event loop until the webview's delegate observes a
    /// fresh `notify_new_frame_ready` callback (i.e. the framebuffer is
    /// consistent for readback) or [`paint_barrier_budget`] elapses. The
    /// caller is responsible for clearing the pending-frame flag before
    /// dispatching `webview.paint()`; otherwise this returns immediately
    /// off the *previous* frame and the race is preserved.
    ///
    /// Returns silently on timeout — `paint()` falls through to
    /// `read_rendered_frame` so callers still get whatever pixels the
    /// rendering context currently holds. That keeps the fast path open
    /// when `ELY_PAINT_BARRIER_MS=0` disables the budget entirely, and
    /// matches the pre-T15 behaviour on the (rare) case where Servo
    /// can't land a frame inside two refresh intervals.
    pub(super) fn wait_for_paint_completion(&mut self, webview_id: &ely_domain::WebViewId) {
        let budget = paint_barrier_budget();
        if budget.is_zero() {
            return;
        }
        let started_at = Instant::now();
        loop {
            self.servo.spin_event_loop();
            let ready = self
                .webviews
                .get(webview_id)
                .is_some_and(|webview| webview.delegate.has_pending_frame());
            if ready {
                return;
            }
            if started_at.elapsed() >= budget {
                return;
            }
            thread::sleep(PAINT_BARRIER_POLL_INTERVAL);
        }
    }

    pub(super) fn read_rendered_frame(
        rendering_context: &dyn RenderingContext,
    ) -> Result<RenderedFrame, ServoHostError> {
        let size = rendering_context.size();
        let width =
            i32::try_from(size.width).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let height =
            i32::try_from(size.height).map_err(|_| ServoHostError::RenderedFrameUnavailable)?;
        let frame_rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::new(0, 0),
            DeviceIntSize::new(width, height),
        );
        let image = rendering_context
            .read_to_image(frame_rect)
            .ok_or(ServoHostError::RenderedFrameUnavailable)?;

        Ok(RenderedFrame::from_rgba_bytes(size.width, size.height, image.into_raw()))
    }
}
