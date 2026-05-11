//! Headless hardware [`RenderingContext`] for Servo, vendored from
//! `servo-paint-api`'s private `SurfmanRenderingContext` and reshaped
//! so it can be constructed without a `RawWindowHandle`.
//!
//! Why this file exists: `servo-paint-api 0.1` exposes three
//! constructors — `SoftwareRenderingContext` (CPU-only),
//! `WindowRenderingContext` (requires `DisplayHandle + WindowHandle`),
//! and `OffscreenRenderingContext` (must be a child of a
//! `WindowRenderingContext`). The sidecar process has no window, so
//! none of the three works for us when we want **hardware**
//! rasterising. The underlying `SurfmanRenderingContext` glue *can*
//! drive a hardware adapter against a `SurfaceType::Generic`
//! offscreen surface — that's exactly what we need — but its
//! constructor is `fn new` (private). Until Servo accepts an upstream
//! PR exposing a headless hardware constructor, this file vendors the
//! minimal slice of glue we need.
//!
//! Scope kept deliberately narrow:
//!
//!   * `prepare_for_rendering`, `read_to_image`, `size`, `resize`,
//!     `present`, `make_current`, `gleam_gl_api`, `glow_gl_api`, and
//!     `connection` are vendored. `connection` is mandatory:
//!     `servo-paint`'s painter calls `rendering_context.connection()
//!     .expect("Failed to get connection")` while constructing its
//!     painter, so a `None` default panics the compositor before the
//!     first frame is ever painted.
//!   * `create_texture`/`destroy_texture` still fall through to the
//!     trait defaults — Servo only reaches for them when sharing
//!     surfman surfaces with its compositor for WebGL/WebGPU, which
//!     this readback path does not exercise.
//!   * No `RefreshDriver`. The sidecar drives its own polling loop.
//!   * The reading path inlines `read_framebuffer_to_image` from the
//!     same upstream file so we don't take a dependency on a private
//!     helper that may change shape.
//!
//! This is feature-gated on `hardware-render`. The default build path
//! (and every existing test in this repo) keeps using
//! `SoftwareRenderingContext`; the hardware constructor only exists
//! when the feature is enabled, which is also when the additional
//! surfman/gleam/glow deps are pulled in.

#![cfg(feature = "hardware-render")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use euclid::Size2D;
use gleam::gl::{self, Gl};
use image::RgbaImage;
use servo::{DeviceIntRect, RenderingContext};
use surfman::chains::{PreserveBuffer, SwapChain};
use surfman::{
    Connection, Context, ContextAttributeFlags, ContextAttributes, Device, Error as SurfmanError,
    GLApi, NativeWidget, Surface, SurfaceAccess, SurfaceType,
};

/// A headless hardware-backed [`RenderingContext`].
///
/// Construct with [`HardwareOffscreenContext::new`]; drop normally to
/// release the surfman context, surface, and swap chain.
pub struct HardwareOffscreenContext {
    size: Cell<PhysicalSize<u32>>,
    inner: SurfmanInner,
    swap_chain: SwapChain<Device>,
}

impl HardwareOffscreenContext {
    /// Build a new hardware context with an offscreen
    /// [`SurfaceType::Generic`] surface of the requested size.
    ///
    /// Uses `Connection::new()` to pick the platform default
    /// (CGL on macOS — which backs surfaces with `IOSurface`s —
    /// EGL on Linux, WGL on Windows) and `create_adapter()` for the
    /// real GPU adapter. Falls back nowhere: if the host can't give
    /// us a hardware GL context, the returned `Err` carries the
    /// surfman cause and the caller is expected to either retry with
    /// the software path or surface the failure.
    pub fn new(size: PhysicalSize<u32>) -> Result<Self, SurfmanError> {
        let connection = Connection::new()?;
        let adapter = connection.create_adapter()?;
        let inner = SurfmanInner::new(&connection, &adapter)?;
        let surfman_size = Size2D::new(size.width as i32, size.height as i32);
        let surface = inner.create_surface(SurfaceType::Generic { size: surfman_size })?;
        inner.bind_surface(surface)?;
        inner.make_current()?;
        let swap_chain = inner.create_attached_swap_chain()?;
        Ok(Self { size: Cell::new(size), inner, swap_chain })
    }
}

impl Drop for HardwareOffscreenContext {
    fn drop(&mut self) {
        let device = &mut self.inner.device.borrow_mut();
        let context = &mut self.inner.context.borrow_mut();
        let _ = self.swap_chain.destroy(device, context);
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
        if self.size.get() == size {
            return;
        }

        self.size.set(size);

        let device = &mut self.inner.device.borrow_mut();
        let context = &mut self.inner.context.borrow_mut();
        let size = Size2D::new(size.width as i32, size.height as i32);
        let _ = self.swap_chain.resize(device, context, size);
    }

    fn present(&self) {
        let device = &mut self.inner.device.borrow_mut();
        let context = &mut self.inner.context.borrow_mut();
        let _ = self.swap_chain.swap_buffers(device, context, PreserveBuffer::No);
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
use crate::iosurface_handle::{IOSurfaceHandle, IOSurfaceIdentity};

#[cfg(target_os = "macos")]
impl HardwareOffscreenContext {
    /// Cheap, non-mutating identity probe of the currently bound
    /// surface. Reads `Device::context_surface_info` (no unbind, no
    /// mach port creation) so callers can dedup before paying the
    /// price of `current_iosurface_mach_port`.
    pub fn peek_iosurface_identity(&self) -> Result<IOSurfaceIdentity, SurfmanError> {
        let device = self.inner.device.borrow();
        let context = self.inner.context.borrow();
        let info = device.context_surface_info(&context)?.ok_or(SurfmanError::Failed)?;
        Ok(IOSurfaceIdentity {
            surface_id: info.id.0 as u64,
            width: u32::try_from(info.size.width).unwrap_or(0),
            height: u32::try_from(info.size.height).unwrap_or(0),
        })
    }

    /// Snapshot the IOSurface currently bound to the context and
    /// return its mach port name plus dimensions and stable surface
    /// id. Increments the IOSurface's mach-port use count; the
    /// receiving process holds it via `IOSurfaceLookupFromMachPort` and
    /// is responsible for `mach_port_deallocate` once the import is
    /// finished.
    ///
    /// Implementation note: surfman's CGL backend keeps the bound
    /// surface inside the GL context. To inspect it we temporarily
    /// `unbind_surface_from_context`, call `device.native_surface()`
    /// (which retains the `IOSurfaceRef`), then `bind_surface_to_context`
    /// again. The unbind path calls `glFlush` so the IOSurface contents
    /// are consistent for any reader importing it after this returns.
    pub fn current_iosurface_mach_port(&self) -> Result<IOSurfaceHandle, SurfmanError> {
        let device = &mut self.inner.device.borrow_mut();
        let context = &mut self.inner.context.borrow_mut();
        // `new` always binds a surface and `current_iosurface_mach_port`
        // is the only method that unbinds; the `None` branch only fires
        // if the invariant has been broken from outside.
        let surface =
            device.unbind_surface_from_context(context)?.ok_or(SurfmanError::Failed)?;
        let native = device.native_surface(&surface);
        let mach_port = native.0.create_mach_port();
        let info = device.surface_info(&surface);
        let handle = IOSurfaceHandle {
            mach_port_name: mach_port,
            surface_id: info.id.0 as u64,
            width: u32::try_from(info.size.width).unwrap_or(0),
            height: u32::try_from(info.size.height).unwrap_or(0),
        };
        device.bind_surface_to_context(context, surface).map_err(|(error, _)| error)?;
        Ok(handle)
    }
}

/// Trimmed mirror of `paint_api::rendering_context::SurfmanRenderingContext`.
///
/// Only the methods the public type above actually uses are kept; the
/// upstream original also wires up texture sharing, refresh drivers,
/// and several other knobs that Servo's compositor reaches into but
/// the embedder's headless readback path does not.
struct SurfmanInner {
    gleam_gl: Rc<dyn Gl>,
    glow_gl: Arc<glow::Context>,
    device: RefCell<Device>,
    context: RefCell<Context>,
}

impl Drop for SurfmanInner {
    fn drop(&mut self) {
        let device = &mut self.device.borrow_mut();
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
        let version = match &gl_api {
            GLApi::GLES => surfman::GLVersion { major: 3, minor: 0 },
            GLApi::GL => surfman::GLVersion { major: 3, minor: 2 },
        };
        let context_descriptor =
            device.create_context_descriptor(&ContextAttributes { flags, version })?;
        let context = device.create_context(&context_descriptor, None)?;

        // Loading the GL function pointers requires unsafe ABI calls
        // through surfman's `get_proc_address` — these are the same
        // calls the upstream `SurfmanRenderingContext::new` makes,
        // and they're sound for the same reason: surfman guarantees
        // the returned function pointers match the requested API.
        #[expect(unsafe_code)]
        let gleam_gl = {
            match gl_api {
                GLApi::GL => unsafe {
                    gl::GlFns::load_with(|name| device.get_proc_address(&context, name))
                },
                GLApi::GLES => unsafe {
                    gl::GlesFns::load_with(|name| device.get_proc_address(&context, name))
                },
            }
        };

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
        let device = &mut self.device.borrow_mut();
        let context = &self.context.borrow();
        device.create_surface(context, SurfaceAccess::GPUOnly, surface_type)
    }

    fn bind_surface(&self, surface: Surface) -> Result<(), SurfmanError> {
        let device = &self.device.borrow();
        let context = &mut self.context.borrow_mut();
        device
            .bind_surface_to_context(context, surface)
            .map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(context, &mut surface);
                err
            })?;
        Ok(())
    }

    fn create_attached_swap_chain(&self) -> Result<SwapChain<Device>, SurfmanError> {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        SwapChain::create_attached(device, context, SurfaceAccess::GPUOnly)
    }

    fn make_current(&self) -> Result<(), SurfmanError> {
        let device = &self.device.borrow();
        let context = &self.context.borrow();
        device.make_context_current(context)
    }

    fn framebuffer_id(&self) -> u32 {
        let device = &self.device.borrow();
        let context = &self.context.borrow();
        device
            .context_surface_info(context)
            .unwrap_or(None)
            .and_then(|info| info.framebuffer_object)
            .map_or(0, |framebuffer| framebuffer.0.into())
    }

    fn prepare_for_rendering(&self) {
        let framebuffer_id = self.framebuffer_id();
        self.gleam_gl.bind_framebuffer(gleam::gl::FRAMEBUFFER, framebuffer_id);
    }

    /// Inlined copy of `Framebuffer::read_framebuffer_to_image` from
    /// `paint-api`. Reads the bound framebuffer into a `Vec<u8>`,
    /// flips it vertically (GL's origin is bottom-left, the rest of
    /// the embedder expects top-left), and returns it as an
    /// [`RgbaImage`]. Returns `None` if `RgbaImage::from_raw` rejects
    /// the buffer (size mismatch); GL errors are logged but don't
    /// abort the read — the caller can decide whether a corrupt
    /// frame is recoverable.
    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        let framebuffer_id = self.framebuffer_id();
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer_id);
        // Working around an OSMesa headless bug carried forward from
        // the upstream implementation, see servo/servo#18606.
        self.gleam_gl.bind_vertex_array(0);

        let mut pixels = self.gleam_gl.read_pixels(
            source_rectangle.min.x,
            source_rectangle.min.y,
            source_rectangle.width(),
            source_rectangle.height(),
            gl::RGBA,
            gl::UNSIGNED_BYTE,
        );
        let gl_error = self.gleam_gl.get_error();
        if gl_error != gl::NO_ERROR {
            log::warn!("GL error 0x{gl_error:x} after read_pixels in hardware offscreen context");
        }

        let source_rectangle = source_rectangle.to_usize();
        let stride = source_rectangle.width().checked_mul(4)?;
        let mirror = pixels.clone();
        for y in 0..source_rectangle.height() {
            let dst_start = y.checked_mul(stride)?;
            let src_start = (source_rectangle.height().checked_sub(y + 1)?).checked_mul(stride)?;
            let dst_end = dst_start.checked_add(stride)?;
            let src_end = src_start.checked_add(stride)?;
            if dst_end > pixels.len() || src_end > mirror.len() {
                return None;
            }
            pixels[dst_start..dst_end].clone_from_slice(&mirror[src_start..src_end]);
        }

        RgbaImage::from_raw(
            source_rectangle.width() as u32,
            source_rectangle.height() as u32,
            pixels,
        )
    }
}
