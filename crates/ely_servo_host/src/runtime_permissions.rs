use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};
use servo::WebView;

use crate::{
    ConsumedPermission, PermissionDecision, PermissionSnapshotEntry, PermissionSnapshotState,
};

pub(super) type PermissionStore = Rc<RefCell<PermissionState>>;

#[derive(Default)]
pub(super) struct PermissionState {
    entries: HashMap<PermissionKey, StoredPermission>,
    generations: HashMap<ProfileId, u64>,
    consumed: Vec<ConsumedPermission>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredPermission {
    decision: PermissionDecision,
    grant_revision: u64,
    consumed: bool,
}

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

pub(super) fn replace_permission_decisions(
    permissions: &PermissionStore,
    profile_id: &ProfileId,
    generation: u64,
    entries: Vec<PermissionSnapshotEntry>,
) {
    let mut state = permissions.borrow_mut();
    if state.generations.get(profile_id).is_some_and(|current| generation < *current) {
        return;
    }
    let mut freshly_consumed = Vec::new();
    let replacements = entries
        .into_iter()
        .map(|entry| {
            let key = PermissionKey::new(profile_id.clone(), entry.origin, entry.feature);
            let stored = match entry.state {
                PermissionSnapshotState::Decision(decision) => {
                    let consumed = decision == PermissionDecision::AllowOnce
                        && state.entries.get(&key).is_some_and(|current| {
                            current.decision == PermissionDecision::AllowOnce
                                && current.grant_revision == entry.revision
                                && current.consumed
                        });
                    StoredPermission { decision, grant_revision: entry.revision, consumed }
                }
                PermissionSnapshotState::TransferredAllowOnce => match state.entries.get(&key) {
                    Some(current)
                        if current.decision == PermissionDecision::AllowOnce
                            && current.grant_revision == entry.revision =>
                    {
                        StoredPermission {
                            decision: PermissionDecision::AllowOnce,
                            grant_revision: current.grant_revision,
                            consumed: current.consumed,
                        }
                    }
                    _ => {
                        freshly_consumed.push(ConsumedPermission {
                            profile_id: profile_id.clone(),
                            origin: key.origin.clone(),
                            feature: key.feature,
                            grant_revision: entry.revision,
                        });
                        StoredPermission {
                            decision: PermissionDecision::AllowOnce,
                            grant_revision: entry.revision,
                            consumed: true,
                        }
                    }
                },
            };
            (key, stored)
        })
        .collect::<Vec<_>>();
    state.entries.retain(|key, _| &key.profile_id != profile_id);
    state.entries.extend(replacements);
    state.consumed.extend(freshly_consumed);
    state.generations.insert(profile_id.clone(), generation);
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
    let mut state = permissions.borrow_mut();
    let (decision, grant_revision) = {
        let stored = state.entries.get_mut(&key)?;
        match stored.decision {
            PermissionDecision::AllowOnce if stored.consumed => return None,
            PermissionDecision::AllowOnce => {
                stored.consumed = true;
                (PermissionDecision::AllowOnce, Some(stored.grant_revision))
            }
            decision => (decision, None),
        }
    };
    if let Some(grant_revision) = grant_revision {
        state.consumed.push(ConsumedPermission {
            profile_id: key.profile_id,
            origin: key.origin,
            feature: key.feature,
            grant_revision,
        });
    }
    Some(decision)
}

pub(super) fn drain_consumed_permissions(permissions: &PermissionStore) -> Vec<ConsumedPermission> {
    std::mem::take(&mut permissions.borrow_mut().consumed)
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
#[path = "runtime_permissions_tests.rs"]
mod tests;
