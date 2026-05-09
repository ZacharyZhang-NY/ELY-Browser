use ely_domain::{ProfileId, WebViewId};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ServoHostError {
    #[error("webview not found: {id}")]
    WebViewNotFound { id: WebViewId },

    #[error("invalid navigation url: {value}")]
    InvalidNavigationUrl { value: String },

    #[error("permission request missing profile context")]
    MissingProfileContext,

    #[error("permission profile mismatch for {webview_id}: expected {expected}, got {actual}")]
    PermissionProfileMismatch { webview_id: WebViewId, expected: ProfileId, actual: ProfileId },

    #[error("servo runtime is already started in this process")]
    RuntimeAlreadyStarted,

    #[error("servo rendering context is unavailable")]
    RenderingContextUnavailable,

    #[error("servo rendering context could not be made current")]
    RenderingContextNotCurrent,

    #[error("servo rendered frame is unavailable")]
    RenderedFrameUnavailable,

    #[error("servo screenshot capture timed out for {id}")]
    ScreenshotTimedOut { id: WebViewId },

    #[error("servo screenshot capture failed: {reason}")]
    ScreenshotUnavailable { reason: String },
}
