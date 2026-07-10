use serde::{Deserialize, Serialize};

use super::ServoLiveSitePermission;

pub(super) const LIVE_PROTOCOL_VERSION: u32 = 3;
pub(super) const MAX_FRAME_DIMENSION: u32 = 16_384;
pub(super) const MAX_FRAME_BYTE_COUNT: usize = 256 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum LiveRequest {
    Handshake {
        protocol_version: u32,
    },
    Ensure {
        tab_id: String,
        profile_id: String,
        url: String,
        width: u32,
        height: u32,
        page_zoom_percent: u16,
        device_pixel_ratio: f32,
        scroll_delta_x: i32,
        scroll_delta_y: i32,
        scroll_point_x: Option<u32>,
        scroll_point_y: Option<u32>,
        click_x: Option<u32>,
        click_y: Option<u32>,
        hover_x: Option<u32>,
        hover_y: Option<u32>,
        typed_text: Option<String>,
        site_permission_generation: u64,
        site_permissions: Vec<ServoLiveSitePermission>,
        ready_surface_ids: Vec<u64>,
        pending_surface_ids: Vec<u64>,
    },
    Poll {
        tab_id: String,
        ready_surface_ids: Vec<u64>,
        pending_surface_ids: Vec<u64>,
    },
    Close {
        tab_id: String,
    },
    Shutdown,
}

#[derive(Deserialize)]
pub(super) struct LiveResponse {
    #[serde(default)]
    pub(super) protocol_version: Option<u32>,
    pub(super) error: Option<String>,
    pub(super) frame: Option<LiveFrameReport>,
    #[serde(default)]
    pub(super) surface_handle: Option<LiveSurfaceHandle>,
    #[serde(default)]
    pub(super) current_surface_id: Option<u64>,
    #[serde(default)]
    pub(super) permission_consumptions: Vec<LivePermissionConsumption>,
}

#[derive(Deserialize)]
pub(super) struct LivePermissionConsumption {
    pub(super) profile_id: String,
    pub(super) origin: String,
    pub(super) feature: String,
    pub(super) grant_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) struct LiveSurfaceHandle {
    pub(super) mach_port_name: u32,
    pub(super) surface_id: u64,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Deserialize)]
pub(super) struct LiveFrameReport {
    pub(super) loaded_url: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: String,
    pub(super) width: u32,
    pub(super) height: u32,
    #[serde(default = "default_device_pixel_ratio")]
    pub(super) device_pixel_ratio: f32,
    #[serde(default)]
    pub(super) css_viewport_width: u32,
    #[serde(default)]
    pub(super) css_viewport_height: u32,
    pub(super) rgba_byte_count: usize,
    pub(super) pixels_changed: bool,
    #[cfg(all(test, feature = "live-site-smoke"))]
    #[serde(default)]
    pub(super) non_white_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    #[serde(default)]
    pub(super) content_pixel_count: u64,
    #[cfg(all(test, feature = "live-site-smoke"))]
    #[serde(default)]
    pub(super) sample_hash: u64,
}

fn default_device_pixel_ratio() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{LIVE_PROTOCOL_VERSION, LiveRequest, ServoLiveSitePermission};

    #[test]
    fn handshake_request_serializes_protocol_version() -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(LiveRequest::Handshake {
            protocol_version: LIVE_PROTOCOL_VERSION,
        })?;

        assert_eq!(value, json!({"type": "handshake", "protocol_version": LIVE_PROTOCOL_VERSION}));
        Ok(())
    }

    #[test]
    fn close_request_serializes_to_wire() -> Result<(), serde_json::Error> {
        let value =
            serde_json::to_value(LiveRequest::Close { tab_id: "tab-live-close".to_string() })?;

        assert_eq!(value, json!({"type": "close", "tab_id": "tab-live-close"}));
        Ok(())
    }

    #[test]
    fn poll_serializes_hardware_surface_states() -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(LiveRequest::Poll {
            tab_id: "tab-live".to_string(),
            ready_surface_ids: vec![7, 11],
            pending_surface_ids: vec![13],
        })?;

        assert_eq!(
            value,
            json!({
                "type": "poll",
                "tab_id": "tab-live",
                "ready_surface_ids": [7, 11],
                "pending_surface_ids": [13]
            })
        );
        Ok(())
    }

    #[test]
    fn site_permission_serializes_revision() -> Result<(), serde_json::Error> {
        let permission =
            ServoLiveSitePermission::new("https://example.com", "camera", "allow-once", 7);

        assert_eq!(
            serde_json::to_value(permission)?,
            json!({
                "origin": "https://example.com",
                "feature": "camera",
                "state": "allow-once",
                "revision": 7,
            }),
        );
        Ok(())
    }
}
