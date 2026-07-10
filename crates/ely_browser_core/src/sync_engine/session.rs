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
    use std::{
        cell::Cell,
        io::{Read, Write},
        net::TcpListener,
        thread::{self, JoinHandle},
        time::Duration,
    };

    use super::{SyncEngine, reconcile_authenticated_result};
    use crate::sync_engine::browser_data_root;
    use ely_domain::ProfileId;
    use ely_sync_client::{
        ApiClientConfig, BearerToken, BearerTokenStore, DeviceIdentity, SyncApiClient,
        SyncClientError, SyncOwnerStore,
    };

    type IdentityServer = JoinHandle<std::io::Result<String>>;

    #[test]
    fn owner_mismatch_stops_before_device_registration() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_id = ProfileId::new();
        let profile_data_dir = directory.path().join(profile_id.as_str()).join("servo");
        std::fs::create_dir_all(&profile_data_dir)?;
        let owner_store = SyncOwnerStore::new(directory.path());
        owner_store.claim("user-A")?;
        let (base_url, server) = spawn_identity_server("user-B")?;
        let engine = SyncEngine {
            api_config: ApiClientConfig::custom(base_url.clone(), "auto"),
            bearer_store: BearerTokenStore::new(&profile_id, &profile_data_dir),
            account_key_lock_dir: directory.path().join(".sync-key-locks"),
            owner_store,
            identity: DeviceIdentity {
                device_id: "device-01".to_string(),
                public_key: "a".repeat(64),
                wrapping_public_key: "b".repeat(64),
                device_name: "Test".to_string(),
                platform: "test".to_string(),
            },
            last_outcome: None,
        };
        let client = SyncApiClient::new(
            ApiClientConfig::custom(base_url, "auto"),
            BearerToken::new("a".repeat(64))?,
        )?;

        let result = engine.registered_client(client);
        assert!(matches!(result, Err(SyncClientError::SyncOwnerMismatch)));
        let request = join_identity_server(server)?;
        assert!(request.starts_with("GET /api/devices HTTP/1.1\r\n"));
        Ok(())
    }

    #[test]
    fn profile_data_path_must_resolve_to_the_global_root() {
        let profile_id = ProfileId::new();
        let valid = std::path::PathBuf::from("/profiles").join(profile_id.as_str()).join("servo");

        assert!(matches!(
            browser_data_root(&profile_id, &valid),
            Ok(root) if root == std::path::Path::new("/profiles")
        ));
        assert!(browser_data_root(&profile_id, std::path::Path::new("/servo")).is_err());
        assert!(
            browser_data_root(
                &profile_id,
                &std::path::PathBuf::from("/profiles")
                    .join(ProfileId::new().as_str())
                    .join("servo")
            )
            .is_err()
        );
        assert!(
            browser_data_root(
                &profile_id,
                &std::path::PathBuf::from("/profiles").join(profile_id.as_str()).join("other")
            )
            .is_err()
        );
    }

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

    fn spawn_identity_server(user_id: &str) -> Result<(String, IdentityServer), std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let body = format!(r#"{{"version":1,"user_id":"{user_id}","devices":[]}}"#);
        let server = thread::spawn(move || {
            let (mut stream, _) = (0..100)
                .find_map(|_| match listener.accept() {
                    Ok(connection) => Some(Ok(connection)),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        None
                    }
                    Err(error) => Some(Err(error)),
                })
                .ok_or_else(|| std::io::Error::other("identity request was not received"))??;
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk)?;
                if read == 0 || request.len() + read > 8192 {
                    return Err(std::io::Error::other("identity request headers are incomplete"));
                }
                request.extend_from_slice(&chunk[..read]);
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes())?;
            stream.flush()?;
            String::from_utf8(request)
                .map_err(|_| std::io::Error::other("identity request encoding is invalid"))
        });
        Ok((format!("http://{address}"), server))
    }

    fn join_identity_server(server: IdentityServer) -> Result<String, Box<dyn std::error::Error>> {
        match server.join() {
            Ok(result) => result.map_err(Into::into),
            Err(_) => Err("identity server thread panicked".into()),
        }
    }
}
