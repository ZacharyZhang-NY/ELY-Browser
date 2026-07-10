//! Cross-process IOSurface descriptors used by the macOS hardware path.

/// A send right for importing the current IOSurface in another process.
///
/// `surface_id` is the system IOSurface ID. The receiver owns the transferred
/// Mach send right and must deallocate it after importing the surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "servo-engine", derive(serde::Deserialize, serde::Serialize))]
pub struct IOSurfaceHandle {
    pub mach_port_name: u32,
    pub surface_id: u64,
    pub width: u32,
    pub height: u32,
}

/// Stable identity of a presented IOSurface without creating a Mach port.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IOSurfaceIdentity {
    pub surface_id: u64,
    pub width: u32,
    pub height: u32,
}

impl IOSurfaceIdentity {
    #[must_use]
    pub fn from_handle(handle: IOSurfaceHandle) -> Self {
        Self { surface_id: handle.surface_id, width: handle.width, height: handle.height }
    }
}
