use keyring_core::{CredentialStore, Entry, Error as KeyringError};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

static NATIVE_STORE: Mutex<Option<Arc<CredentialStore>>> = Mutex::new(None);

pub(crate) fn load_secret(
    service: &str,
    account: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    native_operation(service, account, |entry| match entry.get_secret() {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(error),
    })
}

pub(crate) fn save_secret(service: &str, account: &str, secret: &[u8]) -> Result<(), String> {
    native_operation(service, account, |entry| entry.set_secret(secret))
}

pub(crate) fn clear_secret(service: &str, account: &str) -> Result<(), String> {
    native_operation(service, account, |entry| match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(error),
    })
}

fn native_operation<T>(
    service: &str,
    account: &str,
    operation: impl Fn(&Entry) -> keyring_core::Result<T>,
) -> Result<T, String> {
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    {
        return retry_once(
            || entry(service, account).and_then(|entry| operation(&entry)),
            retryable_store_error,
            clear_cached_store,
        )
        .map_err(|error| error.to_string());
    }
    #[cfg(not(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )))]
    {
        entry(service, account)
            .and_then(|entry| operation(&entry))
            .map_err(|error| error.to_string())
    }
}

#[cfg(any(
    test,
    all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android")))
))]
fn retry_once<T, E>(
    mut attempt: impl FnMut() -> Result<T, E>,
    should_retry: impl Fn(&E) -> bool,
    before_retry: impl FnOnce(),
) -> Result<T, E> {
    let first = attempt();
    if first.as_ref().is_err_and(should_retry) {
        before_retry();
        attempt()
    } else {
        first
    }
}

fn entry(service: &str, account: &str) -> keyring_core::Result<Entry> {
    let store = {
        let mut guard = NATIVE_STORE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_none() {
            *guard = Some(platform_store()?);
        }
        guard.as_ref().cloned().ok_or(KeyringError::NoDefaultStore)?
    };
    build_entry(store.as_ref(), service, account)
}

#[cfg(any(
    test,
    all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android")))
))]
fn retryable_store_error(error: &KeyringError) -> bool {
    matches!(
        error,
        KeyringError::PlatformFailure(_)
            | KeyringError::NoStorageAccess(_)
            | KeyringError::NoDefaultStore
    )
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
fn clear_cached_store() {
    let mut guard = NATIVE_STORE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = None;
}

#[cfg(target_os = "windows")]
fn build_entry(
    store: &CredentialStore,
    service: &str,
    account: &str,
) -> keyring_core::Result<Entry> {
    let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
    store.build(service, account, Some(&modifiers))
}

#[cfg(not(target_os = "windows"))]
fn build_entry(
    store: &CredentialStore,
    service: &str,
    account: &str,
) -> keyring_core::Result<Entry> {
    store.build(service, account, None)
}

#[cfg(target_os = "macos")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    apple_native_keyring_store::keychain::Store::new().map(|store| store as Arc<CredentialStore>)
}

#[cfg(target_os = "windows")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    windows_native_keyring_store::Store::new().map(|store| store as Arc<CredentialStore>)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    zbus_secret_service_keyring_store::Store::new().map(|store| store as Arc<CredentialStore>)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(unix, not(any(target_os = "ios", target_os = "android")))
)))]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Err(KeyringError::NoDefaultStore)
}

#[cfg(test)]
mod tests {
    use super::{KeyringError, retry_once, retryable_store_error};
    use std::cell::Cell;

    #[test]
    fn retry_policy_targets_store_connectivity_errors() {
        assert!(retryable_store_error(&KeyringError::NoDefaultStore));
        assert!(!retryable_store_error(&KeyringError::NoEntry));
    }

    #[test]
    fn retry_evicts_once_and_recreates_the_store_once() {
        let attempts = Cell::new(0);
        let evictions = Cell::new(0);
        let result = retry_once(
            || {
                let attempt = attempts.get() + 1;
                attempts.set(attempt);
                Err::<(), _>(attempt)
            },
            |_| true,
            || evictions.set(evictions.get() + 1),
        );

        assert_eq!(result, Err(2));
        assert_eq!(attempts.get(), 2);
        assert_eq!(evictions.get(), 1);
    }
}
