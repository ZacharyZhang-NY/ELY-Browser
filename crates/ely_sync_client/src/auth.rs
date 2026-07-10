use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use crate::error::SyncClientError;

/// Better Auth bearer token issued by `https://<base>/api/auth/*`. The
/// token grants access to the per-user `withAuthenticatedApiControls`
/// routes and, once the device is bound to the session, to the
/// `withApprovedDeviceApiControls` routes used by sync.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerToken(String);

impl BearerToken {
    /// Construct a token from an existing string. Trims whitespace and
    /// enforces the same character envelope the worker validates so
    /// obviously-malformed tokens fail before we hit the network.
    pub fn new(value: impl Into<String>) -> Result<Self, SyncClientError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || !is_better_auth_bearer(trimmed) {
            return Err(SyncClientError::TokenStorage(
                "bearer token is not a Better Auth session token".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_better_auth_bearer(token: &str) -> bool {
    let length_ok = (32..=4096).contains(&token.len());
    let charset_ok = token
        .as_bytes()
        .iter()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-'));
    length_ok && charset_ok
}

/// File-backed bearer token store. Lives in the per-profile data
/// directory so a private window never inherits the standard
/// profile's session — same isolation the rest of the runtime
/// enforces. Writes go through a temp-rename so a partial write
/// can't corrupt the persisted token.
#[derive(Clone, Debug)]
pub struct BearerTokenStore {
    path: PathBuf,
}

impl BearerTokenStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<BearerToken>, SyncClientError> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(None);
                }
                BearerToken::new(contents).map(Some)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(SyncClientError::TokenStorage(error.to_string())),
        }
    }

    pub fn save(&self, token: &BearerToken) -> Result<(), SyncClientError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, token.as_str()).map_err(io_err)?;
        fs::rename(&tmp, &self.path).map_err(io_err)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), SyncClientError> {
        remove_file_if_present(&self.path)?;
        remove_file_if_present(&self.path.with_extension("tmp"))
    }
}

fn remove_file_if_present(path: &Path) -> Result<(), SyncClientError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SyncClientError::TokenStorage(error.to_string())),
    }
}

fn io_err(error: io::Error) -> SyncClientError {
    SyncClientError::TokenStorage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn rejects_obviously_broken_tokens() {
        assert!(BearerToken::new("").is_err());
        assert!(BearerToken::new("   ").is_err());
        assert!(BearerToken::new("short").is_err());
        // Spaces are not in the Better Auth bearer charset.
        assert!(BearerToken::new(format!("{}aaa bb", "a".repeat(40))).is_err());
    }

    #[test]
    fn accepts_better_auth_shape() -> Result<(), SyncClientError> {
        let token = format!("{}-{}_{}", "a".repeat(20), "b".repeat(20), "c".repeat(20));
        BearerToken::new(token).map(|_| ())
    }

    #[test]
    fn token_store_round_trips() -> Result<(), SyncClientError> {
        let dir = temp_dir().join(format!("ely-token-{}", uuid::Uuid::now_v7().simple()));
        let store = BearerTokenStore::new(dir.join("token"));
        let token = BearerToken::new("a".repeat(64))?;
        assert_eq!(store.load()?, None);

        store.save(&token)?;
        assert_eq!(store.load()?, Some(token.clone()));
        fs::write(store.path().with_extension("tmp"), token.as_str()).map_err(io_err)?;

        store.clear()?;
        assert_eq!(store.load()?, None);
        assert!(!store.path().with_extension("tmp").exists());
        Ok(())
    }
}
