use ely_browser_core::BrowserSnapshot;
use ely_domain::{BrowserTab, SiteOrigin};

use crate::services::servo_sidecar::SidecarSitePermission;

pub(super) fn sidecar_site_permissions_for_tab(
    tab: &BrowserTab,
    snapshot: &BrowserSnapshot,
) -> Vec<SidecarSitePermission> {
    let Ok(Some(origin)) = SiteOrigin::from_url(tab.url()) else {
        return Vec::new();
    };

    snapshot
        .site_permissions
        .iter()
        .filter(|entry| entry.profile_id() == tab.profile_id())
        .filter(|entry| entry.origin() == &origin)
        .map(|entry| {
            SidecarSitePermission::new(entry.origin().clone(), entry.feature(), entry.decision())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ely_browser_core::{BrowserCore, BrowserSnapshot, InitialBrowserConfig};
    use ely_domain::{
        BrowserTab, SiteOrigin, SitePermissionDecision, SitePermissionFeature, UrlText,
    };

    use super::sidecar_site_permissions_for_tab;

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
        let permissions = sidecar_site_permissions_for_tab(tab, &snapshot);

        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].origin().as_str(), "https://example.com");
        assert_eq!(permissions[0].feature(), SitePermissionFeature::Camera);
        assert_eq!(permissions[0].decision(), SitePermissionDecision::AllowAlways);
        Ok(())
    }

    #[test]
    fn filters_other_origins() -> Result<(), Box<dyn Error>> {
        let mut core = browser_core_for("https://example.com/page")?;
        core.set_site_permission(
            SiteOrigin::parse("https://other.test")?,
            SitePermissionFeature::Location,
            SitePermissionDecision::AllowOnce,
        )?;

        let snapshot = core.snapshot()?;
        let tab = active_tab(&snapshot)?;

        assert!(sidecar_site_permissions_for_tab(tab, &snapshot).is_empty());
        Ok(())
    }

    fn browser_core_for(url: &str) -> Result<BrowserCore, Box<dyn Error>> {
        let mut core = BrowserCore::new(InitialBrowserConfig {
            space_name: "Work".to_string(),
            space_icon: "W".to_string(),
            profile_name: "Default".to_string(),
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
