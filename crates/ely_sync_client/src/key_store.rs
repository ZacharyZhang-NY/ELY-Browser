use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    AccountKey, SyncClientError,
    credential_store::{clear_secret, load_secret, save_secret},
};

const KEYCHAIN_SERVICE: &str = "com.elydora.ely-browser.sync.account-key.v3";
const KEY_RECORD_VERSION: u8 = 3;
const KEY_RECORD_HEADER_BYTES: usize = 11;
const KEY_RECORD_ENTRY_BYTES: usize = 40;
const MAX_STORED_KEYS: usize = 1024;

#[derive(Clone, Debug)]
pub struct StoredAccountKeys {
    current_generation: u64,
    keys: BTreeMap<u64, AccountKey>,
}

impl StoredAccountKeys {
    pub fn current_generation(&self) -> u64 {
        self.current_generation
    }

    pub fn current_key(&self) -> Option<&AccountKey> {
        self.keys.get(&self.current_generation)
    }

    pub fn key(&self, generation: u64) -> Option<&AccountKey> {
        self.keys.get(&generation)
    }

    fn from_current(key: &AccountKey, generation: u64) -> Result<Self, SyncClientError> {
        assert_generation(generation)?;
        let mut keys = BTreeMap::new();
        keys.insert(generation, key.clone());
        Ok(Self { current_generation: generation, keys })
    }

    fn set_current(&mut self, key: &AccountKey, generation: u64) -> Result<bool, SyncClientError> {
        assert_generation(generation)?;
        if generation < self.current_generation {
            return Err(storage_error("sync account key generation would roll back"));
        }
        let changed = self.insert_key(key, generation)?;
        if generation == self.current_generation {
            return Ok(changed);
        }
        self.current_generation = generation;
        Ok(true)
    }

    fn insert_historical(
        &mut self,
        key: &AccountKey,
        generation: u64,
    ) -> Result<bool, SyncClientError> {
        assert_generation(generation)?;
        if generation > self.current_generation {
            return Err(storage_error("historical sync key exceeds current generation"));
        }
        self.insert_key(key, generation)
    }

    fn insert_key(&mut self, key: &AccountKey, generation: u64) -> Result<bool, SyncClientError> {
        if let Some(stored) = self.keys.get(&generation) {
            if stored.key_id() != key.key_id() {
                return Err(storage_error("sync account key changed within one generation"));
            }
            return Ok(false);
        }
        if self.keys.len() >= MAX_STORED_KEYS {
            return Err(storage_error("sync account key history is full"));
        }
        self.keys.insert(generation, key.clone());
        Ok(true)
    }
}

#[derive(Clone, Debug)]
pub struct AccountKeyStore {
    user_id: String,
    lock_path: PathBuf,
}

impl AccountKeyStore {
    pub fn new(
        user_id: impl Into<String>,
        lock_directory: impl Into<PathBuf>,
    ) -> Result<Self, SyncClientError> {
        let user_id = user_id.into();
        if user_id.trim().is_empty() {
            return Err(storage_error("sync user identifier is empty"));
        }
        let lock_name = format!("{}.lock", hex_string(&Sha256::digest(user_id.as_bytes())));
        Ok(Self { user_id, lock_path: lock_directory.into().join(lock_name) })
    }

    pub fn load(&self) -> Result<Option<StoredAccountKeys>, SyncClientError> {
        match load_secret(KEYCHAIN_SERVICE, &self.user_id).map_err(storage_error)? {
            Some(record) => decode_key_record(record).map(Some),
            None => Ok(None),
        }
    }

    pub fn save_current(&self, key: &AccountKey, generation: u64) -> Result<(), SyncClientError> {
        self.with_lock(|| {
            let Some(mut stored) = self.load()? else {
                return self.write(&StoredAccountKeys::from_current(key, generation)?);
            };
            if !stored.set_current(key, generation)? {
                return Ok(());
            }
            self.write(&stored)
        })
    }

    pub fn save_historical(
        &self,
        key: &AccountKey,
        generation: u64,
    ) -> Result<(), SyncClientError> {
        self.with_lock(|| {
            let mut stored = self
                .load()?
                .ok_or_else(|| storage_error("current sync account key is unavailable"))?;
            if !stored.insert_historical(key, generation)? {
                return Ok(());
            }
            self.write(&stored)
        })
    }

    pub fn clear(&self) -> Result<(), SyncClientError> {
        self.with_lock(|| clear_secret(KEYCHAIN_SERVICE, &self.user_id).map_err(storage_error))
    }

    fn write(&self, stored: &StoredAccountKeys) -> Result<(), SyncClientError> {
        let record = encode_key_record(stored)?;
        save_secret(KEYCHAIN_SERVICE, &self.user_id, record.as_slice()).map_err(storage_error)
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, SyncClientError>,
    ) -> Result<T, SyncClientError> {
        let lock = open_lock_file(&self.lock_path)?;
        lock.lock_exclusive().map_err(|error| storage_error(error.to_string()))?;
        let result = operation();
        let unlock_result =
            FileExt::unlock(&lock).map_err(|error| storage_error(error.to_string()));
        match (result, unlock_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

fn open_lock_file(path: &Path) -> Result<File, SyncClientError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| storage_error(error.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| storage_error(error.to_string()))?;
        }
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|error| storage_error(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| storage_error(error.to_string()))?;
    }
    Ok(file)
}

fn encode_key_record(stored: &StoredAccountKeys) -> Result<Zeroizing<Vec<u8>>, SyncClientError> {
    if stored.keys.is_empty()
        || stored.keys.len() > MAX_STORED_KEYS
        || !stored.keys.contains_key(&stored.current_generation)
    {
        return Err(storage_error("sync account key history is invalid"));
    }
    let count = u16::try_from(stored.keys.len())
        .map_err(|_| storage_error("sync account key history is too large"))?;
    let mut record = Zeroizing::new(Vec::with_capacity(
        KEY_RECORD_HEADER_BYTES + stored.keys.len() * KEY_RECORD_ENTRY_BYTES,
    ));
    record.push(KEY_RECORD_VERSION);
    record.extend_from_slice(&stored.current_generation.to_be_bytes());
    record.extend_from_slice(&count.to_be_bytes());
    for (generation, key) in &stored.keys {
        record.extend_from_slice(&generation.to_be_bytes());
        record.extend_from_slice(key.bytes());
    }
    Ok(record)
}

fn decode_key_record(record: Zeroizing<Vec<u8>>) -> Result<StoredAccountKeys, SyncClientError> {
    if record.len() < KEY_RECORD_HEADER_BYTES || record[0] != KEY_RECORD_VERSION {
        return Err(storage_error("sync account key record is invalid"));
    }
    let current_generation = u64::from_be_bytes(
        record[1..9]
            .try_into()
            .map_err(|_| storage_error("sync account key generation is invalid"))?,
    );
    assert_generation(current_generation)?;
    let count = usize::from(u16::from_be_bytes(
        record[9..11].try_into().map_err(|_| storage_error("sync account key count is invalid"))?,
    ));
    if count == 0
        || count > MAX_STORED_KEYS
        || record.len() != KEY_RECORD_HEADER_BYTES + count * KEY_RECORD_ENTRY_BYTES
    {
        return Err(storage_error("sync account key record size is invalid"));
    }
    let mut keys = BTreeMap::new();
    for entry in record[KEY_RECORD_HEADER_BYTES..].chunks_exact(KEY_RECORD_ENTRY_BYTES) {
        let generation = u64::from_be_bytes(
            entry[..8]
                .try_into()
                .map_err(|_| storage_error("sync account key generation is invalid"))?,
        );
        assert_generation(generation)?;
        let mut bytes = Zeroizing::new([0_u8; 32]);
        bytes.copy_from_slice(&entry[8..]);
        if keys.insert(generation, AccountKey::from_secret(bytes)).is_some() {
            return Err(storage_error("sync account key generation is duplicated"));
        }
    }
    if !keys.contains_key(&current_generation) {
        return Err(storage_error("current sync account key is missing"));
    }
    Ok(StoredAccountKeys { current_generation, keys })
}

fn assert_generation(generation: u64) -> Result<(), SyncClientError> {
    if generation == 0 {
        return Err(storage_error("sync account key generation is invalid"));
    }
    Ok(())
}

fn storage_error(message: impl Into<String>) -> SyncClientError {
    SyncClientError::AccountKeyStorage(message.into())
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_record_round_trips_current_and_historical_keys() -> Result<(), SyncClientError> {
        let current = AccountKey::from_bytes([19; 32]);
        let historical = AccountKey::from_bytes([17; 32]);
        let mut stored = StoredAccountKeys::from_current(&current, 7)?;
        stored.insert_historical(&historical, 3)?;
        let record = encode_key_record(&stored)?;
        let decoded = decode_key_record(Zeroizing::new(record.to_vec()))?;

        assert_eq!(decoded.current_key().map(AccountKey::key_id), Some(current.key_id()));
        assert_eq!(decoded.key(3).map(AccountKey::key_id), Some(historical.key_id()));
        assert_eq!(decoded.current_generation(), 7);
        Ok(())
    }

    #[test]
    fn key_record_rejects_unknown_versions() {
        let record = vec![KEY_RECORD_VERSION + 1; KEY_RECORD_HEADER_BYTES];
        assert!(decode_key_record(Zeroizing::new(record)).is_err());
    }

    #[test]
    fn key_record_rejects_zero_generation() {
        assert!(StoredAccountKeys::from_current(&AccountKey::from_bytes([21; 32]), 0).is_err());
    }

    #[test]
    fn current_generation_cannot_roll_back() -> Result<(), SyncClientError> {
        let key = AccountKey::from_bytes([23; 32]);
        let mut stored = StoredAccountKeys::from_current(&key, 4)?;
        assert!(stored.set_current(&AccountKey::from_bytes([25; 32]), 3).is_err());
        assert!(stored.insert_historical(&AccountKey::from_bytes([27; 32]), 3)?);
        assert_eq!(stored.current_generation(), 4);
        Ok(())
    }
}
