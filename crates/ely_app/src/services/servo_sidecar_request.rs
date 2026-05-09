use ely_domain::{ProfileId, UrlText};

use super::ProfileDataMode;

#[derive(Clone, Debug)]
pub struct SidecarSnapshotRequest {
    pub(in crate::services) url: UrlText,
    pub(in crate::services) profile_id: ProfileId,
    pub(in crate::services) profile_data_mode: ProfileDataMode,
    pub(in crate::services) width: u32,
    pub(in crate::services) height: u32,
    pub(in crate::services) scroll_x: i32,
    pub(in crate::services) scroll_y: i32,
    pub(in crate::services) click_point: Option<SidecarClickPoint>,
    pub(in crate::services) typed_text: Option<String>,
}

impl SidecarSnapshotRequest {
    #[must_use]
    pub fn new(url: UrlText, profile_id: ProfileId, width: u32, height: u32) -> Self {
        Self {
            url,
            profile_id,
            profile_data_mode: ProfileDataMode::Persistent,
            width,
            height,
            scroll_x: 0,
            scroll_y: 0,
            click_point: None,
            typed_text: None,
        }
    }

    #[must_use]
    pub fn with_profile_data_mode(mut self, profile_data_mode: ProfileDataMode) -> Self {
        self.profile_data_mode = profile_data_mode;
        self
    }

    #[must_use]
    pub fn with_scroll_offset(mut self, scroll_x: i32, scroll_y: i32) -> Self {
        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
        self
    }

    #[must_use]
    pub fn with_click_point(mut self, x: u32, y: u32) -> Self {
        self.click_point = Some(SidecarClickPoint { x, y });
        self
    }

    #[must_use]
    pub fn with_typed_text(mut self, typed_text: String) -> Self {
        self.typed_text = Some(typed_text);
        self
    }

    #[cfg(test)]
    pub(crate) fn typed_text_for_test(&self) -> Option<&str> {
        self.typed_text.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn profile_id_for_test(&self) -> &ProfileId {
        &self.profile_id
    }

    #[cfg(test)]
    pub(crate) fn profile_data_mode_for_test(&self) -> ProfileDataMode {
        self.profile_data_mode
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::services) struct SidecarClickPoint {
    pub(in crate::services) x: u32,
    pub(in crate::services) y: u32,
}
