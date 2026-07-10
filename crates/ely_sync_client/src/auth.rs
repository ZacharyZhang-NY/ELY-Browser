use std::{
    fmt,
    path::{Path, PathBuf},
};

use ely_domain::ProfileId;
use fs2::FileExt;
use zeroize::Zeroizing;

use crate::{
    auth_files::{
        ensure_migration_marker, migration_marker_exists, open_lock_file, read_legacy_bytes,
        remove_file_if_present,
    },
    credential_store::{clear_secret, load_secret, save_secret},
    error::SyncClientError,
};

const KEYCHAIN_SERVICE: &str = "com.elydora.ely-browser.auth.bearer.v1";
const MIN_BEARER_TOKEN_BYTES: usize = 32;
const MAX_BEARER_TOKEN_BYTES: usize = 2560;

/// Better Auth bearer token issued by `https://<base>/api/auth/*`.
pub struct BearerToken(Zeroizing<String>);

impl Clone for BearerToken {
    fn clone(&self) -> Self {
        Self(Zeroizing::new(self.as_str().to_string()))
    }
}

impl fmt::Debug for BearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BearerToken([REDACTED])")
    }
}

impl PartialEq for BearerToken {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for BearerToken {}

impl BearerToken {
    pub fn new(value: impl Into<String>) -> Result<Self, SyncClientError> {
        let value = Zeroizing::new(value.into());
        let trimmed = value.trim();
        if trimmed.is_empty() || !is_better_auth_bearer(trimmed) {
            return Err(storage_error("bearer token is not a Better Auth session token"));
        }
        Ok(Self(Zeroizing::new(trimmed.to_string())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_better_auth_bearer(token: &str) -> bool {
    let length_ok = (MIN_BEARER_TOKEN_BYTES..=MAX_BEARER_TOKEN_BYTES).contains(&token.len());
    let charset_ok = token
        .as_bytes()
        .iter()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-'));
    length_ok && charset_ok
}

trait CredentialBackend {
    fn load(&self, service: &str, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String>;
    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), String>;
    fn clear(&self, service: &str, account: &str) -> Result<(), String>;
}

struct NativeCredentialBackend;

impl CredentialBackend for NativeCredentialBackend {
    fn load(&self, service: &str, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        load_secret(service, account)
    }

    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
        save_secret(service, account, secret)
    }

    fn clear(&self, service: &str, account: &str) -> Result<(), String> {
        clear_secret(service, account)
    }
}

/// Profile-scoped native credential store with one-time plaintext-file migration.
#[derive(Clone, Debug)]
pub struct BearerTokenStore {
    account: String,
    lock_path: PathBuf,
    migration_marker_path: PathBuf,
    legacy_paths: Vec<PathBuf>,
}

impl BearerTokenStore {
    pub fn new(profile_id: &ProfileId, profile_data_dir: &Path) -> Self {
        let sync_dir = profile_data_dir.join("sync");
        Self {
            account: profile_id.as_str().to_string(),
            lock_path: sync_dir.join("bearer.lock"),
            migration_marker_path: sync_dir.join("bearer.migrated"),
            legacy_paths: vec![sync_dir.join("bearer.token")],
        }
    }

    pub fn with_legacy_path(mut self, path: PathBuf) -> Self {
        if !self.legacy_paths.contains(&path) {
            self.legacy_paths.push(path);
        }
        self
    }

    pub fn load(&self) -> Result<Option<BearerToken>, SyncClientError> {
        self.load_with(&NativeCredentialBackend)
    }

    pub fn save(&self, token: &BearerToken) -> Result<(), SyncClientError> {
        self.save_with(&NativeCredentialBackend, token)
    }

    pub fn clear(&self) -> Result<(), SyncClientError> {
        self.clear_with(&NativeCredentialBackend)
    }

    pub fn clear_if_matches(&self, token: &BearerToken) -> Result<bool, SyncClientError> {
        self.clear_if_matches_with(&NativeCredentialBackend, token)
    }

    fn clear_if_matches_with<B: CredentialBackend>(
        &self,
        backend: &B,
        token: &BearerToken,
    ) -> Result<bool, SyncClientError> {
        self.with_lock(|| {
            let current = load_backend_token(backend, &self.account)?;
            ensure_migration_marker(&self.migration_marker_path)?;
            self.cleanup_legacy_paths()?;
            if current.as_ref().is_some_and(|current| current != token) {
                return Ok(false);
            }
            backend.clear(KEYCHAIN_SERVICE, &self.account).map_err(storage_error)?;
            Ok(true)
        })
    }

    pub fn clear_legacy_files(&self) -> Result<(), SyncClientError> {
        self.with_lock(|| self.cleanup_legacy_paths())
    }

    fn load_with<B: CredentialBackend>(
        &self,
        backend: &B,
    ) -> Result<Option<BearerToken>, SyncClientError> {
        self.with_lock(|| self.load_locked(backend))
    }

    fn save_with<B: CredentialBackend>(
        &self,
        backend: &B,
        token: &BearerToken,
    ) -> Result<(), SyncClientError> {
        self.with_lock(|| {
            ensure_migration_marker(&self.migration_marker_path)?;
            self.cleanup_legacy_paths()?;
            backend
                .save(KEYCHAIN_SERVICE, &self.account, token.as_str().as_bytes())
                .map_err(storage_error)
        })
    }

    fn clear_with<B: CredentialBackend>(&self, backend: &B) -> Result<(), SyncClientError> {
        self.with_lock(|| {
            ensure_migration_marker(&self.migration_marker_path)?;
            self.cleanup_legacy_paths()?;
            backend.clear(KEYCHAIN_SERVICE, &self.account).map_err(storage_error)
        })
    }

    fn load_locked<B: CredentialBackend>(
        &self,
        backend: &B,
    ) -> Result<Option<BearerToken>, SyncClientError> {
        if let Some(token) = load_backend_token(backend, &self.account)? {
            ensure_migration_marker(&self.migration_marker_path)?;
            self.cleanup_legacy_paths()?;
            return Ok(Some(token));
        }

        if migration_marker_exists(&self.migration_marker_path)? {
            self.cleanup_legacy_paths()?;
            return Ok(None);
        }

        let mut candidate = None;
        for path in &self.legacy_paths {
            if let Some(token) = read_legacy_token(path)? {
                candidate = Some(token);
                break;
            }
        }
        let Some(token) = candidate else {
            self.cleanup_legacy_tmp_files()?;
            return Ok(None);
        };
        backend
            .save(KEYCHAIN_SERVICE, &self.account, token.as_str().as_bytes())
            .map_err(storage_error)?;
        if let Err(error) = verify_backend_token(backend, &self.account, &token) {
            return Err(rollback_migration(backend, &self.account, error));
        }
        if let Err(error) = ensure_migration_marker(&self.migration_marker_path) {
            if matches!(migration_marker_exists(&self.migration_marker_path), Ok(false)) {
                return Err(rollback_migration(backend, &self.account, error));
            }
            return Err(error);
        }
        self.cleanup_legacy_paths()?;
        Ok(Some(token))
    }

    fn cleanup_legacy_paths(&self) -> Result<(), SyncClientError> {
        for path in &self.legacy_paths {
            remove_file_if_present(path)?;
            remove_file_if_present(&path.with_extension("tmp"))?;
        }
        Ok(())
    }

    fn cleanup_legacy_tmp_files(&self) -> Result<(), SyncClientError> {
        for path in &self.legacy_paths {
            remove_file_if_present(&path.with_extension("tmp"))?;
        }
        Ok(())
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, SyncClientError>,
    ) -> Result<T, SyncClientError> {
        let lock = open_lock_file(&self.lock_path)?;
        lock.lock_exclusive().map_err(|error| storage_error(error.to_string()))?;
        operation()
    }
}

fn load_backend_token<B: CredentialBackend>(
    backend: &B,
    account: &str,
) -> Result<Option<BearerToken>, SyncClientError> {
    let Some(secret) = backend.load(KEYCHAIN_SERVICE, account).map_err(storage_error)? else {
        return Ok(None);
    };
    let value =
        std::str::from_utf8(&secret).map_err(|error| storage_error(error.to_string()))?.to_string();
    BearerToken::new(value).map(Some)
}

fn verify_backend_token<B: CredentialBackend>(
    backend: &B,
    account: &str,
    token: &BearerToken,
) -> Result<(), SyncClientError> {
    let stored = load_backend_token(backend, account)?;
    if stored.as_ref() != Some(token) {
        return Err(storage_error("native credential read-back did not match"));
    }
    Ok(())
}

fn rollback_migration<B: CredentialBackend>(
    backend: &B,
    account: &str,
    cause: SyncClientError,
) -> SyncClientError {
    match backend.clear(KEYCHAIN_SERVICE, account) {
        Ok(()) => cause,
        Err(rollback_error) => {
            storage_error(format!("{cause}; native credential rollback failed: {rollback_error}"))
        }
    }
}

fn read_legacy_token(path: &Path) -> Result<Option<BearerToken>, SyncClientError> {
    let Some(bytes) = read_legacy_bytes(path, MAX_BEARER_TOKEN_BYTES + 1)? else {
        return Ok(None);
    };
    let value =
        std::str::from_utf8(&bytes).map_err(|error| storage_error(error.to_string()))?.to_string();
    BearerToken::new(value).map(Some)
}

fn storage_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::BearerCredentialStorage(message.into())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
