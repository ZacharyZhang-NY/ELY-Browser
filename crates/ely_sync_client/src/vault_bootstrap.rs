use serde::Serialize;

use crate::{
    DeviceIdentity, SyncClientError,
    device::decode_hex_32,
    device_proof::{is_idempotency_key_shape, push_field},
    vault::WrappedAccountKey,
};

const BOOTSTRAP_PROOF_DOMAIN: &str = "elydora-sync-vault-bootstrap-v2";
const MAX_USER_ID_BYTES: usize = 4096;

#[derive(Clone, Debug, Serialize)]
pub struct SyncVaultBootstrapRequest<'a> {
    pub version: u32,
    pub key_id: &'a str,
    pub generation: u64,
    pub envelope: &'a WrappedAccountKey,
    pub idempotency_key: &'a str,
    pub bootstrap_proof: String,
}

impl<'a> SyncVaultBootstrapRequest<'a> {
    pub fn signed(
        user_id: &str,
        identity: &DeviceIdentity,
        key_id: &'a str,
        envelope: &'a WrappedAccountKey,
        idempotency_key: &'a str,
    ) -> Result<Self, SyncClientError> {
        let generation = 1;
        let message = bootstrap_proof_message(
            user_id,
            identity,
            key_id,
            generation,
            envelope,
            idempotency_key,
        )?;
        Ok(Self {
            version: 2,
            key_id,
            generation,
            envelope,
            idempotency_key,
            bootstrap_proof: identity.sign_message(&message)?,
        })
    }
}

fn bootstrap_proof_message(
    user_id: &str,
    identity: &DeviceIdentity,
    key_id: &str,
    generation: u64,
    envelope: &WrappedAccountKey,
    idempotency_key: &str,
) -> Result<Vec<u8>, SyncClientError> {
    if user_id.trim().is_empty() || user_id.len() > MAX_USER_ID_BYTES {
        return Err(bootstrap_error("vault user identifier is invalid"));
    }
    identity.validate()?;
    decode_hex_32(key_id, "vault account key identifier is invalid")?;
    if generation != 1 {
        return Err(bootstrap_error("vault bootstrap generation is invalid"));
    }
    envelope.validate_wire()?;
    if !is_idempotency_key_shape(idempotency_key) {
        return Err(bootstrap_error("vault bootstrap idempotency key is invalid"));
    }

    let generation = generation.to_string();
    let envelope_version = envelope.version.to_string();
    let fields = [
        BOOTSTRAP_PROOF_DOMAIN,
        user_id,
        &identity.device_id,
        key_id,
        &generation,
        &envelope_version,
        &envelope.suite,
        &envelope.encapped_key,
        &envelope.ciphertext,
        idempotency_key,
    ];
    let mut message = Vec::with_capacity(512);
    for field in fields {
        push_field(&mut message, field);
    }
    Ok(message)
}

fn bootstrap_error(reason: &'static str) -> SyncClientError {
    SyncClientError::VaultCrypto { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountKey, device::generate_key_material, vault::WrappedAccountKey};

    #[test]
    fn proof_matches_worker_canonical_bytes() -> Result<(), SyncClientError> {
        let (identity, _) = generate_key_material("Test".to_string(), "macos".to_string())?;
        let account_key = AccountKey::from_bytes([37; 32]);
        let key_id = account_key.key_id();
        let user_id = "usér-01";
        let envelope = WrappedAccountKey::self_wrap(&account_key, user_id, &identity, 1)?;
        let idempotency_key = "vault-bootstrap:01";
        let message =
            bootstrap_proof_message(user_id, &identity, &key_id, 1, &envelope, idempotency_key)?;
        let fields = [
            BOOTSTRAP_PROOF_DOMAIN.to_string(),
            user_id.to_string(),
            identity.device_id,
            key_id,
            "1".to_string(),
            envelope.version.to_string(),
            envelope.suite,
            envelope.encapped_key,
            envelope.ciphertext,
            idempotency_key.to_string(),
        ];
        let expected =
            fields.iter().map(|field| format!("{}:{field}", field.len())).collect::<String>();

        assert_eq!(message, expected.as_bytes());
        Ok(())
    }
}
