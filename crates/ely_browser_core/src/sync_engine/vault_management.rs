use ely_sync_client::{
    AccountKey, AccountKeyStore, SyncApiClient, SyncClientError, SyncLatestSnapshotDocument,
    SyncVaultBootstrapRequest, SyncVaultDocument, WrappedAccountKey,
};

use super::{ResolvedVault, SyncEngine};

impl SyncEngine {
    pub(super) fn resolve_vault(
        &self,
        client: &SyncApiClient,
        user_id: &str,
    ) -> Result<ResolvedVault, SyncClientError> {
        match client.current_sync_vault() {
            Ok(document) => self.resolve_vault_document(user_id, document),
            Err(SyncClientError::HttpStatus { status: 404, .. }) => {
                self.bootstrap_vault(client, user_id)
            }
            Err(error) => Err(error),
        }
    }

    fn resolve_vault_document(
        &self,
        user_id: &str,
        document: SyncVaultDocument,
    ) -> Result<ResolvedVault, SyncClientError> {
        let store = self.account_key_store(user_id)?;
        let key = document.unwrap_for(user_id, &self.identity)?;
        if let Some(stored) = store.load()? {
            if stored.current_generation() > document.generation {
                return Err(SyncClientError::VaultCrypto {
                    reason: "sync vault generation rolled back",
                });
            }
            if let Some(stored_key) = stored.key(document.generation)
                && stored_key.key_id() != key.key_id()
            {
                return Err(SyncClientError::VaultCrypto {
                    reason: "sync vault key changed within one generation",
                });
            }
        }
        store.save_current(&key, document.generation)?;
        Ok(ResolvedVault { account_key: key, generation: document.generation })
    }

    fn bootstrap_vault(
        &self,
        client: &SyncApiClient,
        user_id: &str,
    ) -> Result<ResolvedVault, SyncClientError> {
        let store = self.account_key_store(user_id)?;
        if store.load()?.is_some() {
            return Err(SyncClientError::VaultCrypto {
                reason: "sync vault bootstrap would discard stored key history",
            });
        }
        let key = AccountKey::generate()?;
        let envelope = WrappedAccountKey::self_wrap(&key, user_id, &self.identity, 1)?;
        let key_id = key.key_id();
        let idempotency_key = format!("vault-bootstrap:{}:{key_id}", self.identity.device_id);
        let request = SyncVaultBootstrapRequest::signed(
            user_id,
            &self.identity,
            &key_id,
            &envelope,
            &idempotency_key,
        )?;
        let document = client.bootstrap_sync_vault(&request)?;
        let confirmed_key = document.unwrap_for(user_id, &self.identity)?;
        if confirmed_key.key_id() != key_id {
            return Err(SyncClientError::AccountKeyUnavailable);
        }
        store.save_current(&confirmed_key, document.generation)?;
        Ok(ResolvedVault { account_key: confirmed_key, generation: document.generation })
    }

    pub(super) fn resolve_historical_key(
        &self,
        client: &SyncApiClient,
        user_id: &str,
        snapshot: &SyncLatestSnapshotDocument,
    ) -> Result<AccountKey, SyncClientError> {
        let store = self.account_key_store(user_id)?;
        if let Some(stored) = store.load()?
            && let Some(key) = stored.key(snapshot.vault_generation)
        {
            if key.key_id() != snapshot.key_id {
                return Err(SyncClientError::AccountKeyUnavailable);
            }
            return Ok(key.clone());
        }
        let document = client.sync_vault_generation(snapshot.vault_generation, &snapshot.key_id)?;
        let key = document.unwrap_for(user_id, &self.identity)?;
        if document.generation != snapshot.vault_generation || key.key_id() != snapshot.key_id {
            return Err(SyncClientError::AccountKeyUnavailable);
        }
        store.save_historical(&key, snapshot.vault_generation)?;
        Ok(key)
    }

    pub(super) fn account_key_store(
        &self,
        user_id: &str,
    ) -> Result<AccountKeyStore, SyncClientError> {
        AccountKeyStore::new(user_id, &self.account_key_lock_dir)
    }
}
