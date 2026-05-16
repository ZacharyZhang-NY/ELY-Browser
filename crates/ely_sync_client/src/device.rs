use std::{
    fs,
    io::{self, ErrorKind},
    path::Path,
};

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::SyncClientError;

/// Locally-stable device identity. Constructed once per profile data
/// directory and persisted so reinstalls don't trigger re-approval
/// requests — the same `device_id` is reused across runs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub public_key: String,
    pub device_name: String,
    pub platform: String,
}

impl DeviceIdentity {
    /// Load the persisted identity, or create-and-save a new one.
    /// The identity file lives at `path`; callers usually pick
    /// `<profile_data>/sync/device.json`.
    pub fn load_or_create(
        path: &Path,
        device_name: impl Into<String>,
        platform: impl Into<String>,
    ) -> Result<Self, SyncClientError> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                let identity: Self = serde_json::from_str(&contents).map_err(|error| {
                    SyncClientError::TokenStorage(format!(
                        "device identity is corrupt at {}: {error}",
                        path.display()
                    ))
                })?;
                identity.validate()?;
                Ok(identity)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let identity = Self::generate(device_name, platform);
                identity.save(path)?;
                Ok(identity)
            }
            Err(error) => Err(SyncClientError::TokenStorage(error.to_string())),
        }
    }

    pub fn generate(device_name: impl Into<String>, platform: impl Into<String>) -> Self {
        let device_id = format!("ely-{}", Uuid::now_v7().simple());
        let public_key = public_key_hex();
        Self { device_id, public_key, device_name: device_name.into(), platform: platform.into() }
    }

    pub fn save(&self, path: &Path) -> Result<(), SyncClientError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = path.with_extension("tmp");
        let serialized = serde_json::to_string_pretty(self).map_err(|error| {
            SyncClientError::TokenStorage(format!("device identity serialize: {error}"))
        })?;
        fs::write(&tmp, serialized).map_err(io_err)?;
        fs::rename(&tmp, path).map_err(io_err)?;
        Ok(())
    }

    fn validate(&self) -> Result<(), SyncClientError> {
        if !is_device_id_shape(&self.device_id) {
            return Err(SyncClientError::TokenStorage(
                "device_id does not match the Cloudflare worker pattern".to_string(),
            ));
        }
        if self.public_key.trim().is_empty() {
            return Err(SyncClientError::TokenStorage("device public_key is empty".to_string()));
        }
        if self.device_name.trim().is_empty() {
            return Err(SyncClientError::TokenStorage("device_name is empty".to_string()));
        }
        if self.platform.trim().is_empty() {
            return Err(SyncClientError::TokenStorage("platform is empty".to_string()));
        }
        Ok(())
    }
}

fn is_device_id_shape(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b':' | b'-'))
}

fn io_err(error: io::Error) -> SyncClientError {
    SyncClientError::TokenStorage(error.to_string())
}

fn public_key_hex() -> String {
    let mut seed = [0_u8; 32];
    seed[..16].copy_from_slice(Uuid::now_v7().as_bytes());
    seed[16..].copy_from_slice(Uuid::now_v7().as_bytes());
    let signing_key = SigningKey::from_bytes(&seed);
    hex_string(&signing_key.verifying_key().to_bytes())
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceRegistration<'a> {
    pub device_id: &'a str,
    pub public_key: &'a str,
    pub device_name: &'a str,
    pub platform: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceListResponse {
    pub version: u32,
    pub user_id: String,
    pub devices: Vec<DeviceRecord>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub approval_status: String,
    pub created_at: u64,
    pub approved_at: Option<u64>,
    pub last_active_at: Option<u64>,
    pub revoked_at: Option<u64>,
}

impl DeviceRecord {
    pub fn is_approved(&self) -> bool {
        self.approval_status == "approved" && self.revoked_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn identity_round_trips() -> Result<(), SyncClientError> {
        let dir = temp_dir().join(format!("ely-device-{}", Uuid::now_v7().simple()));
        let path = dir.join("device.json");
        let identity = DeviceIdentity::load_or_create(&path, "Test", "macos")?;
        identity.validate()?;
        assert_eq!(identity.public_key.len(), 64);
        assert!(identity.public_key.as_bytes().iter().all(u8::is_ascii_hexdigit));

        let again = DeviceIdentity::load_or_create(&path, "ignored", "ignored")?;
        assert_eq!(identity, again);
        Ok(())
    }
}
