use sha2::{Digest, Sha256};

use crate::{DeviceIdentity, DeviceRecord, SyncClientError, device::decode_hex_32};

const REGISTRATION_PROOF_DOMAIN: &str = "elydora-device-registration-v2";
const VERIFICATION_CODE_DOMAIN: &str = "elydora-device-verification-v1";

impl DeviceIdentity {
    pub fn registration_proof(&self, idempotency_key: &str) -> Result<String, SyncClientError> {
        let message = self.registration_proof_message(idempotency_key)?;
        self.sign_message(&message)
    }

    pub fn verification_code(&self) -> Result<String, SyncClientError> {
        self.validate()?;
        verification_code(
            &self.device_id,
            &self.public_key,
            &self.wrapping_public_key,
            &self.device_name,
            &self.platform,
        )
    }

    fn registration_proof_message(
        &self,
        idempotency_key: &str,
    ) -> Result<Vec<u8>, SyncClientError> {
        self.validate()?;
        if !is_idempotency_key_shape(idempotency_key) {
            return Err(proof_error("device registration idempotency key is invalid"));
        }
        let fields = [
            REGISTRATION_PROOF_DOMAIN,
            &self.device_id,
            &self.public_key,
            &self.wrapping_public_key,
            &self.device_name,
            &self.platform,
            idempotency_key,
        ];
        let mut message = Vec::with_capacity(512);
        for field in fields {
            push_field(&mut message, field);
        }
        Ok(message)
    }
}

impl DeviceRecord {
    pub fn verification_code(&self) -> Result<String, SyncClientError> {
        let wrapping_public_key = self
            .wrapping_public_key
            .as_deref()
            .ok_or_else(|| proof_error("device wrapping public key is unavailable"))?;
        verification_code(
            &self.device_id,
            &self.public_key,
            wrapping_public_key,
            &self.device_name,
            &self.platform,
        )
    }
}

fn verification_code(
    device_id: &str,
    public_key: &str,
    wrapping_public_key: &str,
    device_name: &str,
    platform: &str,
) -> Result<String, SyncClientError> {
    if !(3..=128).contains(&device_id.len())
        || !device_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(proof_error("device identifier is invalid"));
    }
    decode_hex_32(public_key, "device signing public key encoding is invalid")?;
    decode_hex_32(wrapping_public_key, "device wrapping public key encoding is invalid")?;
    let fields = [
        VERIFICATION_CODE_DOMAIN,
        device_id,
        public_key,
        wrapping_public_key,
        device_name,
        platform,
    ];
    let mut message = Vec::with_capacity(512);
    for field in fields {
        push_field(&mut message, field);
    }
    let digest = Sha256::digest(message);
    Ok(digest[..8]
        .chunks_exact(2)
        .map(|chunk| format!("{:02X}{:02X}", chunk[0], chunk[1]))
        .collect::<Vec<_>>()
        .join("-"))
}

pub(crate) fn is_idempotency_key_shape(value: &str) -> bool {
    (16..=128).contains(&value.len())
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b':' | b'-'))
}

pub(crate) fn push_field(message: &mut Vec<u8>, value: &str) {
    message.extend_from_slice(value.len().to_string().as_bytes());
    message.push(b':');
    message.extend_from_slice(value.as_bytes());
}

fn proof_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::DeviceKeyStorage(message.into())
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey, Verifier};

    use super::*;
    use crate::device::generate_key_material;

    #[test]
    fn registration_proof_matches_worker_canonical_bytes() -> Result<(), SyncClientError> {
        let (identity, secrets) = generate_key_material("ELY ñ".to_string(), "macOS".to_string())?;
        let idempotency_key = "device-register:01";
        let message = identity.registration_proof_message(idempotency_key)?;
        let fields = [
            REGISTRATION_PROOF_DOMAIN,
            &identity.device_id,
            &identity.public_key,
            &identity.wrapping_public_key,
            &identity.device_name,
            &identity.platform,
            idempotency_key,
        ];
        let expected =
            fields.iter().map(|field| format!("{}:{field}", field.len())).collect::<String>();
        assert_eq!(message, expected.as_bytes());

        let signing_key = SigningKey::from_bytes(secrets.signing_private_key());
        let signature = signing_key.sign(&message);
        signing_key
            .verifying_key()
            .verify(&message, &signature)
            .map_err(|_| proof_error("device registration proof verification failed"))
    }

    #[test]
    fn verification_code_binds_every_public_identity_field() -> Result<(), SyncClientError> {
        let (identity, _) = generate_key_material("ELY ñ".to_string(), "macOS".to_string())?;
        let code = identity.verification_code()?;
        assert_eq!(code.len(), 19);

        let changed = [
            DeviceIdentity { device_id: "device-02".to_string(), ..identity.clone() },
            DeviceIdentity { public_key: "01".repeat(32), ..identity.clone() },
            DeviceIdentity { wrapping_public_key: "02".repeat(32), ..identity.clone() },
            DeviceIdentity { device_name: "Other".to_string(), ..identity.clone() },
            DeviceIdentity { platform: "linux".to_string(), ..identity.clone() },
        ];
        for candidate in changed {
            assert_ne!(candidate.verification_code()?, code);
        }
        Ok(())
    }
}
