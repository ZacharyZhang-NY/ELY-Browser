mod error;
#[cfg(feature = "hardware-render")]
mod hardware_rendering_context;
mod host;
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

pub use error::ServoHostError;
#[cfg(feature = "hardware-render")]
pub use hardware_rendering_context::HardwareOffscreenContext;
#[cfg(all(feature = "hardware-render", target_os = "macos"))]
pub use hardware_rendering_context::IOSurfaceHandle;
pub use host::{
    KeyboardTextRequest, MouseClickRequest, MouseDragRequest, MouseHoverRequest,
    NavigationRequest, PageZoomRequest, PermissionDecision, PermissionRequest, RenderedFrame,
    RenderedFrameSummary, ResizeRequest, ScreenshotRequest, ScrollRequest, ServoHost,
    TouchTapRequest, WebViewSnapshot, WebViewState,
};
#[cfg(feature = "servo-engine")]
pub use runtime::{RenderingContextKind, ServoSurfaceSize, SoftwareServoHost};
