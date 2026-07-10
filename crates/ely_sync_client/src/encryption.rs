use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{error::SyncClientError, snapshot::MAX_SNAPSHOT_BYTES};

pub const SNAPSHOT_ENCRYPTION_VERSION: u32 = 2;

const ENVELOPE_MAGIC: &[u8; 8] = b"ELYSYNC\0";
const ENVELOPE_VERSION: u8 = 1;
const ALGORITHM_XCHACHA20_POLY1305: u8 = 1;
const KEY_BYTES: usize = 32;
const HASH_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + 2 + KEY_BYTES + HASH_BYTES + NONCE_BYTES;
const MAX_PLAINTEXT_BYTES: usize = MAX_SNAPSHOT_BYTES - HEADER_BYTES - TAG_BYTES;
const HKDF_SALT: &[u8] = b"ely-sync-account-key-v1";
const ENCRYPTION_KEY_INFO: &[u8] = b"snapshot-encryption-key";
const CONTENT_KEY_INFO: &[u8] = b"snapshot-content-authentication-key";
const KEY_ID_DOMAIN: &[u8] = b"ely-sync-key-id-v1\0";
const AAD_DOMAIN_V1: &[u8] = b"ely-sync-snapshot-aad-v1\0";
const AAD_DOMAIN_V2: &[u8] = b"ely-sync-snapshot-aad-v2\0";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct AccountKey(Zeroizing<[u8; KEY_BYTES]>);

impl AccountKey {
    pub fn generate() -> Result<Self, SyncClientError> {
        let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
        getrandom::fill(bytes.as_mut())
            .map_err(|_| encryption_error("secure randomness unavailable"))?;
        Ok(Self::from_secret(bytes))
    }

    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self::from_secret(Zeroizing::new(bytes))
    }

    pub fn key_id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(KEY_ID_DOMAIN);
        hasher.update(self.0.as_slice());
        hex_string(&hasher.finalize())
    }

    pub fn content_hash(&self, plaintext: &[u8]) -> Result<String, SyncClientError> {
        let key = self.derived_key(CONTENT_KEY_INFO)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(key.as_slice())
            .map_err(|_| encryption_error("content authentication key is invalid"))?;
        mac.update(plaintext);
        Ok(hex_string(&mac.finalize().into_bytes()))
    }

    pub fn encrypt(
        &self,
        context: &SnapshotCryptoContext<'_>,
        plaintext: &[u8],
    ) -> Result<EncryptedSnapshot, SyncClientError> {
        self.encrypt_with_version(context, plaintext, SNAPSHOT_ENCRYPTION_VERSION)
    }

    fn encrypt_with_version(
        &self,
        context: &SnapshotCryptoContext<'_>,
        plaintext: &[u8],
        encryption_version: u32,
    ) -> Result<EncryptedSnapshot, SyncClientError> {
        if !matches!(encryption_version, 1 | SNAPSHOT_ENCRYPTION_VERSION) {
            return Err(encryption_error("snapshot encryption version is unsupported"));
        }
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(SyncClientError::SnapshotTooLarge {
                bytes: plaintext.len(),
                limit: MAX_PLAINTEXT_BYTES,
            });
        }

        let key_id = self.key_id();
        let content_hash = self.content_hash(plaintext)?;
        let key_id_bytes = decode_hex_32(&key_id)?;
        let content_hash_bytes = decode_hex_32(&content_hash)?;
        let aad = snapshot_aad(context, encryption_version, &key_id_bytes, &content_hash_bytes)?;
        let encryption_key = self.derived_key(ENCRYPTION_KEY_INFO)?;
        let cipher = XChaCha20Poly1305::new_from_slice(encryption_key.as_slice())
            .map_err(|_| encryption_error("snapshot encryption key is invalid"))?;
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce)
            .map_err(|_| encryption_error("secure randomness unavailable"))?;
        let nonce = nonce_ref(&nonce)?;
        let ciphertext = cipher
            .encrypt(nonce, Payload { msg: plaintext, aad: &aad })
            .map_err(|_| encryption_error("snapshot encryption failed"))?;

        let mut bytes = Vec::with_capacity(HEADER_BYTES + ciphertext.len());
        bytes.extend_from_slice(ENVELOPE_MAGIC);
        bytes.push(ENVELOPE_VERSION);
        bytes.push(ALGORITHM_XCHACHA20_POLY1305);
        bytes.extend_from_slice(&key_id_bytes);
        bytes.extend_from_slice(&content_hash_bytes);
        bytes.extend_from_slice(nonce);
        bytes.extend_from_slice(&ciphertext);

        Ok(EncryptedSnapshot { bytes, key_id, content_hash })
    }

    pub fn decrypt(
        &self,
        context: &SnapshotCryptoContext<'_>,
        encryption_version: u32,
        expected_key_id: &str,
        expected_content_hash: &str,
        envelope: &[u8],
    ) -> Result<Vec<u8>, SyncClientError> {
        if !matches!(encryption_version, 1 | SNAPSHOT_ENCRYPTION_VERSION) {
            return Err(encryption_error("snapshot encryption version is unsupported"));
        }
        if envelope.len() < HEADER_BYTES + TAG_BYTES {
            return Err(encryption_error("snapshot envelope is truncated"));
        }
        if &envelope[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC {
            return Err(encryption_error("snapshot envelope magic is invalid"));
        }
        if envelope[ENVELOPE_MAGIC.len()] != ENVELOPE_VERSION
            || envelope[ENVELOPE_MAGIC.len() + 1] != ALGORITHM_XCHACHA20_POLY1305
        {
            return Err(encryption_error("snapshot envelope algorithm is unsupported"));
        }

        let mut offset = ENVELOPE_MAGIC.len() + 2;
        let key_id_bytes = array_at::<KEY_BYTES>(envelope, offset)?;
        offset += KEY_BYTES;
        let content_hash_bytes = array_at::<HASH_BYTES>(envelope, offset)?;
        offset += HASH_BYTES;
        let nonce = array_at::<NONCE_BYTES>(envelope, offset)?;
        offset += NONCE_BYTES;

        let key_id = hex_string(&key_id_bytes);
        let content_hash = hex_string(&content_hash_bytes);
        if key_id != expected_key_id || key_id != self.key_id() {
            return Err(encryption_error("snapshot key identifier does not match"));
        }
        if content_hash != expected_content_hash {
            return Err(encryption_error("snapshot content hash does not match"));
        }

        let aad = snapshot_aad(context, encryption_version, &key_id_bytes, &content_hash_bytes)?;
        let encryption_key = self.derived_key(ENCRYPTION_KEY_INFO)?;
        let cipher = XChaCha20Poly1305::new_from_slice(encryption_key.as_slice())
            .map_err(|_| encryption_error("snapshot encryption key is invalid"))?;
        let nonce = nonce_ref(&nonce)?;
        let plaintext = cipher
            .decrypt(nonce, Payload { msg: &envelope[offset..], aad: &aad })
            .map_err(|_| encryption_error("snapshot authentication failed"))?;
        if self.content_hash(&plaintext)? != content_hash {
            return Err(encryption_error("snapshot plaintext authentication failed"));
        }
        Ok(plaintext)
    }

    pub(crate) fn bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    pub(crate) fn from_secret(bytes: Zeroizing<[u8; KEY_BYTES]>) -> Self {
        Self(bytes)
    }

    fn derived_key(&self, info: &[u8]) -> Result<Zeroizing<[u8; KEY_BYTES]>, SyncClientError> {
        let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), self.0.as_slice());
        let mut output = Zeroizing::new([0_u8; KEY_BYTES]);
        hkdf.expand(info, output.as_mut())
            .map_err(|_| encryption_error("account key derivation failed"))?;
        Ok(output)
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AccountKey").field(&"[REDACTED]").finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SnapshotCryptoContext<'a> {
    pub user_id: &'a str,
    pub vault_generation: u64,
    pub snapshot_id: &'a str,
    pub schema_rev: u32,
    pub logical_clock: u64,
    pub device_id: &'a str,
    pub head_revision: u64,
    pub base_head: Option<&'a SnapshotHeadRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotHeadRef {
    pub(crate) revision: u64,
    pub(crate) snapshot_id: String,
    pub(crate) payload_hash: String,
}

impl SnapshotHeadRef {
    pub(crate) fn new(
        revision: u64,
        snapshot_id: impl Into<String>,
        payload_hash: impl Into<String>,
    ) -> Result<Self, SyncClientError> {
        let head =
            Self { revision, snapshot_id: snapshot_id.into(), payload_hash: payload_hash.into() };
        head.validate()?;
        Ok(head)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub fn payload_hash(&self) -> &str {
        &self.payload_hash
    }

    fn validate(&self) -> Result<(), SyncClientError> {
        if self.revision == 0
            || self.revision > MAX_SAFE_INTEGER
            || self.snapshot_id.is_empty()
            || self.snapshot_id.len() > 128
            || !self.snapshot_id.as_bytes()[0].is_ascii_lowercase()
                && !self.snapshot_id.as_bytes()[0].is_ascii_digit()
            || !self.snapshot_id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
            || decode_hex_32(&self.payload_hash).is_err()
        {
            return Err(encryption_error("snapshot head reference is invalid"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedSnapshot {
    bytes: Vec<u8>,
    key_id: String,
    content_hash: String,
}

impl EncryptedSnapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

fn snapshot_aad(
    context: &SnapshotCryptoContext<'_>,
    encryption_version: u32,
    key_id: &[u8; KEY_BYTES],
    content_hash: &[u8; HASH_BYTES],
) -> Result<Vec<u8>, SyncClientError> {
    let domain = if encryption_version == 1 { AAD_DOMAIN_V1 } else { AAD_DOMAIN_V2 };
    let mut aad = Vec::with_capacity(domain.len() + 3 * 130 + 128);
    aad.extend_from_slice(domain);
    push_text(&mut aad, context.user_id)?;
    aad.extend_from_slice(&context.vault_generation.to_be_bytes());
    push_text(&mut aad, context.snapshot_id)?;
    aad.extend_from_slice(&context.schema_rev.to_be_bytes());
    aad.extend_from_slice(&context.logical_clock.to_be_bytes());
    push_text(&mut aad, context.device_id)?;
    aad.extend_from_slice(key_id);
    aad.extend_from_slice(content_hash);
    if encryption_version == SNAPSHOT_ENCRYPTION_VERSION {
        push_head_lineage(&mut aad, context)?;
    }
    Ok(aad)
}

fn push_head_lineage(
    aad: &mut Vec<u8>,
    context: &SnapshotCryptoContext<'_>,
) -> Result<(), SyncClientError> {
    if context.head_revision == 0 {
        return Err(encryption_error("snapshot head revision is invalid"));
    }
    aad.extend_from_slice(&context.head_revision.to_be_bytes());
    match context.base_head {
        None if context.head_revision == 1 => aad.push(0),
        Some(base) if base.revision.checked_add(1) == Some(context.head_revision) => {
            base.validate()?;
            aad.push(1);
            aad.extend_from_slice(&base.revision.to_be_bytes());
            push_text(aad, &base.snapshot_id)?;
            aad.extend_from_slice(&decode_hex_32(&base.payload_hash)?);
        }
        _ => return Err(encryption_error("snapshot head lineage is invalid")),
    }
    Ok(())
}

fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), SyncClientError> {
    let length = u16::try_from(value.len())
        .map_err(|_| encryption_error("snapshot authenticated metadata is too long"))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], SyncClientError> {
    bytes
        .get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| encryption_error("snapshot envelope is truncated"))
}

fn nonce_ref(bytes: &[u8; NONCE_BYTES]) -> Result<&XNonce, SyncClientError> {
    bytes.as_slice().try_into().map_err(|_| encryption_error("snapshot nonce is invalid"))
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], SyncClientError> {
    if value.len() != 64 {
        return Err(encryption_error("snapshot hash encoding is invalid"));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, SyncClientError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(encryption_error("snapshot hash encoding is invalid")),
    }
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encryption_error(reason: &'static str) -> SyncClientError {
    SyncClientError::SnapshotEncryption { reason }
}

#[cfg(test)]
#[path = "encryption_tests.rs"]
mod tests;
