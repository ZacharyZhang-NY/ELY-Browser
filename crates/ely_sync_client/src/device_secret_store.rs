use zeroize::{Zeroize, Zeroizing};

use crate::{
    SyncClientError,
    credential_store::{clear_secret, load_secret, save_secret},
    device::is_device_id_shape,
};

const KEYCHAIN_SERVICE: &str = "com.elydora.ely-browser.sync.device-secrets.v2";
const RECORD_VERSION: u8 = 2;
const SECRET_BYTES: usize = 32;
const RECORD_BYTES: usize = 1 + 2 * SECRET_BYTES;

#[derive(Clone, Debug)]
pub struct DeviceSecretStore {
    device_id: String,
}

impl DeviceSecretStore {
    pub fn new(device_id: impl Into<String>) -> Result<Self, SyncClientError> {
        let device_id = device_id.into();
        if !is_device_id_shape(&device_id) {
            return Err(storage_error("device identifier is invalid"));
        }
        Ok(Self { device_id })
    }

    pub(crate) fn load(&self) -> Result<Option<DeviceSecrets>, SyncClientError> {
        match load_secret(KEYCHAIN_SERVICE, &self.device_id).map_err(storage_error)? {
            Some(record) => decode_secret_record(record).map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn load_required(&self) -> Result<DeviceSecrets, SyncClientError> {
        self.load()?.ok_or_else(|| SyncClientError::DeviceKeyUnavailable {
            device_id: self.device_id.clone(),
        })
    }

    pub(crate) fn save(&self, secrets: &DeviceSecrets) -> Result<(), SyncClientError> {
        let record = encode_secret_record(secrets);
        save_secret(KEYCHAIN_SERVICE, &self.device_id, record.as_slice()).map_err(storage_error)
    }

    pub fn clear(&self) -> Result<(), SyncClientError> {
        clear_secret(KEYCHAIN_SERVICE, &self.device_id).map_err(storage_error)
    }
}

pub(crate) struct DeviceSecrets {
    signing_private_key: Zeroizing<[u8; SECRET_BYTES]>,
    wrapping_private_key: Zeroizing<[u8; SECRET_BYTES]>,
}

impl DeviceSecrets {
    pub(crate) fn new(
        signing_private_key: [u8; SECRET_BYTES],
        wrapping_private_key: [u8; SECRET_BYTES],
    ) -> Self {
        Self {
            signing_private_key: Zeroizing::new(signing_private_key),
            wrapping_private_key: Zeroizing::new(wrapping_private_key),
        }
    }

    pub(crate) fn signing_private_key(&self) -> &[u8; SECRET_BYTES] {
        &self.signing_private_key
    }

    pub(crate) fn wrapping_private_key(&self) -> &[u8; SECRET_BYTES] {
        &self.wrapping_private_key
    }
}

fn encode_secret_record(secrets: &DeviceSecrets) -> Zeroizing<[u8; RECORD_BYTES]> {
    let mut record = Zeroizing::new([0_u8; RECORD_BYTES]);
    record[0] = RECORD_VERSION;
    record[1..1 + SECRET_BYTES].copy_from_slice(secrets.signing_private_key());
    record[1 + SECRET_BYTES..].copy_from_slice(secrets.wrapping_private_key());
    record
}

fn decode_secret_record(mut record: Zeroizing<Vec<u8>>) -> Result<DeviceSecrets, SyncClientError> {
    if record.len() != RECORD_BYTES || record[0] != RECORD_VERSION {
        return Err(storage_error("device secret record is invalid"));
    }
    let mut signing_private_key = [0_u8; SECRET_BYTES];
    let mut wrapping_private_key = [0_u8; SECRET_BYTES];
    signing_private_key.copy_from_slice(&record[1..1 + SECRET_BYTES]);
    wrapping_private_key.copy_from_slice(&record[1 + SECRET_BYTES..]);
    record.zeroize();
    Ok(DeviceSecrets::new(signing_private_key, wrapping_private_key))
}

fn storage_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::DeviceKeyStorage(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_record_round_trips_both_private_keys() -> Result<(), SyncClientError> {
        let secrets = DeviceSecrets::new([7; SECRET_BYTES], [19; SECRET_BYTES]);
        let record = encode_secret_record(&secrets);
        let decoded = decode_secret_record(Zeroizing::new(record.to_vec()))?;

        assert_eq!(decoded.signing_private_key(), &[7; SECRET_BYTES]);
        assert_eq!(decoded.wrapping_private_key(), &[19; SECRET_BYTES]);
        Ok(())
    }

    #[test]
    fn secret_record_rejects_unknown_versions() {
        let mut record = vec![0_u8; RECORD_BYTES];
        record[0] = RECORD_VERSION + 1;
        assert!(decode_secret_record(Zeroizing::new(record)).is_err());
    }
}
