use ely_domain::{
    DEFAULT_ZOOM_PERCENT, ProfileId, SiteOrigin, SitePermissionDecision, SitePermissionFeature,
    UrlText,
};

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
    pub(in crate::services) page_zoom_percent: u16,
    pub(in crate::services) click_point: Option<SidecarClickPoint>,
    pub(in crate::services) typed_text: Option<String>,
    pub(in crate::services) site_permissions: Vec<SidecarSitePermission>,
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
            page_zoom_percent: DEFAULT_ZOOM_PERCENT,
            click_point: None,
            typed_text: None,
            site_permissions: Vec::new(),
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
    pub fn with_page_zoom_percent(mut self, page_zoom_percent: u16) -> Self {
        self.page_zoom_percent = page_zoom_percent;
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

    #[must_use]
    pub fn with_site_permissions(mut self, site_permissions: Vec<SidecarSitePermission>) -> Self {
        self.site_permissions = site_permissions;
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

    #[cfg(test)]
    pub(crate) fn page_zoom_percent_for_test(&self) -> u16 {
        self.page_zoom_percent
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::services) struct SidecarClickPoint {
    pub(in crate::services) x: u32,
    pub(in crate::services) y: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidecarSitePermission {
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    decision: SitePermissionDecision,
}

impl SidecarSitePermission {
    #[must_use]
    pub fn new(
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        decision: SitePermissionDecision,
    ) -> Self {
        Self { origin, feature, decision }
    }

    pub(in crate::services) fn to_arg(&self) -> String {
        serde_json::json!({
            "origin": self.origin.as_str(),
            "feature": self.feature.as_str(),
            "decision": self.decision.as_str(),
        })
        .to_string()
    }

    #[cfg(test)]
    pub(crate) fn origin(&self) -> &SiteOrigin {
        &self.origin
    }

    #[cfg(test)]
    pub(crate) fn feature(&self) -> SitePermissionFeature {
        self.feature
    }

    #[cfg(test)]
    pub(crate) fn decision(&self) -> SitePermissionDecision {
        self.decision
    }
}
