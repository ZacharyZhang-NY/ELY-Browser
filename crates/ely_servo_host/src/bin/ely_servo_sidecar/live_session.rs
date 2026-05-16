use std::collections::HashMap;

use ely_domain::{DEFAULT_ZOOM_PERCENT, ProfileId, TabId};
use ely_servo_host::{
    KeyboardTextRequest, MouseClickRequest, MouseHoverRequest, PageZoomRequest, PermissionDecision,
    PermissionRequest, ResizeRequest, ScrollRequest, ServoHost, ServoSurfaceSize,
    SoftwareServoHost,
};

use super::live_protocol::{LiveSidecarError, LiveSitePermission};

#[derive(Clone)]
pub(super) struct LiveSession {
    pub(super) webview_id: ely_domain::WebViewId,
    pub(super) requested_url: String,
    pub(super) width: u32,
    pub(super) height: u32,
    page_zoom_percent: u16,
    /// Last hidpi factor pushed to Servo, encoded as `(scale × 1000)`.
    /// Stored as a u32 so equality is cheap and stable across the
    /// f32 jitter that JSON parsing can introduce. Init to 0 so the
    /// first apply_layout call always pushes a real value.
    hidpi_scale_milli: u32,
    pub(super) scroll_x: i32,
    pub(super) scroll_y: i32,
    pub(super) awaiting_visible_frame: bool,
    /// Sticky for the lifetime of a single URL: flipped to `true`
    /// the first time `poll_frame` sees a paint with real content
    /// (non-white, non-empty) and reset to `false` on every navigate.
    /// After it's `true`, the visible-content gate stops gating:
    /// scroll/click/hover/type all return on the first
    /// `has_pending_frame=true` (~3 ms) instead of waiting the full
    /// `LIVE_FRAME_WAIT_TIMEOUT`. The gate stays armed for the
    /// initial paint of each new URL so loading frames are still
    /// skipped.
    pub(super) ever_visible_frame: bool,
}

impl LiveSession {
    fn new(webview_id: ely_domain::WebViewId, _width: u32, _height: u32) -> Self {
        Self {
            webview_id,
            requested_url: String::new(),
            width: 0,
            height: 0,
            page_zoom_percent: DEFAULT_ZOOM_PERCENT,
            hidpi_scale_milli: 0,
            scroll_x: 0,
            scroll_y: 0,
            awaiting_visible_frame: false,
            ever_visible_frame: false,
        }
    }

    pub(super) fn device_pixel_ratio(&self) -> f32 {
        hidpi_scale_milli_to_f32(self.hidpi_scale_milli)
    }
}

pub(super) fn ensure_session<'a>(
    host: &mut SoftwareServoHost,
    sessions: &'a mut HashMap<String, LiveSession>,
    key: String,
    tab_id: &TabId,
    profile_id: &ProfileId,
    width: u32,
    height: u32,
) -> Result<&'a mut LiveSession, LiveSidecarError> {
    if !sessions.contains_key(&key) {
        let webview_id = host.create_webview_with_size(
            tab_id.clone(),
            profile_id.clone(),
            ServoSurfaceSize::new(width, height),
        )?;
        sessions.insert(key.clone(), LiveSession::new(webview_id, width, height));
    }

    sessions.get_mut(&key).ok_or(LiveSidecarError::SessionUnavailable)
}

pub(super) fn apply_layout(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    device_pixel_ratio: f32,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    // Push the device pixel ratio BEFORE resize. Servo's WebView
    // defaults hidpi to 1.0; without this the first layout treats
    // physical-pixel viewport widths as CSS-pixel widths and the page
    // lays out half the size you'd expect on a Retina display.
    let hidpi_scale_milli = encode_hidpi_scale_milli(device_pixel_ratio);
    if session.hidpi_scale_milli != hidpi_scale_milli {
        host.set_hidpi_scale(ely_servo_host::HidpiScaleRequest {
            webview_id: session.webview_id.clone(),
            scale_factor: hidpi_scale_milli_to_f32(hidpi_scale_milli),
        })?;
        session.hidpi_scale_milli = hidpi_scale_milli;
        changed = true;
    }

    if session.width != width || session.height != height {
        host.resize(ResizeRequest { webview_id: session.webview_id.clone(), width, height })?;
        session.width = width;
        session.height = height;
        changed = true;
    }

    if session.page_zoom_percent != page_zoom_percent {
        host.set_page_zoom(PageZoomRequest {
            webview_id: session.webview_id.clone(),
            zoom_factor: f32::from(page_zoom_percent) / 100.0,
        })?;
        session.page_zoom_percent = page_zoom_percent;
        changed = true;
    }

    Ok(changed)
}

pub(super) fn apply_permissions(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    profile_id: &ProfileId,
    permissions: Vec<LiveSitePermission>,
) -> Result<(), LiveSidecarError> {
    for permission in permissions {
        host.set_permission(
            PermissionRequest {
                webview_id: session.webview_id.clone(),
                profile_id: profile_id.clone(),
                origin: ely_domain::SiteOrigin::parse(permission.origin)?,
                feature: ely_domain::SitePermissionFeature::parse(permission.feature.as_str())?,
            },
            PermissionDecision::from(ely_domain::SitePermissionDecision::parse(
                permission.decision.as_str(),
            )?),
        )?;
    }

    Ok(())
}

pub(super) fn apply_input(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    input: LiveInput,
) -> Result<bool, LiveSidecarError> {
    let mut changed = false;
    if input.scroll_delta_x != 0 || input.scroll_delta_y != 0 {
        let (point_x, point_y) = input.scroll_point()?;
        host.scroll(ScrollRequest {
            webview_id: session.webview_id.clone(),
            delta_x: input.scroll_delta_x,
            delta_y: input.scroll_delta_y,
            point_x,
            point_y,
        })?;
        session.scroll_x = positive_scroll_component(session.scroll_x, input.scroll_delta_x);
        session.scroll_y = positive_scroll_component(session.scroll_y, input.scroll_delta_y);
        changed = true;
    }

    if let (Some(x), Some(y)) = (input.hover_x, input.hover_y) {
        host.hover(MouseHoverRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let (Some(x), Some(y)) = (input.click_x, input.click_y) {
        host.click(MouseClickRequest { webview_id: session.webview_id.clone(), x, y })?;
        changed = true;
    }

    if let Some(text) = input.typed_text {
        host.type_text(KeyboardTextRequest { webview_id: session.webview_id.clone(), text })?;
        changed = true;
    }

    Ok(changed)
}

pub(super) struct LiveInput {
    pub(super) scroll_delta_x: i32,
    pub(super) scroll_delta_y: i32,
    pub(super) scroll_point_x: Option<u32>,
    pub(super) scroll_point_y: Option<u32>,
    pub(super) click_x: Option<u32>,
    pub(super) click_y: Option<u32>,
    pub(super) hover_x: Option<u32>,
    pub(super) hover_y: Option<u32>,
    pub(super) typed_text: Option<String>,
}

impl LiveInput {
    fn scroll_point(&self) -> Result<(u32, u32), LiveSidecarError> {
        let point = match (self.scroll_point_x, self.scroll_point_y) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(LiveSidecarError::IncompleteScrollPoint),
        };
        Ok(point)
    }
}

fn encode_hidpi_scale_milli(scale: f32) -> u32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1_000;
    }
    let scaled = (scale * 1_000.0).round();
    scaled.clamp(500.0, 5_000.0) as u32
}

fn hidpi_scale_milli_to_f32(milli: u32) -> f32 {
    milli as f32 / 1_000.0
}

fn positive_scroll_component(current: i32, delta: i32) -> i32 {
    let value = i64::from(current) + i64::from(delta);
    value.clamp(0, i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_live_session_forces_first_resize_after_hidpi() {
        let session = LiveSession::new(ely_domain::WebViewId::new(), 1280, 720);

        assert_ne!(
            session.width, 1280,
            "first apply_layout must resize after hidpi has been pushed",
        );
        assert_ne!(
            session.height, 720,
            "first apply_layout must resize after hidpi has been pushed",
        );
    }
}
