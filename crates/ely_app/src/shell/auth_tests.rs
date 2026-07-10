use ely_browser_core::{BrowserCore, InitialBrowserConfig};
use ely_domain::ProfileId;

use super::super::{ShellState, sync_state::SyncStateUpdate};
use super::{
    AuthFlowPhase, active_profile_sync_context_for, bearer_store_for_profile, normalize_email,
    verified_bearer_from, verified_session_persistence_message,
};

#[test]
fn normalize_lowercases_and_trims() {
    assert_eq!(normalize_email("  User@Example.COM  "), Some("user@example.com".to_string()));
}

#[test]
fn normalize_rejects_obviously_broken() {
    assert_eq!(normalize_email("noatsign"), None);
    assert_eq!(normalize_email("@no-local-part"), None);
    assert_eq!(normalize_email("missing-domain@"), None);
    assert_eq!(normalize_email(""), None);
}

#[test]
fn auth_phase_helpers() {
    let profile_id = ProfileId::new();
    let phase =
        AuthFlowPhase::Verifying { profile_id: profile_id.clone(), email: "you@there".to_string() };
    assert!(phase.is_busy());
    assert!(phase.belongs_to(&profile_id));
    assert!(!phase.belongs_to(&ProfileId::new()));
    assert_eq!(phase.error_message(), None);

    let phase = AuthFlowPhase::Error {
        profile_id,
        email: "you@there".to_string(),
        message: "rate limited".to_string(),
    };
    assert_eq!(phase.error_message(), Some("rate limited"));
    assert!(!phase.is_busy());
}

#[test]
fn stale_verified_updates_retain_the_token_for_server_retirement()
-> Result<(), Box<dyn std::error::Error>> {
    let profile_id = ProfileId::new();
    let token = ely_sync_client::BearerToken::new("a".repeat(64))?;
    let update = SyncStateUpdate::AuthVerified {
        profile_id,
        email: "user@example.com".to_string(),
        user_id: "user-01".to_string(),
        token: token.clone(),
    };

    assert_eq!(verified_bearer_from(update), Some(token));
    Ok(())
}

#[test]
fn owner_mismatch_is_actionable_in_the_account_form() {
    assert_eq!(
        verified_session_persistence_message(&ely_sync_client::SyncClientError::SyncOwnerMismatch),
        "This browser data belongs to a different Ely account"
    );
}

#[test]
fn private_profile_has_no_sync_auth_context() -> Result<(), Box<dyn std::error::Error>> {
    let state =
        ShellState::Ready(Box::new(BrowserCore::new(InitialBrowserConfig::private_window()?)?));

    assert_eq!(active_profile_sync_context_for(&state), None);

    let state =
        ShellState::Ready(Box::new(BrowserCore::new(InitialBrowserConfig::ely_defaults()?)?));
    assert!(active_profile_sync_context_for(&state).is_some());

    Ok(())
}

#[test]
fn default_profile_store_cleans_stable_and_old_legacy_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let profile_dir = directory.path().join(profile_id.as_str()).join("servo");
    let stable = profile_dir.join("sync/bearer.token");
    let old = directory.path().join("default/servo/sync/bearer.token");
    std::fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
    std::fs::create_dir_all(old.parent().ok_or("missing old parent")?)?;
    std::fs::write(&stable, "a".repeat(64))?;
    std::fs::write(&old, "b".repeat(64))?;
    let store =
        bearer_store_for_profile(&profile_id, &profile_dir, directory.path(), Some(&profile_id));

    store.clear_legacy_files()?;

    assert!(!stable.exists());
    assert!(!old.exists());
    Ok(())
}

#[test]
fn custom_profile_store_leaves_default_legacy_credentials_untouched()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let default_profile_id = ProfileId::new();
    let profile_dir = directory.path().join(profile_id.as_str()).join("servo");
    let stable = profile_dir.join("sync/bearer.token");
    let old_default = directory.path().join("default/servo/sync/bearer.token");
    std::fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
    std::fs::create_dir_all(old_default.parent().ok_or("missing default parent")?)?;
    std::fs::write(&stable, "a".repeat(64))?;
    std::fs::write(&old_default, "b".repeat(64))?;
    let store = bearer_store_for_profile(
        &profile_id,
        &profile_dir,
        directory.path(),
        Some(&default_profile_id),
    );

    store.clear_legacy_files()?;

    assert!(!stable.exists());
    assert!(old_default.exists());
    Ok(())
}
