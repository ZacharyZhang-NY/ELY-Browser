use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::SyncConnectionState;

use super::{SyncStateUpdate, probe_initial_sync_state_at, sync_failure_update};
use crate::services::servo_profile_data::sync_profile_data_dir;

#[test]
fn private_startup_clears_a_persisted_bearer() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let mut core = BrowserCore::new(InitialBrowserConfig::private_window()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let profile_dir = sync_profile_data_dir(directory.path(), &profile_id);
    let bearer_path = profile_dir.join("sync/bearer.token");
    std::fs::create_dir_all(bearer_path.parent().ok_or("missing bearer parent")?)?;
    std::fs::write(&bearer_path, "a".repeat(64))?;
    std::fs::write(bearer_path.with_extension("tmp"), "b".repeat(64))?;
    core.set_sync_connection_state(SyncConnectionState::SignedIn);

    assert!(!probe_initial_sync_state_at(&mut core, directory.path(), None));
    assert!(!bearer_path.exists());
    assert!(!bearer_path.with_extension("tmp").exists());
    assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SignedOut);

    Ok(())
}

#[test]
fn standard_startup_preserves_a_persisted_bearer() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let profile_dir = sync_profile_data_dir(directory.path(), &profile_id);
    let bearer_path = profile_dir.join("sync/bearer.token");
    std::fs::create_dir_all(bearer_path.parent().ok_or("missing bearer parent")?)?;
    std::fs::write(&bearer_path, "a".repeat(64))?;

    assert!(probe_initial_sync_state_at(&mut core, directory.path(), Some(&profile_id),));
    assert!(!bearer_path.exists());
    assert_eq!(core.snapshot()?.sync_status.connection(), &SyncConnectionState::SignedIn);

    let store = ely_sync_client::BearerTokenStore::new(&profile_id, &profile_dir);
    assert!(store.load()?.is_some());
    store.clear()?;

    Ok(())
}

#[cfg(unix)]
#[test]
fn credential_probe_failure_enters_unavailable_state() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let mut core = BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?;
    let profile_id = core.snapshot()?.active_profile_id;
    let profile_dir = sync_profile_data_dir(directory.path(), &profile_id);
    let lock = profile_dir.join("sync/bearer.lock");
    let target = directory.path().join("untrusted.lock");
    std::fs::create_dir_all(lock.parent().ok_or("missing lock parent")?)?;
    std::fs::write(&target, "target")?;
    symlink(&target, &lock)?;

    assert!(!probe_initial_sync_state_at(&mut core, directory.path(), Some(&profile_id),));
    assert!(matches!(
        core.snapshot()?.sync_status.connection(),
        SyncConnectionState::CredentialUnavailable { .. }
    ));
    Ok(())
}

#[test]
fn credential_storage_failures_use_the_unavailable_update() {
    let profile_id = ely_domain::ProfileId::new();
    let update = sync_failure_update(
        profile_id.clone(),
        ely_sync_client::SyncClientError::BearerCredentialStorage("locked".to_string()),
    );

    assert!(matches!(
        update,
        SyncStateUpdate::CredentialUnavailable {
            profile_id: owner,
            ..
        } if owner == profile_id
    ));
}

#[test]
fn terminal_sessions_expire_upload_authentication() {
    let profile_id = ely_domain::ProfileId::new();
    for error in [
        ely_sync_client::SyncClientError::SessionEnded,
        ely_sync_client::SyncClientError::SessionChanged,
    ] {
        let update = sync_failure_update(profile_id.clone(), error);

        assert!(matches!(
            update,
            SyncStateUpdate::AuthenticationExpired { profile_id: owner } if owner == profile_id
        ));
    }
}
