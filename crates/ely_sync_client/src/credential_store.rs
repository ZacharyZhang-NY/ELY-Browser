use keyring::{Entry, Error as KeyringError};
use zeroize::Zeroizing;

pub(crate) fn load_secret(
    service: &str,
    account: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    match entry(service, account)?.get_secret() {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn save_secret(service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
    entry(service, account)?.set_secret(secret).map_err(|error| error.to_string())
}

pub(crate) fn clear_secret(service: &str, account: &str) -> Result<(), String> {
    match entry(service, account)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn entry(service: &str, account: &str) -> Result<Entry, String> {
    Entry::new(service, account).map_err(|error| error.to_string())
}
