use serde::{Deserialize, Serialize};

use crate::{
    DeviceIdentity, DeviceRecord, SyncClientError,
    device::is_device_id_shape,
    device_proof::{is_idempotency_key_shape, push_field},
    vault::WrappedAccountKey,
};

const REVOCATION_PROOF_DOMAIN: &str = "elydora-device-revocation-v2";
const PENDING_REVOCATION_PROOF_DOMAIN: &str = "elydora-pending-device-revocation-v2";
const MAX_ROTATION_ENVELOPES: usize = 128;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Serialize)]
struct DeviceRevocationEnvelope {
    recipient_device_id: String,
    envelope: WrappedAccountKey,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApprovedDeviceRevocationRequest {
    version: u32,
    mode: &'static str,
    device_id: String,
    previous_key_id: String,
    previous_generation: u64,
    new_key_id: String,
    new_generation: u64,
    envelopes: Vec<DeviceRevocationEnvelope>,
    idempotency_key: String,
    rotation_proof: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PendingDeviceRevocationRequest {
    version: u32,
    mode: &'static str,
    device_id: String,
    idempotency_key: String,
    pending_revocation_proof: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum DeviceRevocationRequest {
    ApprovedRotate(ApprovedDeviceRevocationRequest),
    PendingRevoke(PendingDeviceRevocationRequest),
}

impl DeviceRevocationRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn approved_rotation(
        user_id: &str,
        approver: &DeviceIdentity,
        target_device_id: &str,
        previous_key_id: &str,
        previous_generation: u64,
        new_key_id: &str,
        new_generation: u64,
        envelopes: Vec<(String, WrappedAccountKey)>,
        idempotency_key: &str,
    ) -> Result<Self, SyncClientError> {
        let envelopes = validate_and_sort_envelopes(envelopes, target_device_id)?;
        validate_request_fields(
            user_id,
            approver,
            target_device_id,
            previous_key_id,
            previous_generation,
            new_key_id,
            new_generation,
            idempotency_key,
        )?;
        let message = revocation_proof_message(
            user_id,
            approver,
            target_device_id,
            previous_key_id,
            previous_generation,
            new_key_id,
            new_generation,
            &envelopes,
            idempotency_key,
        );
        Ok(Self::ApprovedRotate(ApprovedDeviceRevocationRequest {
            version: 2,
            mode: "approved_rotate",
            device_id: target_device_id.to_string(),
            previous_key_id: previous_key_id.to_string(),
            previous_generation,
            new_key_id: new_key_id.to_string(),
            new_generation,
            envelopes,
            idempotency_key: idempotency_key.to_string(),
            rotation_proof: approver.sign_message(&message)?,
        }))
    }

    pub fn pending(
        user_id: &str,
        approver: &DeviceIdentity,
        target_device_id: &str,
        idempotency_key: &str,
    ) -> Result<Self, SyncClientError> {
        validate_pending_fields(user_id, approver, target_device_id, idempotency_key)?;
        let message = pending_revocation_proof_message(
            user_id,
            &approver.device_id,
            target_device_id,
            idempotency_key,
        );
        Ok(Self::PendingRevoke(PendingDeviceRevocationRequest {
            version: 2,
            mode: "pending_revoke",
            device_id: target_device_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            pending_revocation_proof: approver.sign_message(&message)?,
        }))
    }
}

fn pending_revocation_proof_message(
    user_id: &str,
    approver_device_id: &str,
    target_device_id: &str,
    idempotency_key: &str,
) -> Vec<u8> {
    let mut message = Vec::with_capacity(256);
    for field in [
        PENDING_REVOCATION_PROOF_DOMAIN,
        user_id,
        approver_device_id,
        target_device_id,
        idempotency_key,
    ] {
        push_field(&mut message, field);
    }
    message
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeviceRevocationDocument {
    ApprovedRotate {
        version: u32,
        user_id: String,
        revoked_by_device_id: String,
        revoked_at: u64,
        key_id: String,
        generation: u64,
        device: DeviceRecord,
    },
    PendingRevoke {
        version: u32,
        user_id: String,
        revoked_by_device_id: String,
        revoked_at: u64,
        device: DeviceRecord,
    },
}

#[allow(clippy::too_many_arguments)]
fn validate_request_fields(
    user_id: &str,
    approver: &DeviceIdentity,
    target_device_id: &str,
    previous_key_id: &str,
    previous_generation: u64,
    new_key_id: &str,
    new_generation: u64,
    idempotency_key: &str,
) -> Result<(), SyncClientError> {
    validate_pending_fields(user_id, approver, target_device_id, idempotency_key)?;
    if !is_key_id(previous_key_id) || !is_key_id(new_key_id) || previous_key_id == new_key_id {
        return Err(protocol_error("device revocation key identifier is invalid"));
    }
    if previous_generation == 0
        || previous_generation >= MAX_SAFE_INTEGER
        || new_generation != previous_generation + 1
    {
        return Err(protocol_error("device revocation generation is invalid"));
    }
    Ok(())
}

fn validate_pending_fields(
    user_id: &str,
    approver: &DeviceIdentity,
    target_device_id: &str,
    idempotency_key: &str,
) -> Result<(), SyncClientError> {
    if user_id.trim().is_empty() || user_id.len() > 4096 {
        return Err(protocol_error("device revocation user identifier is invalid"));
    }
    approver.validate()?;
    if !is_device_id_shape(target_device_id) || target_device_id == approver.device_id {
        return Err(protocol_error("device revocation target is invalid"));
    }
    if !is_idempotency_key_shape(idempotency_key) {
        return Err(protocol_error("device revocation idempotency key is invalid"));
    }
    Ok(())
}

fn validate_and_sort_envelopes(
    envelopes: Vec<(String, WrappedAccountKey)>,
    target_device_id: &str,
) -> Result<Vec<DeviceRevocationEnvelope>, SyncClientError> {
    if envelopes.is_empty() || envelopes.len() > MAX_ROTATION_ENVELOPES {
        return Err(protocol_error("device revocation envelope count is invalid"));
    }
    let mut envelopes = envelopes
        .into_iter()
        .map(|(recipient_device_id, envelope)| {
            if !is_device_id_shape(&recipient_device_id) || recipient_device_id == target_device_id
            {
                return Err(protocol_error("device revocation envelope recipient is invalid"));
            }
            envelope.validate_wire()?;
            Ok(DeviceRevocationEnvelope { recipient_device_id, envelope })
        })
        .collect::<Result<Vec<_>, SyncClientError>>()?;
    envelopes.sort_by(|left, right| left.recipient_device_id.cmp(&right.recipient_device_id));
    if envelopes.windows(2).any(|pair| pair[0].recipient_device_id == pair[1].recipient_device_id) {
        return Err(protocol_error("device revocation envelope recipient is duplicated"));
    }
    Ok(envelopes)
}

#[allow(clippy::too_many_arguments)]
fn revocation_proof_message(
    user_id: &str,
    approver: &DeviceIdentity,
    target_device_id: &str,
    previous_key_id: &str,
    previous_generation: u64,
    new_key_id: &str,
    new_generation: u64,
    envelopes: &[DeviceRevocationEnvelope],
    idempotency_key: &str,
) -> Vec<u8> {
    let previous_generation = previous_generation.to_string();
    let new_generation = new_generation.to_string();
    let envelope_count = envelopes.len().to_string();
    let fields = [
        REVOCATION_PROOF_DOMAIN,
        user_id,
        &approver.device_id,
        target_device_id,
        previous_key_id,
        &previous_generation,
        new_key_id,
        &new_generation,
        idempotency_key,
        &envelope_count,
    ];
    let mut message = Vec::with_capacity(1024);
    for field in fields {
        push_field(&mut message, field);
    }
    for item in envelopes {
        let envelope_version = item.envelope.version.to_string();
        for field in [
            item.recipient_device_id.as_str(),
            &envelope_version,
            &item.envelope.suite,
            &item.envelope.encapped_key,
            &item.envelope.ciphertext,
        ] {
            push_field(&mut message, field);
        }
    }
    message
}

fn is_key_id(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn protocol_error(reason: &'static str) -> SyncClientError {
    SyncClientError::DeviceTrust { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccountKey, VaultContext, device::generate_key_material, vault::WrappedAccountKey,
    };

    #[test]
    fn pending_proof_matches_worker_vector() {
        let message = pending_revocation_proof_message(
            "user-01",
            "device-01",
            "device-02",
            "device-revocation-0001",
        );
        assert_eq!(
            message,
            b"36:elydora-pending-device-revocation-v27:user-019:device-019:device-0222:device-revocation-0001"
        );
    }

    #[test]
    fn proof_matches_worker_order_and_canonical_bytes() -> Result<(), SyncClientError> {
        let (approver, _) = generate_key_material("Approver".to_string(), "macos".to_string())?;
        let (recipient_b, _) = generate_key_material("B".to_string(), "macos".to_string())?;
        let (recipient_a, _) = generate_key_material("A".to_string(), "macos".to_string())?;
        let previous_key = AccountKey::from_bytes([41; 32]);
        let new_key = AccountKey::from_bytes([43; 32]);
        let new_key_id = new_key.key_id();
        let envelopes = validate_and_sort_envelopes(
            vec![
                wrapped(&new_key, &new_key_id, &approver, &recipient_b)?,
                wrapped(&new_key, &new_key_id, &approver, &recipient_a)?,
            ],
            "device-target",
        )?;
        let message = revocation_proof_message(
            "user-01",
            &approver,
            "device-target",
            &previous_key.key_id(),
            1,
            &new_key_id,
            2,
            &envelopes,
            "device-revocation:01",
        );
        assert!(envelopes[0].recipient_device_id < envelopes[1].recipient_device_id);
        let mut fields = vec![
            REVOCATION_PROOF_DOMAIN.to_string(),
            "user-01".to_string(),
            approver.device_id,
            "device-target".to_string(),
            previous_key.key_id(),
            "1".to_string(),
            new_key_id,
            "2".to_string(),
            "device-revocation:01".to_string(),
            envelopes.len().to_string(),
        ];
        for item in envelopes {
            fields.extend([
                item.recipient_device_id,
                item.envelope.version.to_string(),
                item.envelope.suite,
                item.envelope.encapped_key,
                item.envelope.ciphertext,
            ]);
        }
        let expected =
            fields.iter().map(|field| format!("{}:{field}", field.len())).collect::<String>();
        assert_eq!(message, expected.as_bytes());
        Ok(())
    }

    fn wrapped(
        key: &AccountKey,
        key_id: &str,
        approver: &DeviceIdentity,
        recipient: &DeviceIdentity,
    ) -> Result<(String, WrappedAccountKey), SyncClientError> {
        let envelope = WrappedAccountKey::wrap(
            key,
            &VaultContext {
                user_id: "user-01",
                recipient_device_id: &recipient.device_id,
                recipient_wrapping_public_key: &recipient.wrapping_public_key,
                approver_device_id: &approver.device_id,
                generation: 2,
                key_id,
            },
        )?;
        Ok((recipient.device_id.clone(), envelope))
    }
}
