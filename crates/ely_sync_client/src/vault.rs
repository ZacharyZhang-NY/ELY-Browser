use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hpke::{
    Deserializable, Kem, OpModeR, OpModeS, Serializable, aead::ChaCha20Poly1305, kdf::HkdfSha256,
    kem::X25519HkdfSha256, single_shot_open, single_shot_seal,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    AccountKey, DeviceIdentity, DeviceSecretStore, SyncClientError,
    device::{decode_hex_32, is_device_id_shape},
};

pub const ACCOUNT_KEY_WRAP_VERSION: u32 = 1;
pub const ACCOUNT_KEY_WRAP_SUITE: &str = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";

const INFO_DOMAIN: &[u8] = b"ely-sync-account-key-hpke-info-v1\0";
const AAD_DOMAIN: &[u8] = b"ely-sync-account-key-hpke-aad-v1\0";
const ACCOUNT_KEY_BYTES: usize = 32;
const ENCAPSULATED_KEY_BYTES: usize = 32;
const CIPHERTEXT_BYTES: usize = ACCOUNT_KEY_BYTES + 16;
const MAX_CONTEXT_TEXT_BYTES: usize = 4096;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncVaultDocument {
    pub version: u32,
    pub user_id: String,
    pub key_id: String,
    pub generation: u64,
    pub recipient_device_id: String,
    pub approver_device_id: String,
    pub envelope: WrappedAccountKey,
    pub created_at: u64,
}

impl SyncVaultDocument {
    pub fn unwrap_for(
        &self,
        expected_user_id: &str,
        identity: &DeviceIdentity,
    ) -> Result<AccountKey, SyncClientError> {
        if self.version != 1
            || self.user_id != expected_user_id
            || self.recipient_device_id != identity.device_id
        {
            return Err(vault_error("vault document identity does not match"));
        }
        self.envelope.unwrap(
            &VaultContext {
                user_id: &self.user_id,
                recipient_device_id: &self.recipient_device_id,
                recipient_wrapping_public_key: &identity.wrapping_public_key,
                approver_device_id: &self.approver_device_id,
                generation: self.generation,
                key_id: &self.key_id,
            },
            identity,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VaultContext<'a> {
    pub user_id: &'a str,
    pub recipient_device_id: &'a str,
    pub recipient_wrapping_public_key: &'a str,
    pub approver_device_id: &'a str,
    pub generation: u64,
    pub key_id: &'a str,
}

impl VaultContext<'_> {
    fn validate(&self) -> Result<(), SyncClientError> {
        validate_text(self.user_id, "vault user identifier is invalid")?;
        if !is_device_id_shape(self.recipient_device_id) {
            return Err(vault_error("vault recipient device identifier is invalid"));
        }
        if !is_device_id_shape(self.approver_device_id) {
            return Err(vault_error("vault approver device identifier is invalid"));
        }
        decode_vault_hex_32(
            self.recipient_wrapping_public_key,
            "vault recipient wrapping public key is invalid",
        )?;
        decode_vault_hex_32(self.key_id, "vault account key identifier is invalid")?;
        if self.generation == 0 {
            return Err(vault_error("vault generation is invalid"));
        }
        Ok(())
    }
}

/// Strict JSON envelope for an AccountKey wrapped to one approved device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrappedAccountKey {
    pub version: u32,
    pub suite: String,
    pub encapped_key: String,
    pub ciphertext: String,
}

impl WrappedAccountKey {
    pub fn wrap(
        account_key: &AccountKey,
        context: &VaultContext<'_>,
    ) -> Result<Self, SyncClientError> {
        context.validate()?;
        if account_key.key_id() != context.key_id {
            return Err(vault_error("vault account key identifier does not match"));
        }
        let public_key_bytes = decode_vault_hex_32(
            context.recipient_wrapping_public_key,
            "vault recipient wrapping public key is invalid",
        )?;
        let public_key = <X25519HkdfSha256 as Kem>::PublicKey::from_bytes(&public_key_bytes)
            .map_err(|_| vault_error("vault recipient wrapping public key is invalid"))?;
        let info = context_bytes(INFO_DOMAIN, context)?;
        let aad = context_bytes(AAD_DOMAIN, context)?;
        let (encapped_key, ciphertext) =
            single_shot_seal::<ChaCha20Poly1305, HkdfSha256, X25519HkdfSha256>(
                &OpModeS::Base,
                &public_key,
                &info,
                account_key.bytes(),
                &aad,
            )
            .map_err(|_| vault_error("account key wrapping failed"))?;

        Ok(Self {
            version: ACCOUNT_KEY_WRAP_VERSION,
            suite: ACCOUNT_KEY_WRAP_SUITE.to_string(),
            encapped_key: URL_SAFE_NO_PAD.encode(encapped_key.to_bytes()),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        })
    }

    pub fn unwrap(
        &self,
        context: &VaultContext<'_>,
        recipient: &DeviceIdentity,
    ) -> Result<AccountKey, SyncClientError> {
        context.validate()?;
        recipient.validate()?;
        if recipient.device_id != context.recipient_device_id
            || recipient.wrapping_public_key != context.recipient_wrapping_public_key
        {
            return Err(vault_error("vault recipient identity does not match context"));
        }
        let store = DeviceSecretStore::new(recipient.device_id.clone())?;
        let secrets = store.load_required()?;
        recipient.validate_secrets(&secrets)?;
        self.unwrap_with_private_key(context, secrets.wrapping_private_key())
    }

    pub fn self_wrap(
        account_key: &AccountKey,
        user_id: &str,
        identity: &DeviceIdentity,
        generation: u64,
    ) -> Result<Self, SyncClientError> {
        identity.validate()?;
        Self::wrap(
            account_key,
            &VaultContext {
                user_id,
                recipient_device_id: &identity.device_id,
                recipient_wrapping_public_key: &identity.wrapping_public_key,
                approver_device_id: &identity.device_id,
                generation,
                key_id: &account_key.key_id(),
            },
        )
    }

    pub fn self_unwrap(
        &self,
        user_id: &str,
        identity: &DeviceIdentity,
        generation: u64,
        key_id: &str,
    ) -> Result<AccountKey, SyncClientError> {
        self.unwrap(
            &VaultContext {
                user_id,
                recipient_device_id: &identity.device_id,
                recipient_wrapping_public_key: &identity.wrapping_public_key,
                approver_device_id: &identity.device_id,
                generation,
                key_id,
            },
            identity,
        )
    }

    fn unwrap_with_private_key(
        &self,
        context: &VaultContext<'_>,
        private_key_bytes: &[u8; ACCOUNT_KEY_BYTES],
    ) -> Result<AccountKey, SyncClientError> {
        self.validate_wire()?;
        let private_key = <X25519HkdfSha256 as Kem>::PrivateKey::from_bytes(private_key_bytes)
            .map_err(|_| vault_error("vault recipient private key is invalid"))?;
        let encapped_key_bytes = decode_base64url_exact::<ENCAPSULATED_KEY_BYTES>(
            &self.encapped_key,
            "vault encapsulated key encoding is invalid",
        )?;
        let encapped_key = <X25519HkdfSha256 as Kem>::EncappedKey::from_bytes(&encapped_key_bytes)
            .map_err(|_| vault_error("vault encapsulated key is invalid"))?;
        let ciphertext = decode_base64url_exact::<CIPHERTEXT_BYTES>(
            &self.ciphertext,
            "vault ciphertext encoding is invalid",
        )?;
        let info = context_bytes(INFO_DOMAIN, context)?;
        let aad = context_bytes(AAD_DOMAIN, context)?;
        let plaintext = Zeroizing::new(
            single_shot_open::<ChaCha20Poly1305, HkdfSha256, X25519HkdfSha256>(
                &OpModeR::Base,
                &private_key,
                &encapped_key,
                &info,
                &ciphertext,
                &aad,
            )
            .map_err(|_| vault_error("account key unwrap authentication failed"))?,
        );
        if plaintext.len() != ACCOUNT_KEY_BYTES {
            return Err(vault_error("unwrapped account key has invalid length"));
        }
        let mut bytes = Zeroizing::new([0_u8; ACCOUNT_KEY_BYTES]);
        bytes.copy_from_slice(&plaintext);
        let account_key = AccountKey::from_secret(bytes);
        if account_key.key_id() != context.key_id {
            return Err(vault_error("unwrapped account key identifier does not match"));
        }
        Ok(account_key)
    }

    pub(crate) fn validate_wire(&self) -> Result<(), SyncClientError> {
        if self.version != ACCOUNT_KEY_WRAP_VERSION {
            return Err(vault_error("vault envelope version is unsupported"));
        }
        if self.suite != ACCOUNT_KEY_WRAP_SUITE {
            return Err(vault_error("vault envelope suite is unsupported"));
        }
        decode_base64url_exact::<ENCAPSULATED_KEY_BYTES>(
            &self.encapped_key,
            "vault encapsulated key encoding is invalid",
        )?;
        decode_base64url_exact::<CIPHERTEXT_BYTES>(
            &self.ciphertext,
            "vault ciphertext encoding is invalid",
        )?;
        Ok(())
    }
}

fn context_bytes(domain: &[u8], context: &VaultContext<'_>) -> Result<Vec<u8>, SyncClientError> {
    context.validate()?;
    let mut bytes = Vec::with_capacity(domain.len() + 512);
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&ACCOUNT_KEY_WRAP_VERSION.to_be_bytes());
    push_text(&mut bytes, ACCOUNT_KEY_WRAP_SUITE)?;
    push_text(&mut bytes, context.user_id)?;
    push_text(&mut bytes, context.recipient_device_id)?;
    push_text(&mut bytes, context.recipient_wrapping_public_key)?;
    push_text(&mut bytes, context.approver_device_id)?;
    bytes.extend_from_slice(&context.generation.to_be_bytes());
    push_text(&mut bytes, context.key_id)?;
    Ok(bytes)
}

fn validate_text(value: &str, reason: &'static str) -> Result<(), SyncClientError> {
    if value.trim().is_empty() || value.len() > MAX_CONTEXT_TEXT_BYTES {
        return Err(vault_error(reason));
    }
    Ok(())
}

fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), SyncClientError> {
    validate_text(value, "vault authenticated metadata is invalid")?;
    let length = u16::try_from(value.len())
        .map_err(|_| vault_error("vault authenticated metadata is too long"))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_base64url_exact<const N: usize>(
    value: &str,
    reason: &'static str,
) -> Result<[u8; N], SyncClientError> {
    let decoded = URL_SAFE_NO_PAD.decode(value).map_err(|_| vault_error(reason))?;
    if URL_SAFE_NO_PAD.encode(&decoded) != value {
        return Err(vault_error(reason));
    }
    decoded.try_into().map_err(|_| vault_error(reason))
}

fn decode_vault_hex_32(value: &str, reason: &'static str) -> Result<[u8; 32], SyncClientError> {
    decode_hex_32(value, reason).map_err(|_| vault_error(reason))
}

fn vault_error(reason: &'static str) -> SyncClientError {
    SyncClientError::VaultCrypto { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::generate_key_material;

    fn context<'a>(identity: &'a DeviceIdentity, key_id: &'a str) -> VaultContext<'a> {
        VaultContext {
            user_id: "user-01",
            recipient_device_id: &identity.device_id,
            recipient_wrapping_public_key: &identity.wrapping_public_key,
            approver_device_id: &identity.device_id,
            generation: 1,
            key_id,
        }
    }

    #[test]
    fn account_key_round_trips_through_hpke() -> Result<(), SyncClientError> {
        let (identity, secrets) = generate_key_material("Test".to_string(), "macos".to_string())?;
        let account_key = AccountKey::from_bytes([41; ACCOUNT_KEY_BYTES]);
        let key_id = account_key.key_id();
        let context = context(&identity, &key_id);
        let wrapped = WrappedAccountKey::self_wrap(&account_key, "user-01", &identity, 1)?;
        let unwrapped =
            wrapped.unwrap_with_private_key(&context, secrets.wrapping_private_key())?;

        assert_eq!(unwrapped.key_id(), account_key.key_id());
        assert_eq!(wrapped.encapped_key.len(), 43);
        assert_eq!(wrapped.ciphertext.len(), 64);
        Ok(())
    }

    #[test]
    fn authenticated_context_rejects_metadata_changes() -> Result<(), SyncClientError> {
        let (identity, secrets) = generate_key_material("Test".to_string(), "macos".to_string())?;
        let account_key = AccountKey::from_bytes([43; ACCOUNT_KEY_BYTES]);
        let key_id = account_key.key_id();
        let context = context(&identity, &key_id);
        let wrapped = WrappedAccountKey::wrap(&account_key, &context)?;
        let other_key_id = AccountKey::from_bytes([44; ACCOUNT_KEY_BYTES]).key_id();
        let other_wrapping_public_key = "01".repeat(ACCOUNT_KEY_BYTES);

        let changed = [
            VaultContext { user_id: "user-02", ..context },
            VaultContext { recipient_device_id: "device-02", ..context },
            VaultContext { recipient_wrapping_public_key: &other_wrapping_public_key, ..context },
            VaultContext { approver_device_id: "device-02", ..context },
            VaultContext { generation: 2, ..context },
            VaultContext { key_id: &other_key_id, ..context },
        ];
        for changed_context in changed {
            assert!(
                wrapped
                    .unwrap_with_private_key(&changed_context, secrets.wrapping_private_key())
                    .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn wire_schema_rejects_unknown_fields() -> Result<(), SyncClientError> {
        let json = format!(
            r#"{{"version":1,"suite":"{ACCOUNT_KEY_WRAP_SUITE}","encapped_key":"{}","ciphertext":"{}","unknown":true}}"#,
            "A".repeat(43),
            "A".repeat(64)
        );
        assert!(serde_json::from_str::<WrappedAccountKey>(&json).is_err());
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn self_wrap_uses_native_credential_store() -> Result<(), SyncClientError> {
        let identity = DeviceIdentity::generate("Test", "macos")?;
        let store = DeviceSecretStore::new(identity.device_id.clone())?;
        let result = (|| {
            let account_key = AccountKey::from_bytes([47; ACCOUNT_KEY_BYTES]);
            let key_id = account_key.key_id();
            let wrapped = WrappedAccountKey::self_wrap(&account_key, "user-01", &identity, 1)?;
            let unwrapped = wrapped.self_unwrap("user-01", &identity, 1, &key_id)?;
            assert_eq!(unwrapped.key_id(), key_id);
            Ok(())
        })();
        store.clear()?;
        result
    }
}
