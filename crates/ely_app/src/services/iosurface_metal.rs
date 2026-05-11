//! macOS-only import of cross-process IOSurface handles into Metal
//! textures.
//!
//! `T10.4`: the sidecar publishes an [`crate::services::servo_live`]
//! `surface_handle` carrying a `mach_port_name` + stable `surface_id`.
//! The receiver builds an `MTLTexture` from that IOSurface exactly
//! once per `surface_id` and caches it. Every frame after the first
//! reads `current_surface_id` and samples the cached texture — zero
//! pixel copy, zero re-import.
//!
//! Lifetime contract:
//!
//!   * `IOSurfaceCreateMachPort` (sidecar side) gives the receiver a
//!     send right whose refcount is 1 in our task. Once we've called
//!     `IOSurfaceLookupFromMachPort` to materialise the
//!     `IOSurfaceRef`, the mach port has done its job.
//!   * Metal's `newTextureWithDescriptor:iosurface:plane:` retains the
//!     IOSurface for the texture's lifetime. We `mach_port_deallocate`
//!     immediately afterwards so the receiver process doesn't
//!     accumulate idle mach send rights.
//!   * Dropping `MetalSurfaceImporter` releases every cached
//!     `MTLTexture`, which in turn releases each retained IOSurface.
//!     The sidecar still holds its own retain via surfman, so the
//!     IOSurface itself outlives our cache for as long as the sidecar
//!     keeps painting.

#![cfg(target_os = "macos")]

use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSUInteger;
use objc2_io_surface::IOSurfaceRef;
use objc2_metal::{
    MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat, MTLTexture, MTLTextureDescriptor,
    MTLTextureUsage,
};
use thiserror::Error;

/// Owner of the system Metal device + cache of imported textures
/// keyed by IOSurface identity. Constructed once per renderer process
/// when the first hardware-path tab requests an upload.
pub(crate) struct MetalSurfaceImporter {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    textures: HashMap<u64, Retained<ProtocolObject<dyn MTLTexture>>>,
}

#[derive(Debug, Error)]
pub(crate) enum SurfaceImportError {
    #[error("system has no default Metal device — hardware path unavailable")]
    NoMetalDevice,
    #[error("IOSurfaceLookupFromMachPort returned null for port 0x{port:x}")]
    LookupFailed { port: u32 },
    #[error("MTLDevice rejected the IOSurface (size {width}x{height})")]
    TextureBuildFailed { width: u32, height: u32 },
}

impl MetalSurfaceImporter {
    pub fn new() -> Result<Self, SurfaceImportError> {
        let device = MTLCreateSystemDefaultDevice().ok_or(SurfaceImportError::NoMetalDevice)?;
        Ok(Self { device, textures: HashMap::new() })
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
        width: u32,
        height: u32,
    ) -> Result<(), SurfaceImportError> {
        if self.textures.contains_key(&surface_id) {
            deallocate_mach_port(mach_port_name);
            return Ok(());
        }

        let Some(iosurface) = IOSurfaceRef::lookup_from_mach_port(mach_port_name) else {
            // Lookup failed → port is invalid; nothing to deallocate.
            return Err(SurfaceImportError::LookupFailed { port: mach_port_name });
        };

        let descriptor = MTLTextureDescriptor::new();
        // surfman's macOS surface backs IOSurface with
        // kCVPixelFormatType_32BGRA — match it so Metal samples the
        // correct channel order. `setUsage(ShaderRead)` is the minimum
        // Metal needs to expose the texture to a fragment shader sampler.
        // The setters are unsafe because they cross the FFI boundary
        // without descriptor validation; we know our values are sound.
        #[expect(unsafe_code)]
        unsafe {
            descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            descriptor.setWidth(width as NSUInteger);
            descriptor.setHeight(height as NSUInteger);
            descriptor.setUsage(MTLTextureUsage::ShaderRead);
        }

        let texture = self
            .device
            .newTextureWithDescriptor_iosurface_plane(&descriptor, &iosurface, 0)
            .ok_or(SurfaceImportError::TextureBuildFailed { width, height })?;

        self.textures.insert(surface_id, texture);
        deallocate_mach_port(mach_port_name);
        Ok(())
    }

    /// Look up an already-imported texture by `surface_id`. The
    /// receiver's per-frame `current_surface_id` field selects which
    /// of the chain's rotating front/back surfaces to sample.
    /// `#[allow(dead_code)]` until T10.5 wires the renderer; the
    /// import side is exercised by the live perf bench right now.
    #[allow(dead_code)]
    pub fn texture_for(
        &self,
        surface_id: u64,
    ) -> Option<&Retained<ProtocolObject<dyn MTLTexture>>> {
        self.textures.get(&surface_id)
    }

    /// Reports how many distinct surfaces have been imported. Used
    /// by tests and the live perf bench to assert that dedup is
    /// keeping the cache small (one per swap-chain surface).
    #[cfg(test)]
    pub fn cached_surface_count(&self) -> usize {
        self.textures.len()
    }
}

/// Release one send right against the mach port we received. The
/// IOSurface itself stays alive because the `MTLTexture` (or the
/// sidecar's surfman) still retain it. A `KERN_INVALID_NAME` failure
/// here means the port already drained — survivable, log and move
/// on.
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
    /// retain via the MTLTexture so this just frees our port slot.
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{MetalSurfaceImporter, SurfaceImportError};
    use objc2_core_foundation::{
        CFDictionary, CFIndex, CFNumber, CFRetained, CFString, kCFAllocatorDefault,
        kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    };
    use objc2_io_surface::{
        IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceHeight,
        kIOSurfacePixelFormat, kIOSurfaceWidth,
    };
    use objc2_metal::MTLTexture as _;
    use std::os::raw::c_void;

    const TEST_WIDTH: u32 = 64;
    const TEST_HEIGHT: u32 = 48;

    /// Build a CPU-backed IOSurface from scratch, the same way
    /// surfman's macOS backend does — BGRA8 (four-cc '32BGRA'), width
    /// + height + bytes_per_element + bytes_per_row in a Core
    /// Foundation properties dictionary. The pointer-casts mirror
    /// `surfman::platform::macos::system::surface::create_io_surface`;
    /// the dict has CFString keys and CFNumber values and is built
    /// with `kCFTypeDictionaryKeyCallBacks` / `kCFTypeDictionaryValueCallBacks`
    /// so CF retains its entries.
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
    fn imports_local_iosurface_into_mtl_texture() {
        let mut importer = match MetalSurfaceImporter::new() {
            Ok(importer) => importer,
            Err(SurfaceImportError::NoMetalDevice) => {
                eprintln!(
                    "no Metal device on this host — \
                     acceptable in headless / no-GPU CI; skipping"
                );
                return;
            }
            Err(error) => panic!("unexpected importer error: {error:?}"),
        };

        let iosurface = build_local_iosurface();
        let mach_port = iosurface.create_mach_port();
        assert!(mach_port != 0, "IOSurfaceCreateMachPort must yield a real port");
        let surface_id: u64 = 0xDEAD_BEEFu64;

        importer
            .import(mach_port, surface_id, TEST_WIDTH, TEST_HEIGHT)
            .expect("local IOSurface must round-trip through MTLDevice");

        let texture = importer
            .texture_for(surface_id)
            .expect("imported texture must be retrievable by surface_id");
        assert_eq!(texture.width(), TEST_WIDTH as usize, "MTLTexture width must match");
        assert_eq!(texture.height(), TEST_HEIGHT as usize, "MTLTexture height must match");
        assert_eq!(importer.cached_surface_count(), 1);
    }

    #[test]
    fn second_import_with_same_surface_id_is_idempotent() {
        let mut importer = match MetalSurfaceImporter::new() {
            Ok(importer) => importer,
            Err(SurfaceImportError::NoMetalDevice) => return,
            Err(error) => panic!("unexpected importer error: {error:?}"),
        };

        let iosurface = build_local_iosurface();
        let port_a = iosurface.create_mach_port();
        let port_b = iosurface.create_mach_port();
        assert!(port_a != 0 && port_b != 0 && port_a != port_b);

        importer.import(port_a, 0xAAAA_AAAA, TEST_WIDTH, TEST_HEIGHT).expect("first import");
        // Same surface_id → defensive dedup path; port_b is deallocated
        // without minting a duplicate MTLTexture.
        importer
            .import(port_b, 0xAAAA_AAAA, TEST_WIDTH, TEST_HEIGHT)
            .expect("duplicate import is idempotent");
        assert_eq!(importer.cached_surface_count(), 1);
    }
}
