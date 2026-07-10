use ely_sync_client::{SyncApiClient, SyncClientError};

use super::{SyncEngine, SyncOutcome, validate_sync_status};

impl SyncEngine {
    pub fn sync_bytes(&mut self, bytes: Vec<u8>) -> Result<SyncOutcome, SyncClientError> {
        let outcome = self
            .with_authenticated_client(|client| {
                let Some((client, user_id)) = self.approved_client(client)? else {
                    return Ok(SyncOutcome::AwaitingDeviceApproval {
                        device_id: self.identity.device_id.clone(),
                    });
                };
                let vault = self.resolve_vault(&client, &user_id)?;
                let status = client.sync_status()?;
                validate_sync_status(&status, &user_id, &self.identity.device_id)?;
                match status.snapshots.head {
                    Some(head) => {
                        let remote =
                            self.download_remote_snapshot(&client, &user_id, &vault, head)?;
                        if remote.bytes == bytes
                            && remote.merge_base.vault_generation() == vault.generation
                        {
                            Ok(SyncOutcome::AlreadyCurrent {
                                snapshot_id: remote.merge_base.snapshot_id().to_string(),
                                logical_clock: remote.merge_base.logical_clock(),
                                payload_bytes: remote.merge_base.size_bytes(),
                                device_id: remote.merge_base.device_id().to_string(),
                            })
                        } else if remote.bytes == bytes {
                            self.upload_payload(
                                &client,
                                &user_id,
                                &vault,
                                bytes,
                                Some(&remote.merge_base),
                            )
                        } else {
                            Ok(remote.into_outcome(false))
                        }
                    }
                    None => self.upload_payload(&client, &user_id, &vault, bytes, None),
                }
            })?
            .unwrap_or(SyncOutcome::SignedOut);
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }

    pub fn upload_merged_bytes(
        &mut self,
        bytes: Vec<u8>,
        merge_base: ely_sync_client::AuthenticatedSnapshotHead,
    ) -> Result<SyncOutcome, SyncClientError> {
        let outcome = self
            .with_authenticated_client(|client| {
                let Some((client, user_id)) = self.approved_client(client)? else {
                    return Ok(SyncOutcome::AwaitingDeviceApproval {
                        device_id: self.identity.device_id.clone(),
                    });
                };
                let vault = self.resolve_vault(&client, &user_id)?;
                self.upload_payload(&client, &user_id, &vault, bytes, Some(&merge_base))
            })?
            .unwrap_or(SyncOutcome::SignedOut);
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }

    pub(super) fn with_required_authenticated_client<T>(
        &self,
        operation: impl FnOnce(SyncApiClient) -> Result<T, SyncClientError>,
    ) -> Result<T, SyncClientError> {
        self.with_authenticated_client(operation)?.ok_or(SyncClientError::SessionEnded)
    }

    fn with_authenticated_client<T>(
        &self,
        operation: impl FnOnce(SyncApiClient) -> Result<T, SyncClientError>,
    ) -> Result<Option<T>, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            return Ok(None);
        };
        let client = SyncApiClient::new(self.api_config.clone(), bearer.clone())?;
        reconcile_authenticated_result(operation(client), || {
            self.bearer_store.clear_if_matches(&bearer)
        })
    }
}

fn reconcile_authenticated_result<T>(
    result: Result<T, SyncClientError>,
    clear_if_matches: impl FnOnce() -> Result<bool, SyncClientError>,
) -> Result<Option<T>, SyncClientError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(SyncClientError::SessionEnded) => {
            if clear_if_matches()? {
                Ok(None)
            } else {
                Err(SyncClientError::SessionChanged)
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::reconcile_authenticated_result;
    use ely_sync_client::SyncClientError;

    #[test]
    fn terminal_session_clears_the_captured_credential() -> Result<(), SyncClientError> {
        let cleared = Cell::new(false);
        let result =
            reconcile_authenticated_result::<()>(Err(SyncClientError::SessionEnded), || {
                cleared.set(true);
                Ok(true)
            })?;

        assert!(result.is_none());
        assert!(cleared.get());
        Ok(())
    }

    #[test]
    fn replacement_credential_stays_signed_in() {
        let result =
            reconcile_authenticated_result::<()>(Err(SyncClientError::SessionEnded), || Ok(false));

        assert!(matches!(result, Err(SyncClientError::SessionChanged)));
    }

    #[test]
    fn credential_clear_failures_are_preserved() {
        let result =
            reconcile_authenticated_result::<()>(Err(SyncClientError::SessionEnded), || {
                Err(SyncClientError::BearerCredentialStorage("locked".to_string()))
            });

        assert!(matches!(result, Err(SyncClientError::BearerCredentialStorage(_))));
    }

    #[test]
    fn ordinary_failures_preserve_the_credential() {
        let clear_called = Cell::new(false);
        let result =
            reconcile_authenticated_result::<()>(Err(SyncClientError::SnapshotBusy), || {
                clear_called.set(true);
                Ok(true)
            });

        assert!(matches!(result, Err(SyncClientError::SnapshotBusy)));
        assert!(!clear_called.get());
    }
}
