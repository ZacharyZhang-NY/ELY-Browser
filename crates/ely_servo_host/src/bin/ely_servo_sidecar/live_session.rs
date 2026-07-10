use std::collections::{HashMap, hash_map::Entry};

use ely_domain::{ColorScheme, ProfileId, TabId, validate_zoom_percent};
use ely_servo_host::{
    ColorSchemeRequest, HidpiScaleRequest, KeyboardTextRequest, MouseClickRequest,
    MouseHoverRequest, PageZoomRequest, PermissionDecision, PermissionSnapshotEntry,
    PermissionSnapshotRequest, PermissionSnapshotState, RenderedFrame, ResizeRequest,
    ScrollRequest, ServoHost, ServoSurfaceSize, SoftwareServoHost,
};

use super::live_protocol::{LiveSidecarError, LiveSitePermission};

pub(super) struct LiveSession {
    pub(super) webview_id: ely_domain::WebViewId,
    pub(super) requested_url: String,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    hidpi_scale_milli: u32,
    pub(super) last_frame: Option<RenderedFrame>,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) last_surface: Option<ely_servo_host::IOSurfaceIdentity>,
    #[cfg(all(feature = "hardware-render", target_os = "macos"))]
    pub(super) last_surface_ready: bool,
}

impl LiveSession {
    fn new(webview_id: ely_domain::WebViewId) -> Self {
        Self {
            webview_id,
            requested_url: String::new(),
            width: 0,
            height: 0,
            page_zoom_percent: ely_domain::DEFAULT_ZOOM_PERCENT,
            hidpi_scale_milli: 0,
            last_frame: None,
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            last_surface: None,
            #[cfg(all(feature = "hardware-render", target_os = "macos"))]
            last_surface_ready: false,
        }
    }

    pub(super) fn device_pixel_ratio(&self) -> f32 {
        self.hidpi_scale_milli as f32 / 1_000.0
    }

    pub(super) fn clear_presented_frame(&mut self) {
        self.last_frame = None;
        #[cfg(all(feature = "hardware-render", target_os = "macos"))]
        {
            self.last_surface = None;
            self.last_surface_ready = false;
        }
    }
}

pub(super) fn bind_profile(
    active_profile: &mut Option<ProfileId>,
    profile: &ProfileId,
) -> Result<(), LiveSidecarError> {
    match active_profile {
        Some(expected) if expected != profile => Err(LiveSidecarError::ProfileMismatch {
            expected: expected.to_string(),
            actual: profile.to_string(),
        }),
        Some(_) => Ok(()),
        None => {
            *active_profile = Some(profile.clone());
            Ok(())
        }
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
    match sessions.entry(key) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => {
            let webview_id = host.create_webview_with_size(
                tab_id.clone(),
                profile_id.clone(),
                ServoSurfaceSize::new(width, height),
            )?;
            Ok(entry.insert(LiveSession::new(webview_id)))
        }
    }
}

pub(super) fn apply_layout(
    host: &mut SoftwareServoHost,
    session: &mut LiveSession,
    width: u32,
    height: u32,
    page_zoom_percent: u16,
    device_pixel_ratio: f32,
) -> Result<(), LiveSidecarError> {
    let page_zoom_percent = validate_zoom_percent(page_zoom_percent)?;
    let hidpi_scale_milli = encode_hidpi_scale_milli(device_pixel_ratio);
    if session.hidpi_scale_milli != hidpi_scale_milli {
        session.clear_presented_frame();
        host.set_hidpi_scale(HidpiScaleRequest {
            webview_id: session.webview_id.clone(),
            scale_factor: hidpi_scale_milli as f32 / 1_000.0,
        })?;
        session.hidpi_scale_milli = hidpi_scale_milli;
    }
    if session.width != width || session.height != height {
        session.clear_presented_frame();
        host.resize(ResizeRequest { webview_id: session.webview_id.clone(), width, height })?;
        session.width = width;
        session.height = height;
    }
    if session.page_zoom_percent != page_zoom_percent {
        session.clear_presented_frame();
        host.set_page_zoom(PageZoomRequest {
            webview_id: session.webview_id.clone(),
            zoom_factor: f32::from(page_zoom_percent) / 100.0,
        })?;
        session.page_zoom_percent = page_zoom_percent;
    }
    Ok(())
}

pub(super) fn apply_permissions(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    profile_id: &ProfileId,
    generation: u64,
    permissions: Vec<LiveSitePermission>,
) -> Result<(), LiveSidecarError> {
    let entries = permissions
        .into_iter()
        .map(|permission| {
            let state = match permission.state.as_str() {
                "allow-once" => PermissionSnapshotState::Decision(PermissionDecision::AllowOnce),
                "allow-always" => {
                    PermissionSnapshotState::Decision(PermissionDecision::AllowAlways)
                }
                "deny-always" => PermissionSnapshotState::Decision(PermissionDecision::DenyAlways),
                "transferred-allow-once" => PermissionSnapshotState::TransferredAllowOnce,
                value => {
                    return Err(ely_domain::DomainError::InvalidSitePermissionDecision {
                        value: value.to_string(),
                    }
                    .into());
                }
            };
            Ok(PermissionSnapshotEntry {
                origin: ely_domain::SiteOrigin::parse(permission.origin)?,
                feature: ely_domain::SitePermissionFeature::parse(&permission.feature)?,
                state,
                revision: permission.revision,
            })
        })
        .collect::<Result<Vec<_>, LiveSidecarError>>()?;
    host.replace_permissions(PermissionSnapshotRequest {
        webview_id: session.webview_id.clone(),
        profile_id: profile_id.clone(),
        generation,
        entries,
    })
    .map_err(LiveSidecarError::from)
}

pub(super) fn apply_color_scheme(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    color_scheme: ColorScheme,
) -> Result<(), LiveSidecarError> {
    host.set_color_scheme(ColorSchemeRequest {
        webview_id: session.webview_id.clone(),
        color_scheme,
    })
    .map_err(LiveSidecarError::from)
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

pub(super) fn apply_input(
    host: &mut SoftwareServoHost,
    session: &LiveSession,
    input: LiveInput,
) -> Result<(), LiveSidecarError> {
    if input.scroll_delta_x != 0 || input.scroll_delta_y != 0 {
        let (point_x, point_y) =
            paired_point("scroll input", input.scroll_point_x, input.scroll_point_y)?;
        host.scroll(ScrollRequest {
            webview_id: session.webview_id.clone(),
            delta_x: input.scroll_delta_x,
            delta_y: input.scroll_delta_y,
            point_x,
            point_y,
        })?;
    }
    if input.hover_x.is_some() || input.hover_y.is_some() {
        let (x, y) = paired_point("hover input", input.hover_x, input.hover_y)?;
        host.hover(MouseHoverRequest { webview_id: session.webview_id.clone(), x, y })?;
    }
    if input.click_x.is_some() || input.click_y.is_some() {
        let (x, y) = paired_point("click input", input.click_x, input.click_y)?;
        host.click(MouseClickRequest { webview_id: session.webview_id.clone(), x, y })?;
    }
    if let Some(text) = input.typed_text {
        host.type_text(KeyboardTextRequest { webview_id: session.webview_id.clone(), text })?;
    }
    Ok(())
}

fn paired_point(
    input: &'static str,
    x: Option<u32>,
    y: Option<u32>,
) -> Result<(u32, u32), LiveSidecarError> {
    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(LiveSidecarError::IncompletePoint { input }),
    }
}

fn encode_hidpi_scale_milli(scale: f32) -> u32 {
    if scale.is_finite() && scale > 0.0 {
        (scale * 1_000.0).round().clamp(500.0, 5_000.0) as u32
    } else {
        1_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_binds_to_first_profile() -> Result<(), LiveSidecarError> {
        let first = ProfileId::new();
        let second = ProfileId::new();
        let mut active = None;

        bind_profile(&mut active, &first)?;
        assert!(matches!(
            bind_profile(&mut active, &second),
            Err(LiveSidecarError::ProfileMismatch { .. })
        ));
        Ok(())
    }
}
