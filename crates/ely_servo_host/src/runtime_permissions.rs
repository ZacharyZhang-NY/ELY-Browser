use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};
use servo::WebView;

use crate::{PermissionDecision, PermissionRequest};

pub(super) type PermissionStore = Rc<RefCell<HashMap<PermissionKey, PermissionDecision>>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PermissionKey {
    profile_id: ProfileId,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
}

impl PermissionKey {
    fn new(profile_id: ProfileId, origin: SiteOrigin, feature: SitePermissionFeature) -> Self {
        Self { profile_id, origin, feature }
    }
}

pub(super) fn set_permission_decision(
    permissions: &PermissionStore,
    request: PermissionRequest,
    decision: PermissionDecision,
) {
    permissions
        .borrow_mut()
        .insert(PermissionKey::new(request.profile_id, request.origin, request.feature), decision);
}

pub(super) fn permission_decision_for_webview(
    permissions: &PermissionStore,
    profile_id: &ProfileId,
    webview: &WebView,
    fallback_url: Option<String>,
    servo_feature: servo::PermissionFeature,
) -> Option<PermissionDecision> {
    let origin = site_origin_for_webview(webview, fallback_url)?;
    let feature = site_permission_feature_for_servo(servo_feature)?;
    take_permission_decision(permissions, profile_id, origin, feature)
}

fn take_permission_decision(
    permissions: &PermissionStore,
    profile_id: &ProfileId,
    origin: SiteOrigin,
    feature: SitePermissionFeature,
) -> Option<PermissionDecision> {
    let key = PermissionKey::new(profile_id.clone(), origin, feature);
    let mut permissions = permissions.borrow_mut();
    match permissions.get(&key).cloned() {
        Some(PermissionDecision::AllowOnce) => permissions.remove(&key),
        decision => decision,
    }
}

fn site_origin_for_webview(webview: &WebView, fallback_url: Option<String>) -> Option<SiteOrigin> {
    webview
        .url()
        .map(|url| url.to_string())
        .or(fallback_url)
        .and_then(|url| SiteOrigin::parse(url).ok())
}

fn site_permission_feature_for_servo(
    feature: servo::PermissionFeature,
) -> Option<SitePermissionFeature> {
    match feature {
        servo::PermissionFeature::Geolocation => Some(SitePermissionFeature::Location),
        servo::PermissionFeature::Notifications => Some(SitePermissionFeature::Notifications),
        servo::PermissionFeature::Camera => Some(SitePermissionFeature::Camera),
        servo::PermissionFeature::Microphone => Some(SitePermissionFeature::Microphone),
        servo::PermissionFeature::PersistentStorage => {
            Some(SitePermissionFeature::StoragePersistence)
        }
        servo::PermissionFeature::ScreenWakeLock(_)
        | servo::PermissionFeature::Push
        | servo::PermissionFeature::Midi
        | servo::PermissionFeature::Speaker
        | servo::PermissionFeature::DeviceInfo
        | servo::PermissionFeature::BackgroundSync
        | servo::PermissionFeature::Gamepad
        | servo::PermissionFeature::Bluetooth => None,
    }
}

#[cfg(test)]
mod tests {
    use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};

    use crate::{PermissionDecision, PermissionRequest};

    use super::{
        PermissionStore, set_permission_decision, site_permission_feature_for_servo,
        take_permission_decision,
    };

    #[test]
    fn keeps_disabled_servo_permissions_out_of_site_settings() {
        for feature in [
            servo::PermissionFeature::ScreenWakeLock(servo::WakeLockType::Screen),
            servo::PermissionFeature::Gamepad,
        ] {
            assert_eq!(site_permission_feature_for_servo(feature), None);
        }
    }

    #[test]
    fn allow_once_is_consumed_after_one_matching_origin_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let permissions = PermissionStore::default();
        let profile_id = ProfileId::new();
        let origin = SiteOrigin::parse("https://example.com/path")?;

        set_permission_decision(
            &permissions,
            PermissionRequest {
                webview_id: ely_domain::WebViewId::new(),
                profile_id: profile_id.clone(),
                origin: origin.clone(),
                feature: SitePermissionFeature::Camera,
            },
            PermissionDecision::AllowOnce,
        );

        assert_eq!(
            take_permission_decision(
                &permissions,
                &profile_id,
                origin.clone(),
                SitePermissionFeature::Camera,
            ),
            Some(PermissionDecision::AllowOnce)
        );
        assert_eq!(
            take_permission_decision(
                &permissions,
                &profile_id,
                origin,
                SitePermissionFeature::Camera
            ),
            None
        );
        Ok(())
    }

    #[test]
    fn allow_always_stays_scoped_to_profile_origin_and_feature()
    -> Result<(), Box<dyn std::error::Error>> {
        let permissions = PermissionStore::default();
        let profile_id = ProfileId::new();
        let other_profile_id = ProfileId::new();
        let origin = SiteOrigin::parse("https://example.com")?;
        let other_origin = SiteOrigin::parse("https://example.org")?;

        set_permission_decision(
            &permissions,
            PermissionRequest {
                webview_id: ely_domain::WebViewId::new(),
                profile_id: profile_id.clone(),
                origin: origin.clone(),
                feature: SitePermissionFeature::Notifications,
            },
            PermissionDecision::AllowAlways,
        );

        assert_eq!(
            take_permission_decision(
                &permissions,
                &profile_id,
                origin.clone(),
                SitePermissionFeature::Notifications,
            ),
            Some(PermissionDecision::AllowAlways)
        );
        assert_eq!(
            take_permission_decision(
                &permissions,
                &other_profile_id,
                origin,
                SitePermissionFeature::Notifications,
            ),
            None
        );
        assert_eq!(
            take_permission_decision(
                &permissions,
                &profile_id,
                other_origin,
                SitePermissionFeature::Notifications,
            ),
            None
        );
        assert_eq!(
            take_permission_decision(
                &permissions,
                &profile_id,
                SiteOrigin::parse("https://example.com")?,
                SitePermissionFeature::Camera,
            ),
            None
        );
        Ok(())
    }
}
