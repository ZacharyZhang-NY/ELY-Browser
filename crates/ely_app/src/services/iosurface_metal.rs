#![cfg(target_os = "macos")]

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Weak},
};

use core_foundation::base::TCFType as _;
use core_video::pixel_buffer::CVPixelBuffer;
#[allow(deprecated)]
use io_surface::IOSurface;
use mach2::{mach_port::mach_port_deallocate, traps::mach_task_self};
use objc2_core_foundation::CFRetained;
use objc2_io_surface::IOSurfaceRef;
use thiserror::Error;

pub(crate) struct IOSurfaceCache {
    surfaces: BTreeMap<u64, CachedSurface>,
    insertion_order: VecDeque<u64>,
}

const MAX_CACHED_SURFACES: usize = 16;

struct CachedSurface {
    iosurface: CFRetained<IOSurfaceRef>,
    pending_backing: Option<Arc<HardwareSurfaceBacking>>,
    active_backing: Weak<HardwareSurfaceBacking>,
    width: u32,
    height: u32,
}

#[derive(Debug, Error)]
pub(crate) enum SurfaceImportError {
    #[error("IOSurfaceLookupFromMachPort returned null for port 0x{port:x}")]
    LookupFailed { port: u32 },
    #[error("CVPixelBufferCreateWithIOSurface returned status {status}")]
    PixelBufferBuildFailed { status: i32 },
}

pub(crate) struct ImportedPixelBuffer {
    pub(crate) system_surface_id: u64,
    pub(crate) iosurface: CFRetained<IOSurfaceRef>,
    pub(crate) backing: Arc<HardwareSurfaceBacking>,
}

pub(crate) struct HardwareSurfaceBacking {
    pixel_buffer: CVPixelBuffer,
}

// SAFETY: `CVPixelBuffer` is a retained, read-only CoreVideo frame here.
// CoreFoundation retain/release is thread-safe, and both the runtime and GPU
// paths only clone the handle or submit the immutable IOSurface-backed image.
#[expect(unsafe_code)]
unsafe impl Send for HardwareSurfaceBacking {}
// SAFETY: The backing exposes immutable pixel-buffer metadata and clones.
// IOSurface lifetime tracking remains owned by CoreVideo's retained buffer.
#[expect(unsafe_code)]
unsafe impl Sync for HardwareSurfaceBacking {}

impl HardwareSurfaceBacking {
    pub(crate) fn new(pixel_buffer: CVPixelBuffer) -> Arc<Self> {
        Arc::new(Self { pixel_buffer })
    }

    pub(crate) fn pixel_buffer(&self) -> &CVPixelBuffer {
        &self.pixel_buffer
    }
}

impl IOSurfaceCache {
    pub(crate) fn new() -> Self {
        Self { surfaces: BTreeMap::new(), insertion_order: VecDeque::new() }
    }

    #[cfg(test)]
    fn import(&mut self, mach_port_name: u32, surface_id: u64) -> Result<(), SurfaceImportError> {
        let imported = import_pixel_buffer_from_mach_port(mach_port_name)?;
        self.insert_imported(surface_id, imported);
        Ok(())
    }

    pub(crate) fn insert_imported(&mut self, surface_id: u64, imported: ImportedPixelBuffer) {
        let width = imported.backing.pixel_buffer().get_width() as u32;
        let height = imported.backing.pixel_buffer().get_height() as u32;
        if self
            .surfaces
            .get(&surface_id)
            .is_some_and(|cached| cached.width == width && cached.height == height)
        {
            return;
        }
        if !self.surfaces.contains_key(&surface_id) {
            while self.surfaces.len() >= MAX_CACHED_SURFACES {
                if !self.evict_preferred_surface() {
                    break;
                }
            }
            self.insertion_order.push_back(surface_id);
        }
        self.surfaces.insert(
            surface_id,
            CachedSurface {
                iosurface: imported.iosurface,
                pending_backing: Some(imported.backing),
                active_backing: Weak::new(),
                width,
                height,
            },
        );
    }

    pub(crate) fn hardware_surface_for(
        &mut self,
        surface_id: u64,
    ) -> Result<Option<Arc<HardwareSurfaceBacking>>, SurfaceImportError> {
        let Some(cached) = self.surfaces.get_mut(&surface_id) else {
            return Ok(None);
        };
        let backing = cached
            .active_backing
            .upgrade()
            .or_else(|| cached.pending_backing.take())
            .map(Ok)
            .unwrap_or_else(|| build_hardware_surface_backing(&cached.iosurface))?;
        cached.active_backing = Arc::downgrade(&backing);
        self.trim_to_capacity();
        Ok(Some(backing))
    }

    pub(crate) fn surface_ids(&self) -> Vec<u64> {
        self.surfaces.keys().copied().collect()
    }

    #[cfg(test)]
    fn cached_surface_count(&self) -> usize {
        self.surfaces.len()
    }

    fn trim_to_capacity(&mut self) {
        while self.surfaces.len() > MAX_CACHED_SURFACES {
            if !self.evict_preferred_surface() {
                break;
            }
        }
    }

    fn evict_preferred_surface(&mut self) -> bool {
        let inactive_index = self.insertion_order.iter().position(|surface_id| {
            self.surfaces
                .get(surface_id)
                .is_none_or(|cached| cached.active_backing.upgrade().is_none())
        });
        let surface_id = match inactive_index {
            Some(index) => self.insertion_order.remove(index),
            None => self.insertion_order.pop_front(),
        };
        let Some(surface_id) = surface_id else {
            return false;
        };
        self.surfaces.remove(&surface_id);
        true
    }
}

pub(crate) fn import_pixel_buffer_from_mach_port(
    mach_port_name: u32,
) -> Result<ImportedPixelBuffer, SurfaceImportError> {
    let result = build_pixel_buffer_from_mach_port(mach_port_name);
    deallocate_mach_port(mach_port_name);
    result
}

fn build_pixel_buffer_from_mach_port(
    mach_port_name: u32,
) -> Result<ImportedPixelBuffer, SurfaceImportError> {
    let Some(iosurface) = objc2_io_surface::IOSurfaceRef::lookup_from_mach_port(mach_port_name)
    else {
        return Err(SurfaceImportError::LookupFailed { port: mach_port_name });
    };
    let backing = build_hardware_surface_backing(&iosurface)?;
    Ok(ImportedPixelBuffer { system_surface_id: u64::from(iosurface.id()), iosurface, backing })
}

fn build_hardware_surface_backing(
    iosurface: &CFRetained<IOSurfaceRef>,
) -> Result<Arc<HardwareSurfaceBacking>, SurfaceImportError> {
    let raw_ptr: *const std::ffi::c_void =
        (&**iosurface) as *const objc2_io_surface::IOSurfaceRef as *const std::ffi::c_void;
    #[allow(deprecated)]
    let io_surface_view: IOSurface = {
        #[expect(unsafe_code)]
        unsafe {
            IOSurface::wrap_under_get_rule(raw_ptr as io_surface::IOSurfaceRef)
        }
    };

    let pixel_buffer = CVPixelBuffer::from_io_surface(&io_surface_view, None)
        .map_err(|status| SurfaceImportError::PixelBufferBuildFailed { status })?;
    Ok(HardwareSurfaceBacking::new(pixel_buffer))
}

fn deallocate_mach_port(port: u32) {
    #[expect(unsafe_code)]
    let task = unsafe { mach_task_self() };
    #[expect(unsafe_code)]
    let _ = unsafe { mach_port_deallocate(task, port) };
}

#[cfg(test)]
mod tests {
    use std::os::raw::c_void;
    use std::sync::Arc;

    use objc2_core_foundation::{
        CFDictionary, CFIndex, CFNumber, CFString, kCFAllocatorDefault,
        kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    };
    use objc2_io_surface::{
        IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceHeight,
        kIOSurfacePixelFormat, kIOSurfaceWidth,
    };

    use super::IOSurfaceCache;

    fn build_local_iosurface(
        width: u32,
        height: u32,
    ) -> Result<objc2_core_foundation::CFRetained<IOSurfaceRef>, String> {
        let pixel_format = i32::from_be_bytes(*b"BGRA");
        let bytes_per_element = 4_i32;
        let bytes_per_row = (width as i32) * bytes_per_element;
        let width_num = CFNumber::new_i32(width as i32);
        let height_num = CFNumber::new_i32(height as i32);
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
            let properties = CFDictionary::new(
                kCFAllocatorDefault,
                keys.as_ptr() as *mut *const c_void,
                values.as_ptr() as *mut *const c_void,
                keys.len() as CFIndex,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
            .ok_or_else(|| "CFDictionaryCreate returned null".to_string())?;
            IOSurfaceRef::new(&properties)
                .ok_or_else(|| "IOSurfaceCreate returned null".to_string())
        }
    }

    #[test]
    fn imports_iosurface_into_pixel_buffer_cache() -> Result<(), String> {
        let mut cache = IOSurfaceCache::new();
        let iosurface = build_local_iosurface(64, 48)?;
        let surface_id = u64::from(iosurface.id());

        let imported = super::import_pixel_buffer_from_mach_port(iosurface.create_mach_port())
            .map_err(|error| error.to_string())?;
        assert_eq!(imported.system_surface_id, surface_id);
        cache.insert_imported(surface_id, imported);

        let surface = cache
            .hardware_surface_for(surface_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "imported pixel buffer was missing".to_string())?;
        assert_eq!(surface.pixel_buffer().get_width(), 64);
        assert_eq!(surface.pixel_buffer().get_height(), 48);
        assert_eq!(cache.cached_surface_count(), 1);
        Ok(())
    }

    #[test]
    fn repeated_surface_id_reuses_cache_entry() -> Result<(), String> {
        let mut cache = IOSurfaceCache::new();
        let iosurface = build_local_iosurface(64, 48)?;
        let surface_id = u64::from(iosurface.id());

        cache
            .import(iosurface.create_mach_port(), surface_id)
            .map_err(|error| error.to_string())?;
        cache
            .import(iosurface.create_mach_port(), surface_id)
            .map_err(|error| error.to_string())?;

        assert_eq!(cache.cached_surface_count(), 1);
        Ok(())
    }

    #[test]
    fn active_surface_backing_reuses_and_releases_use_count() -> Result<(), String> {
        let mut cache = IOSurfaceCache::new();
        let iosurface = build_local_iosurface(64, 48)?;
        let surface_id = u64::from(iosurface.id());
        let baseline_use_count = iosurface.use_count();
        let imported = super::import_pixel_buffer_from_mach_port(iosurface.create_mach_port())
            .map_err(|error| error.to_string())?;
        assert_eq!(iosurface.use_count(), baseline_use_count + 1);
        cache.insert_imported(surface_id, imported);

        let first = cache
            .hardware_surface_for(surface_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "first hardware surface was missing".to_string())?;
        let second = cache
            .hardware_surface_for(surface_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "second hardware surface was missing".to_string())?;
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(iosurface.use_count(), baseline_use_count + 1);

        drop(first);
        drop(second);
        assert_eq!(iosurface.use_count(), baseline_use_count);

        let rebuilt = cache
            .hardware_surface_for(surface_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "rebuilt hardware surface was missing".to_string())?;
        assert_eq!(iosurface.use_count(), baseline_use_count + 1);
        drop(rebuilt);
        assert_eq!(iosurface.use_count(), baseline_use_count);
        drop(cache);
        assert_eq!(iosurface.use_count(), baseline_use_count);
        Ok(())
    }

    #[test]
    fn cache_evicts_oldest_surface_at_capacity() -> Result<(), String> {
        let mut cache = IOSurfaceCache::new();
        let mut surface_ids = Vec::new();
        for _ in 0..=super::MAX_CACHED_SURFACES {
            let iosurface = build_local_iosurface(8, 8)?;
            let imported = super::import_pixel_buffer_from_mach_port(iosurface.create_mach_port())
                .map_err(|error| error.to_string())?;
            surface_ids.push(imported.system_surface_id);
            cache.insert_imported(imported.system_surface_id, imported);
        }

        assert_eq!(cache.cached_surface_count(), super::MAX_CACHED_SURFACES);
        assert!(
            cache
                .hardware_surface_for(surface_ids[0])
                .map_err(|error| error.to_string())?
                .is_none()
        );
        assert!(
            cache
                .hardware_surface_for(surface_ids[super::MAX_CACHED_SURFACES])
                .map_err(|error| error.to_string())?
                .is_some()
        );
        assert!(!cache.surface_ids().contains(&surface_ids[0]));
        Ok(())
    }

    #[test]
    fn cache_stays_bounded_while_gpu_backings_are_active() -> Result<(), String> {
        let mut cache = IOSurfaceCache::new();
        let mut surface_ids = Vec::new();
        let mut native_surfaces = Vec::new();
        let mut active_backings = Vec::new();
        for _ in 0..super::MAX_CACHED_SURFACES {
            let iosurface = build_local_iosurface(8, 8)?;
            let imported = super::import_pixel_buffer_from_mach_port(iosurface.create_mach_port())
                .map_err(|error| error.to_string())?;
            surface_ids.push(imported.system_surface_id);
            native_surfaces.push(iosurface);
            cache.insert_imported(imported.system_surface_id, imported);
            active_backings.push(
                cache
                    .hardware_surface_for(*surface_ids.last().ok_or("surface id was missing")?)
                    .map_err(|error| error.to_string())?
                    .ok_or("active surface backing was missing")?,
            );
        }
        let overflow = build_local_iosurface(8, 8)?;
        let overflow = super::import_pixel_buffer_from_mach_port(overflow.create_mach_port())
            .map_err(|error| error.to_string())?;
        let overflow_id = overflow.system_surface_id;
        cache.insert_imported(overflow_id, overflow);

        assert_eq!(cache.cached_surface_count(), super::MAX_CACHED_SURFACES);
        assert!(!cache.surface_ids().contains(&surface_ids[0]));
        assert_eq!(active_backings[0].pixel_buffer().get_width(), 8);
        assert!(
            cache
                .hardware_surface_for(surface_ids[0])
                .map_err(|error| error.to_string())?
                .is_none()
        );
        assert!(
            cache.hardware_surface_for(overflow_id).map_err(|error| error.to_string())?.is_some()
        );

        cache
            .import(native_surfaces[0].create_mach_port(), surface_ids[0])
            .map_err(|error| error.to_string())?;
        let reimported = cache
            .hardware_surface_for(surface_ids[0])
            .map_err(|error| error.to_string())?
            .ok_or("reimported surface backing was missing")?;

        assert_eq!(cache.cached_surface_count(), super::MAX_CACHED_SURFACES);
        assert!(cache.surface_ids().contains(&surface_ids[0]));
        assert!(!Arc::ptr_eq(&active_backings[0], &reimported));
        Ok(())
    }
}
