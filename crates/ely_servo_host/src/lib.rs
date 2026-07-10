mod error;
#[cfg(feature = "hardware-render")]
mod hardware_rendering_context;
mod host;
mod iosurface_handle;
#[cfg(feature = "servo-engine")]
mod keyboard;
#[cfg(feature = "servo-engine")]
mod runtime;
#[cfg(feature = "servo-engine")]
mod runtime_input;
#[cfg(feature = "servo-engine")]
mod runtime_permissions;
#[cfg(feature = "servo-engine")]
mod runtime_waker;
#[cfg(feature = "servo-engine")]
mod runtime_webview;

pub const MAX_REQUEST_LINE_BYTES: usize = 1024 * 1024;

pub use error::ServoHostError;
#[cfg(feature = "hardware-render")]
pub use hardware_rendering_context::HardwareOffscreenContext;
pub use host::{
    ConsumedPermission, HidpiScaleRequest, KeyboardTextRequest, MAX_PAGE_TITLE_BYTES,
    MouseClickRequest, MouseDragRequest, MouseHoverRequest, NavigationRequest, PageZoomRequest,
    PermissionDecision, PermissionSnapshotEntry, PermissionSnapshotRequest,
    PermissionSnapshotState, RenderedFrame, RenderedFrameSummary, ResizeRequest, ScrollRequest,
    ServoHost, TouchTapRequest, WebViewSnapshot, WebViewSnapshotPending, WebViewState,
};
pub use iosurface_handle::{IOSurfaceHandle, IOSurfaceIdentity};
#[cfg(feature = "servo-engine")]
pub use runtime::{RenderingContextKind, ServoSurfaceSize, SoftwareServoHost};
