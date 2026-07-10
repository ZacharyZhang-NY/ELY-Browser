use std::rc::Rc;

use dpi::PhysicalSize;
use euclid::Scale;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePixel,
    RenderingContext,
};

use super::SoftwareServoHost;
use crate::{RenderedFrame, ServoHostError};

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
    Hardware,
}

/// Pair of rendering-context handles produced by
/// [`SoftwareServoHost::new_rendering_context`].
pub(super) struct RenderingContextHandles {
    pub(super) rendering_context: Rc<dyn RenderingContext>,
    #[cfg(feature = "hardware-render")]
    pub(super) hardware_context: Option<Rc<crate::HardwareOffscreenContext>>,
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
                Ok(RenderingContextHandles {
                    rendering_context,
                    #[cfg(feature = "hardware-render")]
                    hardware_context: None,
                })
            }
            #[cfg(feature = "hardware-render")]
            RenderingContextKind::Hardware => {
                let hardware_context = Rc::new(
                    crate::HardwareOffscreenContext::new(size.physical())
                        .map_err(|_| ServoHostError::RenderingContextUnavailable)?,
                );
                hardware_context
                    .make_current()
                    .map_err(|_| ServoHostError::RenderingContextNotCurrent)?;
                Ok(RenderingContextHandles {
                    rendering_context: hardware_context.clone(),
                    hardware_context: Some(hardware_context),
                })
            }
            #[cfg(not(feature = "hardware-render"))]
            RenderingContextKind::Hardware => Err(ServoHostError::HardwareRenderUnavailable),
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
        Ok(RenderingContextHandles {
            rendering_context,
            #[cfg(feature = "hardware-render")]
            hardware_context: None,
        })
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
