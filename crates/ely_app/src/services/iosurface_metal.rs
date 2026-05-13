//! macOS-only import of cross-process IOSurface handles into
//! `CVPixelBuffer`s that preserve the sidecar's IOSurface identity.
//!
//! `T10.4` originally imported the IOSurface into an `MTLTexture`
//! directly. GPUI 0.2.2 exposes `Window::paint_surface` /
//! `elements::surface::Surface` for `CVPixelBuffer`, and that public
//! path is wired for NV12 video frames. Servo's hardware renderer
//! publishes BGRA IOSurfaces, so this cache stays as verified
//! cross-process plumbing until the presenter accepts BGRA surfaces.
//!
//! Lifetime contract:
//!
//!   * `IOSurfaceCreateMachPort` (sidecar side) gives the receiver a
//!     send right whose refcount is 1 in our task. After we resolve
//!     the surface and wrap it in a CVPixelBuffer, the mach port has
//!     done its job.
//!   * `CVPixelBufferCreateWithIOSurface` retains the IOSurface for
//!     the pixel buffer's lifetime. We `mach_port_deallocate`
//!     immediately so the receiver process doesn't accumulate idle
//!     mach send rights.
//!   * Dropping `IOSurfaceCache` releases every cached
//!     `CVPixelBuffer`, which in turn releases each retained
//!     IOSurface. The sidecar still holds its own retain via surfman,
//!     so the IOSurface itself outlives our cache for as long as the
//!     sidecar keeps painting.

#![cfg(target_os = "macos")]

use std::collections::HashMap;

use core_foundation::base::TCFType as _;
use core_video::pixel_buffer::CVPixelBuffer;
#[allow(deprecated)]
use io_surface::IOSurface;
use thiserror::Error;

/// Cache of imported `CVPixelBuffer`s keyed by IOSurface identity.
/// Constructed lazily by the renderer-side client on the first
/// hardware-path frame.
pub(crate) struct IOSurfaceCache {
    pixel_buffers: HashMap<u64, CVPixelBuffer>,
}

#[derive(Debug, Error)]
pub(crate) enum SurfaceImportError {
    #[error("IOSurfaceLookupFromMachPort returned null for port 0x{port:x}")]
    LookupFailed { port: u32 },
    #[error("CVPixelBufferCreateWithIOSurface returned status {status}")]
    PixelBufferBuildFailed { status: i32 },
}

impl IOSurfaceCache {
    pub fn new() -> Self {
        Self { pixel_buffers: HashMap::new() }
    }

    /// Import an IOSurface published by the sidecar's
    /// `surface_handle` field. Idempotent on `surface_id`: a second
    /// call with the same id immediately deallocates the duplicate
    /// mach port without re-importing. The sender's T10.3 dedup means
    /// the duplicate path should never fire in practice — it's here
    /// so a misbehaving sidecar can't quietly leak ports.
    pub fn import(
        &mut self,
        mach_port_name: u32,
        surface_id: u64,
    ) -> Result<(), SurfaceImportError> {
        if self.pixel_buffers.contains_key(&surface_id) {
            deallocate_mach_port(mach_port_name);
            return Ok(());
        }

        let Some(iosurface) = objc2_io_surface::IOSurfaceRef::lookup_from_mach_port(mach_port_name)
        else {
            return Err(SurfaceImportError::LookupFailed { port: mach_port_name });
        };

        // Both objc2-io-surface and the legacy `io_surface` crate wrap
        // the same C `__IOSurface` pointer. CVPixelBufferCreateWithIOSurface
        // (via core-video) expects the legacy crate's wrapper. Reach for
        // the raw pointer and let TCFType CFRetain it independently so
        // both Rust handles can drop without double-freeing.
        let raw_ptr: *const std::ffi::c_void =
            (&*iosurface) as *const objc2_io_surface::IOSurfaceRef as *const std::ffi::c_void;
        #[allow(deprecated)]
        let io_surface_view: IOSurface = {
            #[expect(unsafe_code)]
            unsafe {
                IOSurface::wrap_under_get_rule(raw_ptr as io_surface::IOSurfaceRef)
            }
        };

        let pixel_buffer = CVPixelBuffer::from_io_surface(&io_surface_view, None)
            .map_err(|status| SurfaceImportError::PixelBufferBuildFailed { status })?;

        self.pixel_buffers.insert(surface_id, pixel_buffer);
        deallocate_mach_port(mach_port_name);
        Ok(())
    }

    /// Look up an already-imported pixel buffer by `surface_id`. The
    /// receiver's per-frame `current_surface_id` selects which of the
    /// swap chain's rotating front/back surfaces to sample. Returns a
    /// clone (CVPixelBuffer is reference-counted; cloning is a cheap
    /// atomic increment) so the caller can hand it to GPUI's
    /// `surface(...)` element without holding a borrow on the cache.
    pub fn pixel_buffer_for(&self, surface_id: u64) -> Option<CVPixelBuffer> {
        self.pixel_buffers.get(&surface_id).cloned()
    }

    #[cfg(test)]
    pub fn cached_surface_count(&self) -> usize {
        self.pixel_buffers.len()
    }
}

/// Release one send right against the mach port we received. The
/// IOSurface itself stays alive because the `CVPixelBuffer` (or the
/// sidecar's surfman) still retain it.
fn deallocate_mach_port(port: u32) {
    #[expect(unsafe_code)]
    let result = unsafe { mach_port_deallocate(mach_task_self_, port) };
    if result != KERN_SUCCESS {
        tracing::warn!(
            target: "ely::servo::iosurface",
            mach_port_name = port,
            kern_result = result,
            "mach_port_deallocate returned non-success",
        );
    }
}

const KERN_SUCCESS: i32 = 0;

#[expect(unsafe_code)]
unsafe extern "C" {
    /// Global mach task port for the running process. Defined in
    /// `mach/mach_init.h` as `extern mach_port_t mach_task_self_;`.
    static mach_task_self_: u32;

    /// Releases one send right against `name` within `task`. We only
    /// ever call this with our own task; the IOSurface keeps its
    /// retain via the CVPixelBuffer so this just frees our port slot.
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::IOSurfaceCache;
    use objc2_core_foundation::{
        CFDictionary, CFIndex, CFNumber, CFRetained, CFString, kCFAllocatorDefault,
        kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    };
    use objc2_io_surface::{
        IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceHeight,
        kIOSurfacePixelFormat, kIOSurfaceWidth,
    };
    use std::os::raw::c_void;

    const TEST_WIDTH: u32 = 64;
    const TEST_HEIGHT: u32 = 48;

    /// Build a CPU-backed IOSurface from scratch, the same way
    /// surfman's macOS backend does — BGRA8 (four-cc '32BGRA'), width
    /// + height + bytes_per_element + bytes_per_row in a Core
    /// Foundation properties dictionary. The pointer-casts mirror
    /// `surfman::platform::macos::system::surface::create_io_surface`.
    fn build_local_iosurface() -> CFRetained<IOSurfaceRef> {
        let pixel_format: i32 = i32::from_be_bytes(*b"BGRA");
        let bytes_per_element: i32 = 4;
        let bytes_per_row: i32 = (TEST_WIDTH as i32) * bytes_per_element;

        let width_num = CFNumber::new_i32(TEST_WIDTH as i32);
        let height_num = CFNumber::new_i32(TEST_HEIGHT as i32);
        let bpe_num = CFNumber::new_i32(bytes_per_element);
        let bpr_num = CFNumber::new_i32(bytes_per_row);
        let pf_num = CFNumber::new_i32(pixel_format);

        #[expect(unsafe_code)]
        unsafe {
            let keys: [&CFString; 5] = [
                kIOSurfaceWidth,
                kIOSurfaceHeight,
                kIOSurfaceBytesPerElement,
                kIOSurfaceBytesPerRow,
                kIOSurfacePixelFormat,
            ];
            let values: [&CFNumber; 5] = [&width_num, &height_num, &bpe_num, &bpr_num, &pf_num];
            let keys_ptr: *mut *const c_void = keys.as_ptr() as *mut *const c_void;
            let values_ptr: *mut *const c_void = values.as_ptr() as *mut *const c_void;
            let properties = CFDictionary::new(
                kCFAllocatorDefault,
                keys_ptr,
                values_ptr,
                keys.len() as CFIndex,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
            .expect("CFDictionaryCreate must succeed for the properties dict");
            IOSurfaceRef::new(&properties)
                .expect("IOSurfaceCreate must succeed for a well-formed properties dict")
        }
    }

    #[test]
    fn imports_local_iosurface_into_pixel_buffer() {
        let mut cache = IOSurfaceCache::new();
        let iosurface = build_local_iosurface();
        let mach_port = iosurface.create_mach_port();
        assert!(mach_port != 0, "IOSurfaceCreateMachPort must yield a real port");
        let surface_id: u64 = 0xDEAD_BEEFu64;

        cache
            .import(mach_port, surface_id)
            .expect("local IOSurface must round-trip into a CVPixelBuffer");

        let pixel_buffer = cache
            .pixel_buffer_for(surface_id)
            .expect("imported pixel buffer must be retrievable by surface_id");
        assert_eq!(
            pixel_buffer.get_width() as u32,
            TEST_WIDTH,
            "CVPixelBuffer width must match the source IOSurface",
        );
        assert_eq!(
            pixel_buffer.get_height() as u32,
            TEST_HEIGHT,
            "CVPixelBuffer height must match the source IOSurface",
        );
        assert_eq!(cache.cached_surface_count(), 1);
    }

    #[test]
    fn second_import_with_same_surface_id_is_idempotent() {
        let mut cache = IOSurfaceCache::new();
        let iosurface = build_local_iosurface();
        let port_a = iosurface.create_mach_port();
        let port_b = iosurface.create_mach_port();
        assert!(port_a != 0 && port_b != 0 && port_a != port_b);

        cache.import(port_a, 0xAAAA_AAAA).expect("first import");
        // Same surface_id → defensive dedup path; port_b is deallocated
        // without minting a duplicate CVPixelBuffer.
        cache.import(port_b, 0xAAAA_AAAA).expect("duplicate import is idempotent");
        assert_eq!(cache.cached_surface_count(), 1);
    }
}
