use ely_domain::{ProfileId, SiteOrigin, SitePermissionFeature};

use crate::{
    ConsumedPermission, PermissionDecision, PermissionSnapshotEntry, PermissionSnapshotState,
    runtime_permissions::{
        PermissionStore, drain_consumed_permissions, replace_permission_decisions,
        site_permission_feature_for_servo, take_permission_decision,
    },
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
fn allow_once_is_consumed_after_one_matching_request() -> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );

    assert_eq!(take(&permissions, &profile_id, &origin), Some(PermissionDecision::AllowOnce));
    assert_eq!(take(&permissions, &profile_id, &origin), None);
    assert_eq!(
        drain_consumed_permissions(&permissions),
        vec![ConsumedPermission {
            profile_id,
            origin,
            feature: SitePermissionFeature::Camera,
            grant_revision: 1,
        }],
    );
    Ok(())
}

#[test]
fn durable_permissions_stay_scoped_to_profile_origin_and_feature()
-> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let other_profile = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowAlways,
            1,
        )],
    );

    assert_eq!(take(&permissions, &profile_id, &origin), Some(PermissionDecision::AllowAlways));
    assert_eq!(take(&permissions, &other_profile, &origin), None);
    assert_eq!(
        take_permission_decision(
            &permissions,
            &profile_id,
            SiteOrigin::parse("https://other.test")?,
            SitePermissionFeature::Camera,
        ),
        None,
    );
    Ok(())
}

#[test]
fn authoritative_snapshot_removes_missing_durable_entries() -> Result<(), Box<dyn std::error::Error>>
{
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let other_profile = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    let other_origin = SiteOrigin::parse("https://other.test")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowAlways,
            1,
        )],
    );
    replace(
        &permissions,
        &other_profile,
        1,
        vec![decision(
            other_origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::DenyAlways,
            1,
        )],
    );

    replace(&permissions, &profile_id, 2, Vec::new());

    assert_eq!(take(&permissions, &profile_id, &origin), None);
    assert_eq!(
        take(&permissions, &other_profile, &other_origin),
        Some(PermissionDecision::DenyAlways)
    );
    Ok(())
}

#[test]
fn transferred_marker_preserves_existing_token_and_fails_closed_for_fresh_host()
-> Result<(), Box<dyn std::error::Error>> {
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    let marker = state(
        origin.clone(),
        SitePermissionFeature::Camera,
        PermissionSnapshotState::TransferredAllowOnce,
        1,
    );
    let live = PermissionStore::default();
    replace(
        &live,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );
    replace(&live, &profile_id, 2, vec![marker.clone()]);

    let restarted = PermissionStore::default();
    replace(&restarted, &profile_id, 2, vec![marker]);

    assert!(drain_consumed_permissions(&live).is_empty());
    assert_eq!(
        drain_consumed_permissions(&restarted),
        vec![ConsumedPermission {
            profile_id: profile_id.clone(),
            origin: origin.clone(),
            feature: SitePermissionFeature::Camera,
            grant_revision: 1,
        }],
    );
    assert_eq!(take(&live, &profile_id, &origin), Some(PermissionDecision::AllowOnce));
    assert_eq!(take(&restarted, &profile_id, &origin), None);
    assert_eq!(
        drain_consumed_permissions(&live),
        vec![ConsumedPermission {
            profile_id,
            origin,
            feature: SitePermissionFeature::Camera,
            grant_revision: 1,
        }],
    );
    Ok(())
}

#[test]
fn empty_snapshot_removes_transferred_allow_once() -> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );
    replace(
        &permissions,
        &profile_id,
        2,
        vec![state(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionSnapshotState::TransferredAllowOnce,
            1,
        )],
    );
    replace(&permissions, &profile_id, 3, Vec::new());

    assert_eq!(take(&permissions, &profile_id, &origin), None);
    Ok(())
}

#[test]
fn mismatched_transferred_revision_consumes_the_replacement_token()
-> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );
    replace(
        &permissions,
        &profile_id,
        2,
        vec![state(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionSnapshotState::TransferredAllowOnce,
            2,
        )],
    );

    assert_eq!(take(&permissions, &profile_id, &origin), None);
    assert_eq!(
        drain_consumed_permissions(&permissions),
        vec![ConsumedPermission {
            profile_id,
            origin,
            feature: SitePermissionFeature::Camera,
            grant_revision: 2,
        }],
    );
    Ok(())
}

#[test]
fn stale_profile_generation_cannot_restore_revoked_permission()
-> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    let old =
        decision(origin.clone(), SitePermissionFeature::Camera, PermissionDecision::AllowAlways, 1);
    replace(&permissions, &profile_id, 1, vec![old.clone()]);
    replace(&permissions, &profile_id, 2, Vec::new());

    replace(&permissions, &profile_id, 1, vec![old]);

    assert_eq!(take(&permissions, &profile_id, &origin), None);
    Ok(())
}

#[test]
fn same_revision_stays_consumed_and_new_revision_rearms_token()
-> Result<(), Box<dyn std::error::Error>> {
    let permissions = PermissionStore::default();
    let profile_id = ProfileId::new();
    let origin = SiteOrigin::parse("https://example.com")?;
    replace(
        &permissions,
        &profile_id,
        1,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );
    assert_eq!(take(&permissions, &profile_id, &origin), Some(PermissionDecision::AllowOnce));
    replace(
        &permissions,
        &profile_id,
        2,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            1,
        )],
    );
    assert_eq!(take(&permissions, &profile_id, &origin), None);
    replace(
        &permissions,
        &profile_id,
        3,
        vec![decision(
            origin.clone(),
            SitePermissionFeature::Camera,
            PermissionDecision::AllowOnce,
            3,
        )],
    );

    assert_eq!(take(&permissions, &profile_id, &origin), Some(PermissionDecision::AllowOnce));
    Ok(())
}

fn replace(
    permissions: &PermissionStore,
    profile_id: &ProfileId,
    generation: u64,
    entries: Vec<PermissionSnapshotEntry>,
) {
    replace_permission_decisions(permissions, profile_id, generation, entries);
}

fn decision(
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    decision: PermissionDecision,
    revision: u64,
) -> PermissionSnapshotEntry {
    state(origin, feature, PermissionSnapshotState::Decision(decision), revision)
}

fn state(
    origin: SiteOrigin,
    feature: SitePermissionFeature,
    state: PermissionSnapshotState,
    revision: u64,
) -> PermissionSnapshotEntry {
    PermissionSnapshotEntry { origin, feature, state, revision }
}

fn take(
    permissions: &PermissionStore,
    profile_id: &ProfileId,
    origin: &SiteOrigin,
) -> Option<PermissionDecision> {
    take_permission_decision(permissions, profile_id, origin.clone(), SitePermissionFeature::Camera)
}
