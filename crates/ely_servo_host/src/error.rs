use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature, WebViewId};
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

    #[error("duplicate permission snapshot entry for {profile_id} {origin:?} {feature:?}")]
    DuplicatePermissionSnapshotEntry {
        profile_id: ProfileId,
        origin: SiteOrigin,
        feature: SitePermissionFeature,
    },

    #[error("servo runtime is already started in this process")]
    RuntimeAlreadyStarted,

    #[error("servo rendering context is unavailable")]
    RenderingContextUnavailable,

    #[error("servo rendering context could not be made current")]
    RenderingContextNotCurrent,

    #[error(
        "hardware rendering requires the `hardware-render` feature; rebuild with \
         --features servo-engine,hardware-render"
    )]
    HardwareRenderUnavailable,

    #[error("servo rendered frame is unavailable")]
    RenderedFrameUnavailable,

    #[error("servo hardware surface is unavailable for {id}")]
    HardwareSurfaceUnavailable { id: WebViewId },
}
