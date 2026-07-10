use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    encryption::{
        AccountKey, EncryptedSnapshot, SNAPSHOT_ENCRYPTION_VERSION, SnapshotCryptoContext,
        SnapshotHeadRef,
    },
    error::SyncClientError,
};

/// Hard cap from `cloudflare/src/sync_snapshot.ts`: a single snapshot
/// upload may not exceed 10 MiB. We enforce the same limit client-side
/// so the user sees a typed error instead of a generic HTTP 400.
pub const MAX_SNAPSHOT_BYTES: usize = 10 * 1024 * 1024;

/// Validated snapshot payload, ready to ship to `/api/sync/snapshot`.
#[derive(Clone, Debug)]
pub struct SnapshotPayload {
    bytes: Vec<u8>,
    payload_hash: String,
}

impl SnapshotPayload {
    /// Wrap `bytes` and pre-compute the SHA-256 the worker validates
    /// before persisting the R2 object. Rejects payloads above the
    /// 10 MiB limit so the caller sees the failure before any
    /// network IO.
    pub fn new(bytes: Vec<u8>) -> Result<Self, SyncClientError> {
        if bytes.is_empty() {
            return Err(SyncClientError::SnapshotTooLarge { bytes: 0, limit: MAX_SNAPSHOT_BYTES });
        }
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SyncClientError::SnapshotTooLarge {
                bytes: bytes.len(),
                limit: MAX_SNAPSHOT_BYTES,
            });
        }
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let payload_hash = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        Ok(Self { bytes, payload_hash })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn payload_hash(&self) -> &str {
        &self.payload_hash
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SnapshotUploadRequest<'a> {
    pub version: u32,
    pub snapshot_id: &'a str,
    pub region: &'a str,
    pub payload_hash: &'a str,
    pub encryption_version: u32,
    pub vault_generation: u64,
    pub key_id: &'a str,
    pub content_hash: &'a str,
    pub schema_rev: u32,
    pub logical_clock: u64,
    pub head_revision: u64,
    pub base_head: Option<&'a SnapshotHeadRef>,
    pub data_base64: String,
}

impl<'a> SnapshotUploadRequest<'a> {
    pub fn new(
        region: &'a str,
        context: &SnapshotCryptoContext<'a>,
        base_head: Option<&'a AuthenticatedSnapshotHead>,
        encrypted: &'a EncryptedSnapshot,
        payload: &'a SnapshotPayload,
    ) -> Result<Self, SyncClientError> {
        let head_revision = match base_head {
            Some(base) => base.next_revision()?,
            None => 1,
        };
        if context.head_revision != head_revision
            || context.base_head != base_head.map(AuthenticatedSnapshotHead::head_ref)
        {
            return Err(SyncClientError::SnapshotEncryption {
                reason: "snapshot upload context does not match authenticated base",
            });
        }
        Ok(Self {
            version: 3,
            snapshot_id: context.snapshot_id,
            region,
            payload_hash: payload.payload_hash(),
            encryption_version: SNAPSHOT_ENCRYPTION_VERSION,
            vault_generation: context.vault_generation,
            key_id: encrypted.key_id(),
            content_hash: encrypted.content_hash(),
            schema_rev: context.schema_rev,
            logical_clock: context.logical_clock,
            head_revision,
            base_head: base_head.map(AuthenticatedSnapshotHead::head_ref),
            data_base64: encode_base64(payload.bytes()),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SnapshotDownload {
    pub version: u32,
    pub user_id: String,
    pub device_id: String,
    pub snapshot: SnapshotDocument,
    pub data_base64: String,
}

impl SnapshotDownload {
    /// Decode + hash-verify the returned base64 payload against the
    /// snapshot's advertised `payload_hash`. Mirrors the contract the
    /// worker enforces on upload — we re-check on download so a
    /// tampered storage layer doesn't silently desync the user.
    pub fn payload(&self) -> Result<SnapshotPayload, SyncClientError> {
        if self.data_base64.len() > MAX_SNAPSHOT_BYTES.div_ceil(3) * 4 {
            return Err(SyncClientError::SnapshotTooLarge {
                bytes: self.data_base64.len(),
                limit: MAX_SNAPSHOT_BYTES.div_ceil(3) * 4,
            });
        }
        let bytes = decode_base64(&self.data_base64)
            .map_err(|error| SyncClientError::SnapshotBase64(error.to_string()))?;
        let payload = SnapshotPayload::new(bytes)?;
        if payload.payload_hash() != self.snapshot.payload_hash {
            return Err(SyncClientError::SnapshotBase64(
                "downloaded snapshot hash does not match server-advertised hash".to_string(),
            ));
        }
        Ok(payload)
    }

    pub fn authenticate(
        &self,
        expected_head: &SnapshotHeadRef,
        key: &AccountKey,
    ) -> Result<AuthenticatedSnapshot, SyncClientError> {
        if self.version != 3 {
            return Err(SyncClientError::SnapshotEncryption {
                reason: "snapshot response version is unsupported",
            });
        }
        let actual_head = self.snapshot.head_ref()?;
        if &actual_head != expected_head {
            return Err(SyncClientError::SnapshotEncryption {
                reason: "snapshot response head does not match request",
            });
        }
        let payload = self.payload()?;
        let context = SnapshotCryptoContext {
            user_id: &self.user_id,
            vault_generation: self.snapshot.vault_generation,
            snapshot_id: &self.snapshot.snapshot_id,
            schema_rev: self.snapshot.schema_rev,
            logical_clock: self.snapshot.logical_clock,
            device_id: &self.snapshot.device_id,
            head_revision: self.snapshot.head_revision,
            base_head: self.snapshot.base_head.as_ref(),
        };
        let plaintext = key.decrypt(
            &context,
            self.snapshot.encryption_version,
            &self.snapshot.key_id,
            &self.snapshot.content_hash,
            payload.bytes(),
        )?;
        Ok(AuthenticatedSnapshot {
            plaintext,
            head: AuthenticatedSnapshotHead {
                head: actual_head,
                logical_clock: self.snapshot.logical_clock,
                content_hash: self.snapshot.content_hash.clone(),
                vault_generation: self.snapshot.vault_generation,
                key_id: self.snapshot.key_id.clone(),
                device_id: self.snapshot.device_id.clone(),
                size_bytes: self.snapshot.size_bytes,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SnapshotDocument {
    pub snapshot_id: String,
    pub r2_key: String,
    pub payload_hash: String,
    pub encryption_version: u32,
    pub vault_generation: u64,
    pub key_id: String,
    pub content_hash: String,
    pub schema_rev: u32,
    pub logical_clock: u64,
    pub head_revision: u64,
    pub base_head: Option<SnapshotHeadRef>,
    pub device_id: String,
    pub size_bytes: u64,
    pub created_at: u64,
}

impl SnapshotDocument {
    fn head_ref(&self) -> Result<SnapshotHeadRef, SyncClientError> {
        SnapshotHeadRef::new(
            self.head_revision,
            self.snapshot_id.clone(),
            self.payload_hash.clone(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedSnapshotHead {
    head: SnapshotHeadRef,
    logical_clock: u64,
    content_hash: String,
    vault_generation: u64,
    key_id: String,
    device_id: String,
    size_bytes: u64,
}

impl AuthenticatedSnapshotHead {
    pub fn revision(&self) -> u64 {
        self.head.revision()
    }

    pub fn logical_clock(&self) -> u64 {
        self.logical_clock
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    pub fn vault_generation(&self) -> u64 {
        self.vault_generation
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn snapshot_id(&self) -> &str {
        self.head.snapshot_id()
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn next_revision(&self) -> Result<u64, SyncClientError> {
        if self.head.revision >= 9_007_199_254_740_991 {
            return Err(SyncClientError::SnapshotEncryption {
                reason: "snapshot head revision exceeds the wire limit",
            });
        }
        self.head.revision.checked_add(1).ok_or(SyncClientError::SnapshotEncryption {
            reason: "snapshot head revision overflowed",
        })
    }

    pub fn head_ref(&self) -> &SnapshotHeadRef {
        &self.head
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedSnapshot {
    plaintext: Vec<u8>,
    head: AuthenticatedSnapshotHead,
}

impl AuthenticatedSnapshot {
    pub fn into_parts(self) -> (Vec<u8>, AuthenticatedSnapshotHead) {
        (self.plaintext, self.head)
    }
}

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let len = chunk.len();
        let b0 = chunk[0];
        let b1 = if len > 1 { chunk[1] } else { 0 };
        let b2 = if len > 2 { chunk[2] } else { 0 };
        out.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
        out.push(BASE64_CHARS[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if len > 1 {
            BASE64_CHARS[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if len > 2 { BASE64_CHARS[(b2 & 0b111111) as usize] as char } else { '=' });
    }
    out
}

#[derive(Debug)]
pub(crate) struct Base64DecodeError(pub String);

impl std::fmt::Display for Base64DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn decode_base64(value: &str) -> Result<Vec<u8>, Base64DecodeError> {
    let trimmed = value.trim();
    if !trimmed.len().is_multiple_of(4) {
        return Err(Base64DecodeError("input length is not a multiple of 4".to_string()));
    }
    let mut out = Vec::with_capacity(trimmed.len() / 4 * 3);
    let mut buffer = [0u32; 4];
    let mut buffer_len = 0;
    let mut padding = 0;
    for byte in trimmed.bytes() {
        let value = match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padding += 1;
                if padding > 2 {
                    return Err(Base64DecodeError("excess padding".to_string()));
                }
                0
            }
            _ => return Err(Base64DecodeError(format!("unexpected byte {byte:#x}"))),
        };
        buffer[buffer_len] = value;
        buffer_len += 1;
        if buffer_len == 4 {
            let combined = (buffer[0] << 18) | (buffer[1] << 12) | (buffer[2] << 6) | buffer[3];
            out.push(((combined >> 16) & 0xFF) as u8);
            if padding < 2 {
                out.push(((combined >> 8) & 0xFF) as u8);
            }
            if padding < 1 {
                out.push((combined & 0xFF) as u8);
            }
            buffer_len = 0;
            padding = 0;
        }
    }
    if buffer_len != 0 {
        return Err(Base64DecodeError("trailing partial quartet".to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_rejects_empty_and_oversized() {
        assert!(SnapshotPayload::new(vec![]).is_err());
        let too_big = vec![0u8; MAX_SNAPSHOT_BYTES + 1];
        assert!(SnapshotPayload::new(too_big).is_err());
    }

    #[test]
    fn payload_hash_matches_sha256() -> Result<(), SyncClientError> {
        let payload = SnapshotPayload::new(b"ely".to_vec())?;
        // Real SHA-256 of "ely" — verified at test time so a future
        // hashing regression is caught immediately.
        assert_eq!(
            payload.payload_hash(),
            "52fe440ae87b395cc46438ee9275fcadc0f98a048161345f53d6b0038976a6f5"
        );
        Ok(())
    }

    #[test]
    fn base64_round_trip() -> Result<(), Base64DecodeError> {
        for sample in [b"".as_slice(), b"a", b"ab", b"abc", b"ely browser"] {
            let encoded = encode_base64(sample);
            let decoded = decode_base64(&encoded)?;
            assert_eq!(decoded.as_slice(), sample);
        }
        Ok(())
    }

    #[test]
    fn upload_request_serializes_encrypted_wire_v3() -> Result<(), SyncClientError> {
        let key = AccountKey::from_bytes([23; 32]);
        let context = SnapshotCryptoContext {
            user_id: "user-01",
            vault_generation: 1,
            snapshot_id: "device-01",
            schema_rev: 1,
            logical_clock: 42,
            device_id: "device-01",
            head_revision: 1,
            base_head: None,
        };
        let encrypted = key.encrypt(&context, br#"{"secret":"value"}"#)?;
        let payload = SnapshotPayload::new(encrypted.bytes().to_vec())?;
        let request = SnapshotUploadRequest::new("auto", &context, None, &encrypted, &payload)?;
        let value = serde_json::to_value(request)
            .map_err(|source| SyncClientError::Json { endpoint: "test".to_string(), source })?;

        assert_eq!(value["version"], 3);
        assert_eq!(value["encryption_version"], SNAPSHOT_ENCRYPTION_VERSION);
        assert_eq!(value["head_revision"], 1);
        assert!(value["base_head"].is_null());
        assert_eq!(value["key_id"], encrypted.key_id());
        assert_eq!(value["content_hash"], encrypted.content_hash());
        assert!(!value["data_base64"].as_str().unwrap_or_default().contains("secret"));
        Ok(())
    }

    #[test]
    fn downloaded_head_becomes_a_merge_base_after_authentication() -> Result<(), SyncClientError> {
        let key = AccountKey::from_bytes([25; 32]);
        let expected_head = SnapshotHeadRef::new(1, "device-01", "00".repeat(32))?;
        let context = SnapshotCryptoContext {
            user_id: "user-01",
            vault_generation: 1,
            snapshot_id: expected_head.snapshot_id(),
            schema_rev: 1,
            logical_clock: 42,
            device_id: "device-01",
            head_revision: 1,
            base_head: None,
        };
        let encrypted = key.encrypt(&context, b"authenticated head")?;
        let payload = SnapshotPayload::new(encrypted.bytes().to_vec())?;
        let expected_head =
            SnapshotHeadRef::new(1, context.snapshot_id, payload.payload_hash().to_string())?;
        let download = SnapshotDownload {
            version: 3,
            user_id: context.user_id.to_string(),
            device_id: context.device_id.to_string(),
            snapshot: SnapshotDocument {
                snapshot_id: context.snapshot_id.to_string(),
                r2_key: "sync-snapshots/test".to_string(),
                payload_hash: payload.payload_hash().to_string(),
                encryption_version: SNAPSHOT_ENCRYPTION_VERSION,
                vault_generation: context.vault_generation,
                key_id: encrypted.key_id().to_string(),
                content_hash: encrypted.content_hash().to_string(),
                schema_rev: context.schema_rev,
                logical_clock: context.logical_clock,
                head_revision: context.head_revision,
                base_head: None,
                device_id: context.device_id.to_string(),
                size_bytes: u64::try_from(payload.bytes().len()).unwrap_or_default(),
                created_at: 1,
            },
            data_base64: encode_base64(payload.bytes()),
        };
        let (plaintext, authenticated_head) =
            download.authenticate(&expected_head, &key)?.into_parts();

        assert_eq!(plaintext, b"authenticated head");
        assert_eq!(authenticated_head.revision(), 1);
        assert_eq!(authenticated_head.content_hash(), encrypted.content_hash());
        assert!(
            download
                .authenticate(
                    &SnapshotHeadRef::new(1, "device-02", payload.payload_hash().to_string())?,
                    &key,
                )
                .is_err()
        );
        Ok(())
    }
}
