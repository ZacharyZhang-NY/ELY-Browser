//! Cross-process IOSurface descriptor types.
//!
//! These wire types live outside `hardware_rendering_context` (which
//! is hardware-render + macOS gated) so the sidecar's JSON protocol
//! can carry an `Option<IOSurfaceHandle>` regardless of feature
//! flags. The receiver always knows how to parse the field; if no
//! sender ever populates it (software-only build), it's just `None`
//! on every frame.
//!
//! Minting an [`IOSurfaceHandle`] requires a hardware surfman context
//! and a macOS host. That part lives in
//! [`crate::hardware_rendering_context`].

/// Cross-process handle to a hardware surface: the receiving process
/// rebuilds an `IOSurfaceRef` from `mach_port_name` and imports it as
/// a Metal texture without copying pixels.
///
/// `surface_id` is the stable surfman `SurfaceID` (a pointer-shaped
/// `usize` widened to `u64` for the wire). It lets the receiver dedup:
/// when two consecutive frames carry the same `surface_id` the
/// imported `MTLTexture` is reused without re-importing. `width` and
/// `height` are reported in surface pixels (post-DPR).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "servo-engine", derive(serde::Serialize, serde::Deserialize))]
pub struct IOSurfaceHandle {
    pub mach_port_name: u32,
    pub surface_id: u64,
    pub width: u32,
    pub height: u32,
}

/// Identity-only peek of the currently bound IOSurface. Distinguishes
/// "same surface as last frame" from "resize/swap rotated to a new
/// surface" without minting a fresh mach port (mach ports are a scarce
/// kernel resource and `IOSurfaceCreateMachPort` is not cheap).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IOSurfaceIdentity {
    pub surface_id: u64,
    pub width: u32,
    pub height: u32,
}
