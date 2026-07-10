use std::{
    fs,
    io::{self, ErrorKind},
    path::Path,
};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use hpke::{Deserializable, Kem, Serializable, kem::X25519HkdfSha256};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    device_secret_store::{DeviceSecretStore, DeviceSecrets},
    error::SyncClientError,
};

const PUBLIC_KEY_BYTES: usize = 32;
const MAX_DEVICE_TEXT_CHARS: usize = 128;

/// Public device identity persisted in the profile directory. Both private
/// keys live in the macOS data-protection Keychain under `device_id`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceIdentity {
    pub device_id: String,
    /// Ed25519 verification key encoded as lowercase hex.
    pub public_key: String,
    /// RFC 9180 X25519 recipient key encoded as lowercase hex.
    pub wrapping_public_key: String,
    pub device_name: String,
    pub platform: String,
}

impl DeviceIdentity {
    /// Loads a v2 identity and its Keychain secrets. A legacy public-only
    /// identity is rotated to a fresh device ID because its private key was
    /// never persisted and cannot prove device continuity.
    pub fn load_or_create(
        path: &Path,
        device_name: impl Into<String>,
        platform: impl Into<String>,
    ) -> Result<Self, SyncClientError> {
        let device_name = device_name.into();
        let platform = platform.into();
        match fs::read_to_string(path) {
            Ok(contents) => match decode_stored_identity(&contents, path)? {
                StoredIdentity::V2(identity) => {
                    identity.validate()?;
                    match DeviceSecretStore::new(identity.device_id.clone())?.load()? {
                        Some(secrets) => {
                            identity.validate_secrets(&secrets)?;
                            Ok(identity)
                        }
                        None => {
                            Self::create_and_save(path, identity.device_name, identity.platform)
                        }
                    }
                }
                StoredIdentity::Legacy(identity) => {
                    identity.validate()?;
                    Self::create_and_save(path, identity.device_name, identity.platform)
                }
            },
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Self::create_and_save(path, device_name, platform)
            }
            Err(error) => Err(SyncClientError::TokenStorage(error.to_string())),
        }
    }

    pub fn generate(
        device_name: impl Into<String>,
        platform: impl Into<String>,
    ) -> Result<Self, SyncClientError> {
        let (identity, secrets) = generate_key_material(device_name.into(), platform.into())?;
        DeviceSecretStore::new(identity.device_id.clone())?.save(&secrets)?;
        Ok(identity)
    }

    pub fn save(&self, path: &Path) -> Result<(), SyncClientError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = path.with_extension("tmp");
        let serialized = serde_json::to_string_pretty(self).map_err(|error| {
            SyncClientError::TokenStorage(format!("device identity serialize: {error}"))
        })?;
        fs::write(&tmp, serialized).map_err(io_err)?;
        fs::rename(&tmp, path).map_err(io_err)
    }

    /// Signs exact canonical bytes for device registration and rebind proofs.
    pub fn sign_message(&self, message: &[u8]) -> Result<String, SyncClientError> {
        self.validate()?;
        let secrets = DeviceSecretStore::new(self.device_id.clone())?.load_required()?;
        self.validate_secrets(&secrets)?;
        Ok(self.sign_message_with_secrets(message, &secrets))
    }

    fn sign_message_with_secrets(&self, message: &[u8], secrets: &DeviceSecrets) -> String {
        let signing_key = SigningKey::from_bytes(secrets.signing_private_key());
        hex_string(&signing_key.sign(message).to_bytes())
    }

    pub(crate) fn validate(&self) -> Result<(), SyncClientError> {
        validate_common(
            &self.device_id,
            &self.public_key,
            &self.device_name,
            &self.platform,
            true,
        )?;
        let wrapping_public_key = decode_hex_32(
            &self.wrapping_public_key,
            "device wrapping public key encoding is invalid",
        )?;
        <X25519HkdfSha256 as Kem>::PublicKey::from_bytes(&wrapping_public_key)
            .map_err(|_| key_error("device wrapping public key is invalid"))?;
        Ok(())
    }

    pub(crate) fn validate_secrets(&self, secrets: &DeviceSecrets) -> Result<(), SyncClientError> {
        let expected_signing_public_key =
            decode_hex_32(&self.public_key, "device signing public key encoding is invalid")?;
        let signing_key = SigningKey::from_bytes(secrets.signing_private_key());
        if signing_key.verifying_key().to_bytes() != expected_signing_public_key {
            return Err(key_error("device signing private key does not match identity"));
        }

        let private_key =
            <X25519HkdfSha256 as Kem>::PrivateKey::from_bytes(secrets.wrapping_private_key())
                .map_err(|_| key_error("device wrapping private key is invalid"))?;
        let expected_wrapping_public_key = decode_hex_32(
            &self.wrapping_public_key,
            "device wrapping public key encoding is invalid",
        )?;
        if X25519HkdfSha256::sk_to_pk(&private_key).to_bytes().as_slice()
            != expected_wrapping_public_key
        {
            return Err(key_error("device wrapping private key does not match identity"));
        }
        Ok(())
    }

    fn create_and_save(
        path: &Path,
        device_name: String,
        platform: String,
    ) -> Result<Self, SyncClientError> {
        let (identity, secrets) = generate_key_material(device_name, platform)?;
        let store = DeviceSecretStore::new(identity.device_id.clone())?;
        store.save(&secrets)?;
        if let Err(error) = identity.save(path) {
            let _ = store.clear();
            return Err(error);
        }
        Ok(identity)
    }
}

pub(crate) fn generate_key_material(
    device_name: String,
    platform: String,
) -> Result<(DeviceIdentity, DeviceSecrets), SyncClientError> {
    let mut signing_private_key = Zeroizing::new([0_u8; PUBLIC_KEY_BYTES]);
    getrandom::fill(signing_private_key.as_mut())
        .map_err(|_| key_error("secure randomness unavailable"))?;
    let signing_key = SigningKey::from_bytes(&signing_private_key);

    let mut wrapping_ikm = Zeroizing::new([0_u8; PUBLIC_KEY_BYTES]);
    getrandom::fill(wrapping_ikm.as_mut())
        .map_err(|_| key_error("secure randomness unavailable"))?;
    let (wrapping_private_key, wrapping_public_key) =
        X25519HkdfSha256::derive_keypair(wrapping_ikm.as_slice());
    let mut wrapping_private_bytes = wrapping_private_key.to_bytes();
    let mut stored_wrapping_private_key = [0_u8; PUBLIC_KEY_BYTES];
    stored_wrapping_private_key.copy_from_slice(&wrapping_private_bytes);
    wrapping_private_bytes.zeroize();

    let identity = DeviceIdentity {
        device_id: format!("ely-{}", Uuid::now_v7().simple()),
        public_key: hex_string(&signing_key.verifying_key().to_bytes()),
        wrapping_public_key: hex_string(&wrapping_public_key.to_bytes()),
        device_name: device_name.trim().to_string(),
        platform: platform.trim().to_string(),
    };
    identity.validate()?;
    let secrets = DeviceSecrets::new(*signing_private_key, stored_wrapping_private_key);
    identity.validate_secrets(&secrets)?;
    Ok((identity, secrets))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDeviceIdentity {
    device_id: String,
    public_key: String,
    device_name: String,
    platform: String,
}

impl LegacyDeviceIdentity {
    fn validate(&self) -> Result<(), SyncClientError> {
        validate_common(&self.device_id, &self.public_key, &self.device_name, &self.platform, false)
    }
}

enum StoredIdentity {
    V2(DeviceIdentity),
    Legacy(LegacyDeviceIdentity),
}

fn decode_stored_identity(contents: &str, path: &Path) -> Result<StoredIdentity, SyncClientError> {
    let value: serde_json::Value =
        serde_json::from_str(contents).map_err(|error| corrupt_identity_error(path, error))?;
    if value.get("wrapping_public_key").is_some() {
        serde_json::from_value(value)
            .map(StoredIdentity::V2)
            .map_err(|error| corrupt_identity_error(path, error))
    } else {
        serde_json::from_value(value)
            .map(StoredIdentity::Legacy)
            .map_err(|error| corrupt_identity_error(path, error))
    }
}

fn validate_common(
    device_id: &str,
    signing_public_key: &str,
    device_name: &str,
    platform: &str,
    require_canonical_text: bool,
) -> Result<(), SyncClientError> {
    if !is_device_id_shape(device_id) {
        return Err(key_error("device_id does not match the Cloudflare worker pattern"));
    }
    let signing_public_key =
        decode_hex_32(signing_public_key, "device signing public key encoding is invalid")?;
    VerifyingKey::from_bytes(&signing_public_key)
        .map_err(|_| key_error("device signing public key is invalid"))?;
    validate_device_text(device_name, "device_name is invalid", require_canonical_text)?;
    validate_device_text(platform, "platform is invalid", require_canonical_text)?;
    Ok(())
}

fn validate_device_text(
    value: &str,
    reason: &'static str,
    require_canonical: bool,
) -> Result<(), SyncClientError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_DEVICE_TEXT_CHARS
        || trimmed.chars().any(char::is_control)
        || (require_canonical && value != trimmed)
    {
        return Err(key_error(reason));
    }
    Ok(())
}

pub(crate) fn is_device_id_shape(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b':' | b'-'))
}

pub(crate) fn decode_hex_32(
    value: &str,
    reason: &'static str,
) -> Result<[u8; 32], SyncClientError> {
    if value.len() != 64 {
        return Err(key_error(reason));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0]).ok_or_else(|| key_error(reason))? << 4)
            | hex_nibble(pair[1]).ok_or_else(|| key_error(reason))?;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn key_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::DeviceKeyStorage(message.into())
}

fn corrupt_identity_error(path: &Path, error: serde_json::Error) -> SyncClientError {
    SyncClientError::TokenStorage(format!(
        "device identity is corrupt at {}: {error}",
        path.display()
    ))
}

fn io_err(error: io::Error) -> SyncClientError {
    SyncClientError::TokenStorage(error.to_string())
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceRegistration<'a> {
    pub version: u32,
    pub device_id: &'a str,
    pub public_key: &'a str,
    pub wrapping_public_key: &'a str,
    pub registration_proof: &'a str,
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
    pub public_key: String,
    pub wrapping_public_key: Option<String>,
    pub device_name: String,
    pub platform: String,
    pub approval_status: String,
    pub current: bool,
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

    #[test]
    fn generated_identity_contains_public_keys_only() -> Result<(), SyncClientError> {
        let (identity, _) = generate_key_material(" Test ".to_string(), " macos ".to_string())?;
        let value = serde_json::to_value(&identity).map_err(|error| {
            SyncClientError::TokenStorage(format!("device identity serialize: {error}"))
        })?;

        assert_eq!(value.as_object().map(serde_json::Map::len), Some(5));
        assert_eq!(identity.public_key.len(), 64);
        assert_eq!(identity.wrapping_public_key.len(), 64);
        assert_eq!(identity.device_name, "Test");
        assert_eq!(identity.platform, "macos");
        assert!(value.get("private_key").is_none());
        Ok(())
    }

    #[test]
    fn device_registration_serializes_v2_worker_schema() -> Result<(), SyncClientError> {
        let registration = DeviceRegistration {
            version: 2,
            device_id: "device-01",
            public_key: "01",
            wrapping_public_key: "02",
            registration_proof: "03",
            device_name: "MacBook Pro",
            platform: "macOS",
            idempotency_key: "device-register-device-01",
        };
        let value = serde_json::to_value(registration).map_err(|error| {
            SyncClientError::TokenStorage(format!("device registration serialize: {error}"))
        })?;

        assert_eq!(value["version"], 2);
        assert_eq!(value["wrapping_public_key"], "02");
        assert_eq!(value["registration_proof"], "03");
        Ok(())
    }

    #[test]
    fn legacy_identity_is_detected_for_rotation() -> Result<(), SyncClientError> {
        let (identity, _) = generate_key_material("Test".to_string(), "macos".to_string())?;
        let legacy = serde_json::json!({
            "device_id": identity.device_id,
            "public_key": identity.public_key,
            "device_name": identity.device_name,
            "platform": identity.platform,
        });
        let stored = decode_stored_identity(&legacy.to_string(), Path::new("device.json"))?;
        assert!(matches!(stored, StoredIdentity::Legacy(_)));
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn legacy_identity_rotates_to_new_device_id() -> Result<(), SyncClientError> {
        let dir = std::env::temp_dir().join(format!("ely-device-{}", Uuid::now_v7().simple()));
        let path = dir.join("device.json");
        fs::create_dir_all(&dir).map_err(io_err)?;
        let legacy_device_id = "ely-legacy-device";
        let (_, secrets) = generate_key_material("Test".to_string(), "macos".to_string())?;
        let signing_key = SigningKey::from_bytes(secrets.signing_private_key());
        let legacy = serde_json::json!({
            "device_id": legacy_device_id,
            "public_key": hex_string(&signing_key.verifying_key().to_bytes()),
            "device_name": "Legacy",
            "platform": "macos",
        });
        fs::write(&path, legacy.to_string()).map_err(io_err)?;

        let result = (|| {
            let identity = DeviceIdentity::load_or_create(&path, "ignored", "ignored")?;
            assert_ne!(identity.device_id, legacy_device_id);
            assert_eq!(DeviceIdentity::load_or_create(&path, "ignored", "ignored")?, identity);
            DeviceSecretStore::new(identity.device_id)?.clear()
        })();
        let _ = fs::remove_dir_all(dir);
        result
    }
}
