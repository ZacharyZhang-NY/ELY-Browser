use std::time::Instant;

use ely_domain::BrowserTab;

use crate::services::servo_live::ServoLiveSitePermission;

use super::{
    web_surface_cadence::WebSurfaceInputKind,
    web_surface_geometry::{WebSurfaceClickPoint, WebSurfaceScrollDelta, WebSurfaceSize},
    web_surface_permissions::WebSurfaceSitePermission,
    web_surface_state::WebSurfacePendingInput,
};

pub(super) fn scroll_wire_fields(
    delta: Option<WebSurfaceScrollDelta>,
    point: Option<WebSurfaceClickPoint>,
) -> Result<(i32, i32, Option<u32>, Option<u32>), String> {
    match delta {
        Some(delta) => {
            let point = point
                .ok_or_else(|| "Servo scroll input is missing a viewport point".to_string())?;
            Ok((delta.x(), delta.y(), Some(point.x()), Some(point.y())))
        }
        None => Ok((0, 0, None, None)),
    }
}

pub(super) fn input_requests_history_navigation(input: &WebSurfacePendingInput) -> bool {
    input.click_point.is_some()
        || input.typed_text.as_deref().is_some_and(|text| text.contains('\n'))
}

pub(super) fn pending_input_kind(input: &WebSurfacePendingInput) -> WebSurfaceInputKind {
    if input.scroll_delta.is_some() {
        WebSurfaceInputKind::Scroll
    } else if input.click_point.is_some() {
        WebSurfaceInputKind::Click
    } else if input.typed_text.is_some() {
        WebSurfaceInputKind::Text
    } else if input.hover_point.is_some() {
        WebSurfaceInputKind::Hover
    } else {
        WebSurfaceInputKind::Idle
    }
}

pub(super) fn log_ensure_submitted(
    tab: &BrowserTab,
    size: WebSurfaceSize,
    input_kind: &'static str,
    enqueued_at: Option<Instant>,
    started_loading: bool,
) {
    if input_kind == "idle" && !started_loading {
        return;
    }
    let queued_us = enqueued_at.map(|started_at| started_at.elapsed().as_micros());
    tracing::info!(
        target: "ely::web_surface::latency",
        tab_id = %tab.id().as_str(),
        url = %tab.url().as_str(),
        input_kind,
        queued_us,
        started_loading,
        width = size.width,
        height = size.height,
        device_pixel_ratio = size.device_pixel_ratio_f32(),
        "web_surface_ensure_submitted",
    );
}

impl From<&WebSurfaceSitePermission> for ServoLiveSitePermission {
    fn from(permission: &WebSurfaceSitePermission) -> Self {
        Self::new(
            permission.origin().as_str(),
            permission.feature().as_str(),
            permission.decision(),
        )
    }
}
