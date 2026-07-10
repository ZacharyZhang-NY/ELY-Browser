use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    DeviceIdentity, DeviceRecord, SyncClientError,
    device_proof::{is_idempotency_key_shape, push_field},
    vault::WrappedAccountKey,
};

const REBIND_CHALLENGE_VERSION: u32 = 1;
const MAX_CHALLENGE_LIFETIME_SECONDS: u64 = 600;

#[derive(Debug, Serialize)]
pub(crate) struct DeviceRebindChallengeRequest<'a> {
    pub version: u32,
    pub device_id: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceRebindChallengeDocument {
    pub version: u32,
    pub challenge_id: String,
    pub device_id: String,
    pub challenge: String,
    pub expires_at: u64,
}

impl DeviceRebindChallengeDocument {
    pub(crate) fn signed_request(
        &self,
        identity: &DeviceIdentity,
        now_seconds: u64,
    ) -> Result<DeviceRebindRequest, SyncClientError> {
        let _ = self.validated_context(identity, now_seconds)?;
        Ok(DeviceRebindRequest {
            version: REBIND_CHALLENGE_VERSION,
            challenge_id: self.challenge_id.clone(),
            device_id: self.device_id.clone(),
            signature: identity.sign_message(self.challenge.as_bytes())?,
        })
    }

    fn validated_context<'a>(
        &'a self,
        identity: &DeviceIdentity,
        now_seconds: u64,
    ) -> Result<(&'a str, &'a str), SyncClientError> {
        if self.version != REBIND_CHALLENGE_VERSION || self.device_id != identity.device_id {
            return Err(protocol_error("device rebind challenge identity does not match"));
        }
        if self.expires_at <= now_seconds
            || self.expires_at > now_seconds.saturating_add(MAX_CHALLENGE_LIFETIME_SECONDS)
        {
            return Err(protocol_error("device rebind challenge expiry is invalid"));
        }
        let challenge_id = Uuid::parse_str(&self.challenge_id)
            .map_err(|_| protocol_error("device rebind challenge identifier is invalid"))?;
        if challenge_id.hyphenated().to_string() != self.challenge_id {
            return Err(protocol_error("device rebind challenge identifier is not canonical"));
        }

        let lines = self.challenge.split('\n').collect::<Vec<_>>();
        if lines.len() != 7 || lines[0] != "elydora-device-rebind-v1" {
            return Err(protocol_error("device rebind challenge format is invalid"));
        }
        assert_challenge_field(lines[1], "challenge_id", &self.challenge_id)?;
        let user_id = challenge_value(lines[2], "user_id")?;
        let session_id = challenge_value(lines[3], "session_id")?;
        assert_challenge_field(lines[4], "device_id", &self.device_id)?;
        assert_challenge_field(lines[5], "expires_at", &self.expires_at.to_string())?;
        let nonce = challenge_value(lines[6], "nonce")?;
        if !is_lower_hex(nonce, 64) || !is_subject_id(user_id) || !is_subject_id(session_id) {
            return Err(protocol_error("device rebind challenge value is invalid"));
        }
        Ok((user_id, session_id))
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct DeviceRebindRequest {
    pub version: u32,
    pub challenge_id: String,
    pub device_id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRebindDocument {
    pub version: u32,
    pub user_id: String,
    pub session_id: String,
    pub device_id: String,
    pub bound_at: u64,
}

impl DeviceRebindDocument {
    pub(crate) fn validate(
        &self,
        identity: &DeviceIdentity,
        challenge: &DeviceRebindChallengeDocument,
        now_seconds: u64,
    ) -> Result<(), SyncClientError> {
        let (user_id, session_id) = challenge.validated_context(identity, now_seconds)?;
        if self.version != REBIND_CHALLENGE_VERSION
            || self.device_id != identity.device_id
            || self.user_id != user_id
            || self.session_id != session_id
            || self.bound_at > challenge.expires_at
        {
            return Err(protocol_error("device rebind response does not match challenge"));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct DeviceApprovalRequest<'a> {
    pub version: u32,
    pub device_id: &'a str,
    pub key_id: &'a str,
    pub generation: u64,
    pub envelope: &'a WrappedAccountKey,
    pub idempotency_key: &'a str,
    pub proof_created_at: u64,
    pub approval_proof: String,
}

impl<'a> DeviceApprovalRequest<'a> {
    pub fn new(
        user_id: &str,
        approver: &DeviceIdentity,
        device_id: &'a str,
        key_id: &'a str,
        generation: u64,
        envelope: &'a WrappedAccountKey,
        idempotency_key: &'a str,
    ) -> Result<Self, SyncClientError> {
        let proof_created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| protocol_error("system clock is invalid"))?
            .as_secs();
        let message = approval_proof_message(&ApprovalProofFields {
            user_id,
            approver_device_id: &approver.device_id,
            device_id,
            key_id,
            generation,
            envelope,
            idempotency_key,
            proof_created_at,
        })?;
        Ok(Self {
            version: 2,
            device_id,
            key_id,
            generation,
            envelope,
            idempotency_key,
            proof_created_at,
            approval_proof: approver.sign_message(&message)?,
        })
    }
}

struct ApprovalProofFields<'a> {
    user_id: &'a str,
    approver_device_id: &'a str,
    device_id: &'a str,
    key_id: &'a str,
    generation: u64,
    envelope: &'a WrappedAccountKey,
    idempotency_key: &'a str,
    proof_created_at: u64,
}

fn approval_proof_message(fields: &ApprovalProofFields<'_>) -> Result<Vec<u8>, SyncClientError> {
    if fields.user_id.is_empty()
        || fields.generation == 0
        || !is_idempotency_key_shape(fields.idempotency_key)
        || fields.key_id.len() != 64
    {
        return Err(protocol_error("device approval proof fields are invalid"));
    }
    fields.envelope.validate_wire()?;
    let generation = fields.generation.to_string();
    let envelope_version = fields.envelope.version.to_string();
    let proof_created_at = fields.proof_created_at.to_string();
    let mut message = Vec::with_capacity(512);
    for field in [
        "elydora-device-approval-v2",
        fields.user_id,
        fields.approver_device_id,
        fields.device_id,
        fields.key_id,
        &generation,
        &envelope_version,
        &fields.envelope.suite,
        &fields.envelope.encapped_key,
        &fields.envelope.ciphertext,
        fields.idempotency_key,
        &proof_created_at,
    ] {
        push_field(&mut message, field);
    }
    Ok(message)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceApprovalDocument {
    pub version: u32,
    pub user_id: String,
    pub approved_by_device_id: String,
    pub approved_at: u64,
    pub device: DeviceRecord,
}

fn assert_challenge_field(
    line: &str,
    name: &'static str,
    expected: &str,
) -> Result<(), SyncClientError> {
    if challenge_value(line, name)? != expected {
        return Err(protocol_error("device rebind challenge binding does not match"));
    }
    Ok(())
}

fn challenge_value<'a>(line: &'a str, name: &'static str) -> Result<&'a str, SyncClientError> {
    let value = line
        .strip_prefix(name)
        .and_then(|suffix| suffix.strip_prefix('='))
        .ok_or_else(|| protocol_error("device rebind challenge field is invalid"))?;
    if value.is_empty() {
        return Err(protocol_error("device rebind challenge field is empty"));
    }
    Ok(value)
}

fn is_subject_id(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn protocol_error(reason: &'static str) -> SyncClientError {
    SyncClientError::DeviceTrust { reason }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn identity() -> DeviceIdentity {
        DeviceIdentity {
            device_id: "ely-018f0f4fbbcc7f36a241d1a2a1f01111".to_string(),
            public_key: "01".repeat(32),
            wrapping_public_key: "02".repeat(32),
            device_name: "Test".to_string(),
            platform: "macos".to_string(),
        }
    }

    fn challenge() -> DeviceRebindChallengeDocument {
        let challenge_id = "018f0f4f-bbcc-7f36-a241-d1a2a1f01111";
        let device_id = identity().device_id;
        let expires_at = 1_000;
        DeviceRebindChallengeDocument {
            version: 1,
            challenge_id: challenge_id.to_string(),
            device_id: device_id.clone(),
            challenge: format!(
                "elydora-device-rebind-v1\nchallenge_id={challenge_id}\nuser_id=user-01\nsession_id=session-01\ndevice_id={device_id}\nexpires_at={expires_at}\nnonce={}",
                "ab".repeat(32)
            ),
            expires_at,
        }
    }

    #[test]
    fn canonical_rebind_challenge_binds_device_and_session() -> Result<(), SyncClientError> {
        let challenge = challenge();
        let identity = identity();
        assert_eq!(challenge.validated_context(&identity, 700)?, ("user-01", "session-01"));
        Ok(())
    }

    #[test]
    fn rebind_challenge_rejects_metadata_changes() {
        let identity = identity();
        for changed in [
            DeviceRebindChallengeDocument { device_id: "device-02".to_string(), ..challenge() },
            DeviceRebindChallengeDocument { expires_at: 701, ..challenge() },
            DeviceRebindChallengeDocument {
                challenge: challenge().challenge.replace("nonce=ab", "nonce=Ab"),
                ..challenge()
            },
        ] {
            assert!(changed.validated_context(&identity, 700).is_err());
        }
    }

    #[test]
    fn approval_proof_uses_the_frozen_worker_field_order() -> Result<(), SyncClientError> {
        let envelope = WrappedAccountKey {
            version: 1,
            suite: crate::ACCOUNT_KEY_WRAP_SUITE.to_string(),
            encapped_key: "A".repeat(43),
            ciphertext: "B".repeat(64),
        };
        let key_id = "a".repeat(64);
        let message = approval_proof_message(&ApprovalProofFields {
            user_id: "user-01",
            approver_device_id: "device-01",
            device_id: "device-02",
            key_id: &key_id,
            generation: 1,
            envelope: &envelope,
            idempotency_key: "device-approval-0001",
            proof_created_at: 1_780_000_300,
        })?;
        let fields = [
            "elydora-device-approval-v2".to_string(),
            "user-01".to_string(),
            "device-01".to_string(),
            "device-02".to_string(),
            "a".repeat(64),
            "1".to_string(),
            "1".to_string(),
            crate::ACCOUNT_KEY_WRAP_SUITE.to_string(),
            "A".repeat(43),
            "B".repeat(64),
            "device-approval-0001".to_string(),
            "1780000300".to_string(),
        ];
        let expected =
            fields.iter().map(|field| format!("{}:{field}", field.len())).collect::<String>();
        assert_eq!(message, expected.as_bytes());
        let private_key = crate::device::decode_hex_32(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "test private key is invalid",
        )?;
        let signature = SigningKey::from_bytes(&private_key).sign(&message);
        let signature_hex =
            signature.to_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        assert_eq!(
            signature_hex,
            "f12fb7a5f7f20551bd22d0fcf8f5787d49f6202f89e42c332c248772fd9a59c82a9d8b6ac47ea84340170fc1555fc74d70a0d6ba3541df257882d46d6d79d901"
        );
        Ok(())
    }
}
