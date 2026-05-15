use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::SyncClientError;

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
}

#[derive(Clone, Debug, Serialize)]
pub struct SnapshotUploadRequest<'a> {
    pub version: u32,
    pub snapshot_id: &'a str,
    pub region: &'a str,
    pub payload_hash: &'a str,
    pub schema_rev: u32,
    pub logical_clock: u64,
    pub data_base64: String,
}

impl<'a> SnapshotUploadRequest<'a> {
    pub fn new(
        snapshot_id: &'a str,
        region: &'a str,
        schema_rev: u32,
        logical_clock: u64,
        payload: &'a SnapshotPayload,
    ) -> Self {
        Self {
            version: 1,
            snapshot_id,
            region,
            payload_hash: payload.payload_hash(),
            schema_rev,
            logical_clock,
            data_base64: encode_base64(payload.bytes()),
        }
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
}

#[derive(Clone, Debug, Deserialize)]
pub struct SnapshotDocument {
    pub snapshot_id: String,
    pub r2_key: String,
    pub payload_hash: String,
    pub schema_rev: u32,
    pub logical_clock: u64,
    pub device_id: String,
    pub size_bytes: u64,
    pub created_at: u64,
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
}
