//! Headless hardware rendering context backed by Surfman.
//!
//! Servo keeps its Surfman constructor private. This module mirrors the small
//! portion required to create a hardware `SurfaceType::Generic` context and,
//! on macOS, retain the presented IOSurface for cross-process import.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use euclid::Size2D;
use gleam::gl::{self, Gl};
use image::RgbaImage;
use servo::{DeviceIntRect, RenderingContext};
#[cfg(target_os = "macos")]
use surfman::cgl::surface::NativeSurface;
use surfman::chains::{PreserveBuffer, SwapChain, SwapChainAPI};
use surfman::{
    Connection, Context, ContextAttributeFlags, ContextAttributes, Device, Error as SurfmanError,
    GLApi, NativeWidget, Surface, SurfaceAccess, SurfaceType,
};

#[cfg(target_os = "macos")]
use crate::{IOSurfaceHandle, IOSurfaceIdentity};

/// A hardware-backed offscreen Servo rendering context.
pub struct HardwareOffscreenContext {
    size: Cell<PhysicalSize<u32>>,
    inner: SurfmanInner,
    swap_chain: SwapChain<Device>,
    #[cfg(target_os = "macos")]
    held_presented_surfaces: RefCell<Vec<HeldPresentedSurface>>,
    #[cfg(target_os = "macos")]
    last_presented_iosurface: RefCell<Option<PresentedIOSurface>>,
}

impl HardwareOffscreenContext {
    /// Creates a hardware context with a generic offscreen surface.
    pub fn new(size: PhysicalSize<u32>) -> Result<Self, SurfmanError> {
        if size.width == 0 || size.height == 0 {
            return Err(SurfmanError::Failed);
        }

        let connection = Connection::new()?;
        let adapter = connection.create_adapter()?;
        let inner = SurfmanInner::new(&connection, &adapter)?;
        let surface = inner.create_surface(SurfaceType::Generic {
            size: Size2D::new(size.width as i32, size.height as i32),
        })?;
        inner.bind_surface(surface)?;
        inner.make_current()?;
        let swap_chain = inner.create_attached_swap_chain()?;

        Ok(Self {
            size: Cell::new(size),
            inner,
            swap_chain,
            #[cfg(target_os = "macos")]
            held_presented_surfaces: RefCell::new(Vec::new()),
            #[cfg(target_os = "macos")]
            last_presented_iosurface: RefCell::new(None),
        })
    }
}

impl Drop for HardwareOffscreenContext {
    fn drop(&mut self) {
        let device = self.inner.device.borrow();
        let context = &mut self.inner.context.borrow_mut();
        #[cfg(target_os = "macos")]
        self.destroy_held_presented_surfaces(&device, context);
        let _ = self.swap_chain.destroy(&device, context);
    }
}

impl RenderingContext for HardwareOffscreenContext {
    fn prepare_for_rendering(&self) {
        self.inner.prepare_for_rendering();
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.inner.read_to_image(source_rectangle)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if self.size.get() == size || size.width == 0 || size.height == 0 {
            return;
        }

        let device = self.inner.device.borrow();
        let context = &mut self.inner.context.borrow_mut();
        #[cfg(target_os = "macos")]
        self.destroy_held_presented_surfaces(&device, context);
        let surfman_size = Size2D::new(size.width as i32, size.height as i32);
        if self.swap_chain.resize(&device, context, surfman_size).is_ok() {
            self.size.set(size);
        }
    }

    fn present(&self) {
        let device = self.inner.device.borrow();
        let context = &mut self.inner.context.borrow_mut();
        #[cfg(target_os = "macos")]
        self.recycle_acknowledged_surfaces();
        #[cfg(target_os = "macos")]
        self.last_presented_iosurface.borrow_mut().take();
        if self.swap_chain.swap_buffers(&device, context, PreserveBuffer::No).is_err() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            self.capture_presented_iosurface(&device);
            self.recycle_acknowledged_surfaces();
        }
    }

    fn make_current(&self) -> Result<(), SurfmanError> {
        self.inner.make_current()
    }

    fn gleam_gl_api(&self) -> Rc<dyn Gl> {
        self.inner.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.inner.glow_gl.clone()
    }

    fn connection(&self) -> Option<Connection> {
        Some(self.inner.device.borrow().connection())
    }
}

#[cfg(target_os = "macos")]
impl HardwareOffscreenContext {
    /// Returns the identity of the most recently presented IOSurface.
    pub fn peek_iosurface_identity(&self) -> Result<Option<IOSurfaceIdentity>, SurfmanError> {
        Ok(self.last_presented_iosurface.borrow().as_ref().map(|surface| surface.identity))
    }

    /// Creates a Mach send right for the most recently presented IOSurface.
    pub fn current_iosurface_mach_port(&self) -> Result<IOSurfaceHandle, SurfmanError> {
        let presented = self.last_presented_iosurface.borrow();
        let presented = presented.as_ref().ok_or(SurfmanError::Failed)?;
        let mach_port_name = presented.native.0.create_mach_port();
        if mach_port_name == 0 {
            return Err(SurfmanError::Failed);
        }

        Ok(IOSurfaceHandle {
            mach_port_name,
            surface_id: presented.identity.surface_id,
            width: presented.identity.width,
            height: presented.identity.height,
        })
    }

    /// Marks IOSurface IDs reported ready after import by the app process.
    ///
    /// The ready acknowledgement confirms import. Acknowledged surfaces remain
    /// retained while current or while `IOSurfaceIsInUse` reports active
    /// consumer work. Recycling begins after that consumer work completes.
    pub fn acknowledge_iosurfaces(&self, surface_ids: &[u64]) {
        {
            let mut held = self.held_presented_surfaces.borrow_mut();
            for surface in held.iter_mut() {
                if surface_ids.contains(&surface.presented.identity.surface_id) {
                    surface.acknowledged = true;
                }
            }
        }
        self.recycle_acknowledged_surfaces();
    }

    fn capture_presented_iosurface(&self, device: &Device) {
        let Some(surface) = self.swap_chain.take_pending_surface() else {
            self.last_presented_iosurface.borrow_mut().take();
            return;
        };
        let info = device.surface_info(&surface);
        let native = device.native_surface(&surface);
        let identity = IOSurfaceIdentity {
            surface_id: u64::from(native.0.id()),
            width: u32::try_from(info.size.width).unwrap_or(0),
            height: u32::try_from(info.size.height).unwrap_or(0),
        };
        let presented = PresentedIOSurface { identity, native };
        self.held_presented_surfaces.borrow_mut().push(HeldPresentedSurface {
            surface,
            presented: presented.clone(),
            acknowledged: false,
        });
        self.last_presented_iosurface.replace(Some(presented));
    }

    fn recycle_acknowledged_surfaces(&self) {
        let current_id = self
            .last_presented_iosurface
            .borrow()
            .as_ref()
            .map(|surface| surface.identity.surface_id);
        let mut held = self.held_presented_surfaces.borrow_mut();
        let mut index = 0;
        while index < held.len() {
            let surface = &held[index];
            let can_recycle = surface.acknowledged
                && Some(surface.presented.identity.surface_id) != current_id
                && !surface.presented.native.0.is_in_use();
            if can_recycle {
                let surface = held.swap_remove(index);
                self.swap_chain.recycle_surface(surface.surface);
            } else {
                index += 1;
            }
        }
    }

    fn destroy_held_presented_surfaces(&self, device: &Device, context: &mut Context) {
        self.last_presented_iosurface.borrow_mut().take();
        let held = self.held_presented_surfaces.take();
        for mut surface in held {
            let _ = device.destroy_surface(context, &mut surface.surface);
        }
    }
}

#[cfg(target_os = "macos")]
struct HeldPresentedSurface {
    surface: Surface,
    presented: PresentedIOSurface,
    acknowledged: bool,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct PresentedIOSurface {
    identity: IOSurfaceIdentity,
    native: NativeSurface,
}

struct SurfmanInner {
    gleam_gl: Rc<dyn Gl>,
    glow_gl: Arc<glow::Context>,
    device: RefCell<Device>,
    context: RefCell<Context>,
}

impl Drop for SurfmanInner {
    fn drop(&mut self) {
        let device = self.device.borrow();
        let context = &mut self.context.borrow_mut();
        let _ = device.destroy_context(context);
    }
}

impl SurfmanInner {
    fn new(connection: &Connection, adapter: &surfman::Adapter) -> Result<Self, SurfmanError> {
        let device = connection.create_device(adapter)?;
        let flags = ContextAttributeFlags::ALPHA
            | ContextAttributeFlags::DEPTH
            | ContextAttributeFlags::STENCIL;
        let gl_api = connection.gl_api();
        let version = match gl_api {
            GLApi::GLES => surfman::GLVersion { major: 3, minor: 0 },
            GLApi::GL => surfman::GLVersion { major: 3, minor: 2 },
        };
        let descriptor = device.create_context_descriptor(&ContextAttributes { flags, version })?;
        let context = device.create_context(&descriptor, None)?;

        // Surfman owns the current platform GL implementation and supplies matching ABI symbols.
        #[expect(unsafe_code)]
        let gleam_gl = match gl_api {
            GLApi::GL => unsafe {
                gl::GlFns::load_with(|name| device.get_proc_address(&context, name))
            },
            GLApi::GLES => unsafe {
                gl::GlesFns::load_with(|name| device.get_proc_address(&context, name))
            },
        };

        // The loader remains valid for the lifetime of the Surfman device and context below.
        #[expect(unsafe_code)]
        let glow_gl = unsafe {
            glow::Context::from_loader_function(|name| device.get_proc_address(&context, name))
        };

        Ok(Self {
            gleam_gl,
            glow_gl: Arc::new(glow_gl),
            device: RefCell::new(device),
            context: RefCell::new(context),
        })
    }

    fn create_surface(
        &self,
        surface_type: SurfaceType<NativeWidget>,
    ) -> Result<Surface, SurfmanError> {
        self.device.borrow().create_surface(
            &self.context.borrow(),
            SurfaceAccess::GPUOnly,
            surface_type,
        )
    }

    fn bind_surface(&self, surface: Surface) -> Result<(), SurfmanError> {
        let device = self.device.borrow();
        let context = &mut self.context.borrow_mut();
        device.bind_surface_to_context(context, surface).map_err(|(error, mut surface)| {
            let _ = device.destroy_surface(context, &mut surface);
            error
        })
    }

    fn create_attached_swap_chain(&self) -> Result<SwapChain<Device>, SurfmanError> {
        SwapChain::create_attached(
            &self.device.borrow(),
            &mut self.context.borrow_mut(),
            SurfaceAccess::GPUOnly,
        )
    }

    fn make_current(&self) -> Result<(), SurfmanError> {
        self.device.borrow().make_context_current(&self.context.borrow())
    }

    fn framebuffer_id(&self) -> u32 {
        self.device
            .borrow()
            .context_surface_info(&self.context.borrow())
            .unwrap_or(None)
            .and_then(|info| info.framebuffer_object)
            .map_or(0, |framebuffer| framebuffer.0.into())
    }

    fn prepare_for_rendering(&self) {
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id());
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id());
        self.gleam_gl.bind_vertex_array(0);

        let mut pixels = self.gleam_gl.read_pixels(
            source_rectangle.min.x,
            source_rectangle.min.y,
            source_rectangle.width(),
            source_rectangle.height(),
            gl::RGBA,
            gl::UNSIGNED_BYTE,
        );
        if self.gleam_gl.get_error() != gl::NO_ERROR {
            return None;
        }

        let rectangle = source_rectangle.to_usize();
        let stride = rectangle.width().checked_mul(4)?;
        let original = pixels.clone();
        for y in 0..rectangle.height() {
            let destination_start = y.checked_mul(stride)?;
            let source_start = rectangle.height().checked_sub(y + 1)?.checked_mul(stride)?;
            let destination_end = destination_start.checked_add(stride)?;
            let source_end = source_start.checked_add(stride)?;
            pixels
                .get_mut(destination_start..destination_end)?
                .copy_from_slice(original.get(source_start..source_end)?);
        }

        RgbaImage::from_raw(rectangle.width() as u32, rectangle.height() as u32, pixels)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn present(context: &HardwareOffscreenContext) -> Result<IOSurfaceIdentity, String> {
        context.make_current().map_err(|error| format!("make current failed: {error:?}"))?;
        context.prepare_for_rendering();
        context.present();
        context
            .peek_iosurface_identity()
            .map_err(|error| format!("identity probe failed: {error:?}"))?
            .ok_or_else(|| "present did not expose an IOSurface".to_string())
    }

    fn held_ids(context: &HardwareOffscreenContext) -> Vec<u64> {
        context
            .held_presented_surfaces
            .borrow()
            .iter()
            .map(|surface| surface.presented.identity.surface_id)
            .collect()
    }

    #[test]
    fn acknowledgement_recycles_only_noncurrent_surfaces() -> Result<(), String> {
        let context = HardwareOffscreenContext::new(PhysicalSize::new(64, 48))
            .map_err(|error| format!("hardware context creation failed: {error:?}"))?;
        let first = present(&context)?;
        let second = present(&context)?;

        assert_ne!(first.surface_id, second.surface_id);
        assert_eq!(held_ids(&context).len(), 2);

        context.acknowledge_iosurfaces(&[first.surface_id]);
        assert_eq!(held_ids(&context), vec![second.surface_id]);

        context.acknowledge_iosurfaces(&[second.surface_id]);
        assert_eq!(held_ids(&context), vec![second.surface_id]);
        Ok(())
    }

    #[test]
    fn acknowledged_surface_waits_for_iosurface_use_to_finish() -> Result<(), String> {
        let context = HardwareOffscreenContext::new(PhysicalSize::new(64, 48))
            .map_err(|error| format!("hardware context creation failed: {error:?}"))?;
        let first = present(&context)?;
        let second = present(&context)?;
        let native = context
            .held_presented_surfaces
            .borrow()
            .iter()
            .find(|surface| surface.presented.identity == first)
            .map(|surface| surface.presented.native.0.clone())
            .ok_or_else(|| "first IOSurface was not retained".to_string())?;

        native.increment_use_count();
        context.acknowledge_iosurfaces(&[first.surface_id]);
        let retained_while_in_use = held_ids(&context).contains(&first.surface_id);
        native.decrement_use_count();

        assert!(retained_while_in_use);
        context.acknowledge_iosurfaces(&[]);
        assert_eq!(held_ids(&context), vec![second.surface_id]);
        Ok(())
    }

    #[test]
    fn resize_destroys_all_retained_presentations() -> Result<(), String> {
        let context = HardwareOffscreenContext::new(PhysicalSize::new(64, 48))
            .map_err(|error| format!("hardware context creation failed: {error:?}"))?;
        let _ = present(&context)?;
        let _ = present(&context)?;
        assert_eq!(held_ids(&context).len(), 2);

        context.resize(PhysicalSize::new(96, 72));

        assert!(held_ids(&context).is_empty());
        assert_eq!(context.size(), PhysicalSize::new(96, 72));
        assert_eq!(
            context
                .peek_iosurface_identity()
                .map_err(|error| format!("identity probe failed: {error:?}"))?,
            None
        );
        let resized = present(&context)?;
        assert_eq!((resized.width, resized.height), (96, 72));
        Ok(())
    }
}
