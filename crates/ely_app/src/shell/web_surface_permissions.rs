use std::collections::HashMap;

use ely_browser_core::BrowserCore;
use ely_domain::{BrowserTab, SiteOrigin, SitePermissionDecision, SitePermissionFeature};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WebSurfaceSitePermissionState {
    Decision(SitePermissionDecision),
    TransferredAllowOnce,
}

impl WebSurfaceSitePermissionState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Decision(decision) => decision.as_str(),
            Self::TransferredAllowOnce => "transferred-allow-once",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebSurfaceSitePermission {
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    state: WebSurfaceSitePermissionState,
    revision: u64,
}

impl WebSurfaceSitePermission {
    pub(super) fn new(
        origin: SiteOrigin,
        feature: SitePermissionFeature,
        state: WebSurfaceSitePermissionState,
        revision: u64,
    ) -> Self {
        Self { origin, feature, state, revision }
    }

    pub(super) fn origin(&self) -> &SiteOrigin {
        &self.origin
    }

    pub(super) fn feature(&self) -> SitePermissionFeature {
        self.feature
    }

    pub(super) fn state(&self) -> WebSurfaceSitePermissionState {
        self.state
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
}

pub(super) fn web_surface_site_permissions_for_core_tab(
    core: &BrowserCore,
    tab: &BrowserTab,
) -> Vec<WebSurfaceSitePermission> {
    let mut latest = HashMap::new();
    for event in core.site_permission_audit_events_for_profile(tab.profile_id()) {
        let key = (event.origin().clone(), event.feature());
        let revision = latest.entry(key).or_insert(0_u64);
        *revision = revision.saturating_add(1);
    }

    let mut permissions = Vec::new();
    for entry in core.site_permissions_for_profile(tab.profile_id()) {
        let key = (entry.origin().clone(), entry.feature());
        let revision = latest.remove(&key).unwrap_or(0);
        permissions.push(WebSurfaceSitePermission::new(
            entry.origin().clone(),
            entry.feature(),
            WebSurfaceSitePermissionState::Decision(entry.decision()),
            revision,
        ));
    }
    for (entry, grant_revision) in core.transferred_site_permissions_for_profile(tab.profile_id()) {
        let key = (entry.origin().clone(), entry.feature());
        latest.remove(&key);
        permissions.push(WebSurfaceSitePermission::new(
            entry.origin().clone(),
            entry.feature(),
            WebSurfaceSitePermissionState::TransferredAllowOnce,
            grant_revision,
        ));
    }
    permissions.sort_by(|left, right| {
        (left.origin().as_str(), left.feature().as_str())
            .cmp(&(right.origin().as_str(), right.feature().as_str()))
    });
    permissions
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ely_browser_core::{BrowserCore, BrowserSnapshot, InitialBrowserConfig};
    use ely_domain::{
        BrowserTab, SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText,
    };

    use super::{WebSurfaceSitePermissionState, web_surface_site_permissions_for_core_tab};

    #[test]
    fn includes_matching_profile_and_origin_permissions() -> Result<(), Box<dyn Error>> {
        let mut core = browser_core_for("https://example.com/page")?;
        let origin = SiteOrigin::parse("https://example.com")?;
        core.set_site_permission(
            origin,
            SitePermissionFeature::Camera,
            SitePermissionDecision::AllowAlways,
        )?;

        let snapshot = core.snapshot()?;
        let tab = active_tab(&snapshot)?;
        let permissions = web_surface_site_permissions_for_core_tab(&core, tab);

        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].origin().as_str(), "https://example.com");
        assert_eq!(permissions[0].feature(), SitePermissionFeature::Camera);
        assert_eq!(
            permissions[0].state(),
            WebSurfaceSitePermissionState::Decision(SitePermissionDecision::AllowAlways),
        );
        Ok(())
    }

    #[test]
    fn includes_the_complete_profile_snapshot() -> Result<(), Box<dyn Error>> {
        let mut core = browser_core_for("https://example.com/page")?;
        core.set_site_permission(
            SiteOrigin::parse("https://other.test")?,
            SitePermissionFeature::Location,
            SitePermissionDecision::AllowOnce,
        )?;

        let snapshot = core.snapshot()?;
        let tab = active_tab(&snapshot)?;

        let permissions = web_surface_site_permissions_for_core_tab(&core, tab);
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].origin().as_str(), "https://other.test");
        assert_eq!(permissions[0].revision(), 1);
        assert_eq!(
            permissions[0].state(),
            WebSurfaceSitePermissionState::Decision(SitePermissionDecision::AllowOnce),
        );

        let profile_id = tab.profile_id().clone();
        let origin = SiteOrigin::parse("https://other.test")?;
        assert!(core.transfer_site_permission_once(
            &profile_id,
            &origin,
            SitePermissionFeature::Location,
            1,
        )?);
        let transferred = web_surface_site_permissions_for_core_tab(&core, tab);
        assert_eq!(transferred[0].state(), WebSurfaceSitePermissionState::TransferredAllowOnce);
        assert_eq!(transferred[0].revision(), 1);

        core.revoke_site_permission(&origin, SitePermissionFeature::Location)?;
        let revoked = web_surface_site_permissions_for_core_tab(&core, tab);
        assert!(revoked.is_empty());
        Ok(())
    }

    fn browser_core_for(url: &str) -> Result<BrowserCore, Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            space_color_hex: 0xf54e00,
            profile_name: "Default".to_string(),
            profile_color_hex: 0x26251e,
            profile_kind: ely_domain::ProfileKind::Standard,
            profile_id: None,
            new_tab_destination: Default::default(),
        })?;
        core.open_tab(UrlText::parse(url)?);
        Ok(core)
    }

    fn active_tab(snapshot: &BrowserSnapshot) -> Result<&BrowserTab, Box<dyn Error>> {
        snapshot
            .tabs
            .iter()
            .find(|tab| tab.id() == &snapshot.active_tab_id)
            .ok_or_else(|| std::io::Error::other("active tab missing").into())
    }
}
