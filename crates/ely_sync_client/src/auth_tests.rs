use std::{
    collections::BTreeMap,
    fs,
    sync::{Mutex, MutexGuard},
};

use ely_domain::ProfileId;
use zeroize::Zeroizing;

use super::*;

#[derive(Default)]
struct MemoryCredentialBackend {
    values: Mutex<BTreeMap<(String, String), Vec<u8>>>,
}

impl CredentialBackend for MemoryCredentialBackend {
    fn load(&self, service: &str, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        Ok(self
            .values()
            .get(&(service.to_string(), account.to_string()))
            .cloned()
            .map(Zeroizing::new))
    }

    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
        self.values().insert((service.to_string(), account.to_string()), secret.to_vec());
        Ok(())
    }

    fn clear(&self, service: &str, account: &str) -> Result<(), String> {
        self.values().remove(&(service.to_string(), account.to_string()));
        Ok(())
    }
}

impl MemoryCredentialBackend {
    fn values(&self) -> MutexGuard<'_, BTreeMap<(String, String), Vec<u8>>> {
        self.values.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct DroppingCredentialBackend;

impl CredentialBackend for DroppingCredentialBackend {
    fn load(&self, _service: &str, _account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        Ok(None)
    }

    fn save(&self, _service: &str, _account: &str, _secret: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn clear(&self, _service: &str, _account: &str) -> Result<(), String> {
        Ok(())
    }
}

struct SaveFailureBackend;

impl CredentialBackend for SaveFailureBackend {
    fn load(&self, _service: &str, _account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        Ok(None)
    }

    fn save(&self, _service: &str, _account: &str, _secret: &[u8]) -> Result<(), String> {
        Err("credential backend is locked".to_string())
    }

    fn clear(&self, _service: &str, _account: &str) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
struct ClearFailureBackend {
    values: Mutex<BTreeMap<(String, String), Vec<u8>>>,
}

impl CredentialBackend for ClearFailureBackend {
    fn load(&self, service: &str, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        Ok(self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(service.to_string(), account.to_string()))
            .cloned()
            .map(Zeroizing::new))
    }

    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((service.to_string(), account.to_string()), secret.to_vec());
        Ok(())
    }

    fn clear(&self, _service: &str, _account: &str) -> Result<(), String> {
        Err("credential backend is locked".to_string())
    }
}

#[test]
fn validates_token_shape_boundaries() -> Result<(), SyncClientError> {
    assert!(BearerToken::new("").is_err());
    assert!(BearerToken::new("short").is_err());
    assert!(BearerToken::new(format!("{}aaa bb", "a".repeat(40))).is_err());
    BearerToken::new("a".repeat(32))?;
    BearerToken::new("a".repeat(MAX_BEARER_TOKEN_BYTES))?;
    assert!(BearerToken::new("a".repeat(MAX_BEARER_TOKEN_BYTES + 1)).is_err());
    Ok(())
}

#[test]
fn debug_output_redacts_the_token() -> Result<(), SyncClientError> {
    let token = BearerToken::new("secret-session-token".repeat(3))?;
    let output = format!("{token:?}");

    assert_eq!(output, "BearerToken([REDACTED])");
    assert!(!output.contains(token.as_str()));
    Ok(())
}

#[test]
fn keychain_entry_wins_and_cleans_every_legacy_path() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    let old = directory.path().join("old/bearer.token");
    write_token(&stable, 'a')?;
    write_token(&old, 'b')?;
    write_token(&stable.with_extension("tmp"), 'c')?;
    write_token(&old.with_extension("tmp"), 'd')?;
    let backend = MemoryCredentialBackend::default();
    let expected = BearerToken::new("e".repeat(64))?;
    backend.save(KEYCHAIN_SERVICE, profile_id.as_str(), expected.as_str().as_bytes())?;
    let store = BearerTokenStore::new(&profile_id, directory.path()).with_legacy_path(old.clone());

    assert_eq!(store.load_with(&backend)?, Some(expected));
    for path in [stable, old] {
        assert!(!path.exists());
        assert!(!path.with_extension("tmp").exists());
    }
    assert!(migration_marker_path(directory.path()).exists());
    Ok(())
}

#[test]
fn stable_legacy_token_precedes_old_default_source() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    let old = directory.path().join("old/bearer.token");
    write_token(&stable, 'a')?;
    write_token(&old, 'b')?;
    let backend = MemoryCredentialBackend::default();
    let store = BearerTokenStore::new(&profile_id, directory.path()).with_legacy_path(old.clone());

    assert_eq!(
        store.load_with(&backend)?.as_ref().map(BearerToken::as_str),
        Some("a".repeat(64).as_str())
    );
    assert!(!stable.exists());
    assert!(!old.exists());
    Ok(())
}

#[test]
fn failed_keychain_readback_preserves_the_legacy_source() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    write_token(&stable, 'a')?;
    let store = BearerTokenStore::new(&profile_id, directory.path());

    assert!(store.load_with(&DroppingCredentialBackend).is_err());
    assert!(stable.exists());
    Ok(())
}

#[test]
fn existing_keychain_entry_finishes_a_crashed_migration() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    write_token(&stable, 'a')?;
    let backend = MemoryCredentialBackend::default();
    backend.save(KEYCHAIN_SERVICE, profile_id.as_str(), "a".repeat(64).as_bytes())?;
    let store = BearerTokenStore::new(&profile_id, directory.path());

    assert!(store.load_with(&backend)?.is_some());
    assert!(!stable.exists());
    Ok(())
}

#[test]
fn profile_accounts_are_isolated_and_clear_is_idempotent() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let first_id = ProfileId::new();
    let second_id = ProfileId::new();
    let first = BearerTokenStore::new(&first_id, &directory.path().join("first"));
    let second = BearerTokenStore::new(&second_id, &directory.path().join("second"));
    let first_token = BearerToken::new("a".repeat(64))?;
    let second_token = BearerToken::new("b".repeat(64))?;
    let backend = MemoryCredentialBackend::default();

    first.save_with(&backend, &first_token)?;
    second.save_with(&backend, &second_token)?;
    assert_eq!(first.load_with(&backend)?, Some(first_token));
    assert_eq!(second.load_with(&backend)?, Some(second_token.clone()));
    first.clear_with(&backend)?;
    first.clear_with(&backend)?;
    assert_eq!(first.load_with(&backend)?, None);
    assert_eq!(second.load_with(&backend)?, Some(second_token));
    Ok(())
}

#[test]
fn conditional_clear_preserves_a_newer_profile_token() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let old = BearerToken::new("a".repeat(64))?;
    let current = BearerToken::new("b".repeat(64))?;
    let backend = MemoryCredentialBackend::default();

    store.save_with(&backend, &old)?;
    store.save_with(&backend, &current)?;
    assert!(!store.clear_if_matches_with(&backend, &old)?);
    assert_eq!(store.load_with(&backend)?, Some(current.clone()));
    assert!(store.clear_if_matches_with(&backend, &current)?);
    assert_eq!(store.load_with(&backend)?, None);
    Ok(())
}

#[test]
fn clear_failure_preserves_the_credential_for_retry() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let token = BearerToken::new("a".repeat(64))?;
    let backend = ClearFailureBackend::default();

    store.save_with(&backend, &token)?;
    assert!(store.clear_with(&backend).is_err());
    assert_eq!(store.load_with(&backend)?, Some(token));
    Ok(())
}

#[test]
fn migration_marker_blocks_recreated_legacy_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let old = directory.path().join("old/bearer.token");
    let store = BearerTokenStore::new(&profile_id, directory.path()).with_legacy_path(old.clone());
    let backend = MemoryCredentialBackend::default();
    let token = BearerToken::new("a".repeat(64))?;

    store.save_with(&backend, &token)?;
    store.clear_with(&backend)?;
    write_token(&old, 'b')?;

    assert_eq!(store.load_with(&backend)?, None);
    assert!(!old.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn cleanup_failure_prevents_a_direct_keychain_commit() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    let target = directory.path().join("target.tmp");
    write_token(&target, 'b')?;
    fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
    fs::hard_link(&target, stable.with_extension("tmp"))?;
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let backend = MemoryCredentialBackend::default();
    let token = BearerToken::new("a".repeat(64))?;

    assert!(store.save_with(&backend, &token).is_err());
    assert!(backend.load(KEYCHAIN_SERVICE, profile_id.as_str())?.is_none());
    Ok(())
}

#[test]
fn direct_save_failure_retires_stale_legacy_credentials() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    write_token(&stable, 'a')?;
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let token = BearerToken::new("b".repeat(64))?;

    assert!(store.save_with(&SaveFailureBackend, &token).is_err());
    assert!(!stable.exists());
    assert!(migration_marker_path(directory.path()).exists());
    assert_eq!(store.load_with(&MemoryCredentialBackend::default())?, None);
    Ok(())
}

#[cfg(unix)]
#[test]
fn migration_cleanup_failure_keeps_the_committed_keychain() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    let target = directory.path().join("target.tmp");
    write_token(&stable, 'a')?;
    write_token(&target, 'b')?;
    fs::hard_link(&target, stable.with_extension("tmp"))?;
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let backend = MemoryCredentialBackend::default();

    assert!(store.load_with(&backend).is_err());
    assert!(backend.load(KEYCHAIN_SERVICE, profile_id.as_str())?.is_some());
    assert!(migration_marker_path(directory.path()).exists());
    fs::remove_file(stable.with_extension("tmp"))?;
    assert_eq!(
        store.load_with(&backend)?.as_ref().map(BearerToken::as_str),
        Some("a".repeat(64).as_str())
    );
    Ok(())
}

#[test]
fn invalid_migration_marker_never_retires_a_legacy_token() -> Result<(), Box<dyn std::error::Error>>
{
    for marker in [b"".as_slice(), b"partial".as_slice()] {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let stable = stable_legacy_path(directory.path());
        let marker_path = migration_marker_path(directory.path());
        write_token(&stable, 'a')?;
        fs::write(&marker_path, marker)?;
        let store = BearerTokenStore::new(&profile_id, directory.path());
        let backend = MemoryCredentialBackend::default();

        assert!(store.load_with(&backend).is_err());
        assert!(stable.exists());
        assert!(backend.load(KEYCHAIN_SERVICE, profile_id.as_str())?.is_none());
    }
    Ok(())
}

#[test]
fn crashed_marker_temp_is_replaced_atomically() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let marker = migration_marker_path(directory.path());
    let marker_tmp = marker.with_file_name("bearer.migrated.tmp");
    fs::create_dir_all(marker.parent().ok_or("missing marker parent")?)?;
    fs::write(&marker_tmp, b"partial")?;
    let store = BearerTokenStore::new(&profile_id, directory.path());

    store.clear_with(&MemoryCredentialBackend::default())?;

    assert_eq!(fs::read(marker)?, b"ely-bearer-migration-v1\n");
    assert!(!marker_tmp.exists());
    Ok(())
}

#[test]
fn stale_tmp_is_removed_without_becoming_a_token() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let stable = stable_legacy_path(directory.path());
    write_token(&stable.with_extension("tmp"), 'a')?;
    let store = BearerTokenStore::new(&profile_id, directory.path());

    assert_eq!(store.load_with(&MemoryCredentialBackend::default())?, None);
    assert!(!stable.with_extension("tmp").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_and_hardlink_legacy_sources_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    for hard_link in [false, true] {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let target = directory.path().join("target.token");
        let stable = stable_legacy_path(directory.path());
        write_token(&target, 'a')?;
        fs::create_dir_all(stable.parent().ok_or("missing stable parent")?)?;
        if hard_link {
            fs::hard_link(&target, &stable)?;
        } else {
            symlink(&target, &stable)?;
        }
        let backend = MemoryCredentialBackend::default();
        let store = BearerTokenStore::new(&profile_id, directory.path());

        assert!(store.load_with(&backend).is_err());
        assert!(backend.load(KEYCHAIN_SERVICE, profile_id.as_str())?.is_none());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_and_hardlink_lock_files_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    for hard_link in [false, true] {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let target = directory.path().join("target.lock");
        let lock = directory.path().join("sync/bearer.lock");
        fs::write(&target, b"lock-target")?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))?;
        fs::create_dir_all(lock.parent().ok_or("missing lock parent")?)?;
        if hard_link {
            fs::hard_link(&target, &lock)?;
        } else {
            symlink(&target, &lock)?;
        }
        let store = BearerTokenStore::new(&profile_id, directory.path());

        assert!(store.load_with(&MemoryCredentialBackend::default()).is_err());
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o644);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_profile_directories_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let target = directory.path().join("target-profile");
    let linked_profile = directory.path().join("linked-profile");
    fs::create_dir_all(&target)?;
    symlink(&target, &linked_profile)?;
    let store = BearerTokenStore::new(&profile_id, &linked_profile);

    assert!(store.load_with(&MemoryCredentialBackend::default()).is_err());
    assert!(!target.join("sync").exists());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn native_keychain_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let profile_id = ProfileId::new();
    let store = BearerTokenStore::new(&profile_id, directory.path());
    let token = BearerToken::new("n".repeat(64))?;
    let result = (|| {
        store.save(&token)?;
        assert_eq!(store.load()?, Some(token));
        Ok::<(), SyncClientError>(())
    })();
    store.clear()?;
    result?;
    Ok(())
}

fn stable_legacy_path(profile_dir: &std::path::Path) -> std::path::PathBuf {
    profile_dir.join("sync/bearer.token")
}

fn migration_marker_path(profile_dir: &std::path::Path) -> std::path::PathBuf {
    profile_dir.join("sync/bearer.migrated")
}

fn write_token(path: &std::path::Path, character: char) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, character.to_string().repeat(64))
}
