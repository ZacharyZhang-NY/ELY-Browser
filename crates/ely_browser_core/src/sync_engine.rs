use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ely_sync_client::{
    AccountKey, ApiClientConfig, AuthenticatedSnapshotHead, BearerToken, BearerTokenStore,
    DeviceIdentity, SNAPSHOT_ENCRYPTION_VERSION, SnapshotCryptoContext, SnapshotDownloadResult,
    SnapshotPayload, SnapshotUploadRequest, SnapshotUploadResult, SyncApiClient, SyncClientError,
    SyncLatestSnapshotDocument,
};

use crate::state::BrowserCore;
use crate::sync_records::{SNAPSHOT_SCHEMA_REV, SyncSnapshotBody};

mod concurrency;
mod device_management;
mod vault_management;

use concurrency::{conflict_head, ensure_remote_generation_is_available};

/// Per-profile sync engine for device identity, bearer-token storage, and snapshot IO.
#[derive(Debug)]
pub struct SyncEngine {
    api_config: ApiClientConfig,
    bearer_store: BearerTokenStore,
    account_key_lock_dir: PathBuf,
    identity: DeviceIdentity,
    last_outcome: Option<SyncOutcome>,
}

impl SyncEngine {
    /// Bring up the engine for the given profile data root. Generates a
    /// fresh device identity on first use, then keeps it stable across
    /// runs so the server's `user_devices` row stays bound.
    pub fn for_profile_dir(
        profile_data_dir: &Path,
        device_name: impl Into<String>,
        platform: impl Into<String>,
    ) -> Result<Self, SyncClientError> {
        let sync_dir = profile_data_dir.join("sync");
        let account_key_lock_dir = profile_data_dir
            .parent()
            .and_then(Path::parent)
            .unwrap_or(profile_data_dir)
            .join(".sync-key-locks");
        let identity =
            DeviceIdentity::load_or_create(&sync_dir.join("device.json"), device_name, platform)?;
        let bearer_store = BearerTokenStore::new(sync_dir.join("bearer.token"));
        Ok(Self {
            api_config: ApiClientConfig::production(),
            bearer_store,
            account_key_lock_dir,
            identity,
            last_outcome: None,
        })
    }

    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    pub fn bearer_path(&self) -> &Path {
        self.bearer_store.path()
    }

    pub fn last_outcome(&self) -> Option<&SyncOutcome> {
        self.last_outcome.as_ref()
    }

    /// Replace the cached bearer token. Trimming + shape validation is
    /// enforced by `BearerToken::new`. Returns `Ok(())` even when the
    /// caller passes a blank string (treated as a sign-out).
    pub fn install_bearer(&mut self, raw: &str) -> Result<bool, SyncClientError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            self.bearer_store.clear()?;
            return Ok(false);
        }
        let token = BearerToken::new(trimmed)?;
        self.bearer_store.save(&token)?;
        Ok(true)
    }

    pub fn is_signed_in(&self) -> Result<bool, SyncClientError> {
        self.bearer_store.load().map(|token| token.is_some())
    }

    /// Reconcile a pre-serialised local payload with the authenticated global snapshot head.
    pub fn sync_bytes(&mut self, bytes: Vec<u8>) -> Result<SyncOutcome, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            let outcome = SyncOutcome::SignedOut;
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let client = SyncApiClient::new(self.api_config.clone(), bearer)?;
        let Some((client, user_id)) = self.approved_client(client)? else {
            let outcome =
                SyncOutcome::AwaitingDeviceApproval { device_id: self.identity.device_id.clone() };
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };

        let vault = self.resolve_vault(&client, &user_id)?;
        let status = client.sync_status()?;
        validate_sync_status(&status, &user_id, &self.identity.device_id)?;
        let outcome = match status.snapshots.head {
            Some(head) => {
                let remote = self.download_remote_snapshot(&client, &user_id, &vault, head)?;
                if remote.bytes == bytes && remote.merge_base.vault_generation() == vault.generation
                {
                    SyncOutcome::AlreadyCurrent {
                        snapshot_id: remote.merge_base.snapshot_id().to_string(),
                        logical_clock: remote.merge_base.logical_clock(),
                        payload_bytes: remote.merge_base.size_bytes(),
                        device_id: remote.merge_base.device_id().to_string(),
                    }
                } else if remote.bytes == bytes {
                    self.upload_payload(&client, &user_id, &vault, bytes, Some(&remote.merge_base))?
                } else {
                    remote.into_outcome(false)
                }
            }
            None => self.upload_payload(&client, &user_id, &vault, bytes, None)?,
        };
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }

    /// Upload a local payload after the UI thread has applied a remote
    /// snapshot. The caller passes the remote logical clock so the new
    /// merged snapshot is ordered after the downloaded one.
    pub fn upload_merged_bytes(
        &mut self,
        bytes: Vec<u8>,
        merge_base: AuthenticatedSnapshotHead,
    ) -> Result<SyncOutcome, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            let outcome = SyncOutcome::SignedOut;
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let client = SyncApiClient::new(self.api_config.clone(), bearer)?;
        let Some((client, user_id)) = self.approved_client(client)? else {
            let outcome =
                SyncOutcome::AwaitingDeviceApproval { device_id: self.identity.device_id.clone() };
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let vault = self.resolve_vault(&client, &user_id)?;
        let outcome = self.upload_payload(&client, &user_id, &vault, bytes, Some(&merge_base))?;
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }

    fn approved_client(
        &self,
        client: SyncApiClient,
    ) -> Result<Option<(SyncApiClient, String)>, SyncClientError> {
        let (client, registration) = self.registered_client(client)?;
        if registration.device.is_approved() {
            return Ok(Some((client, registration.user_id)));
        }
        if registration.device.approval_status == "pending" {
            return Ok(None);
        }
        Err(SyncClientError::DeviceApprovalStatus {
            device_id: registration.device.device_id,
            status: registration.device.approval_status,
        })
    }

    fn registered_client(
        &self,
        client: SyncApiClient,
    ) -> Result<(SyncApiClient, ely_sync_client::client::DeviceRecordDocument), SyncClientError>
    {
        let idempotency_key = device_registration_idempotency_key(&self.identity);
        let registration = match client.register_device(&self.identity, &idempotency_key) {
            Ok(registration) => registration,
            Err(SyncClientError::HttpStatus { status: 409, .. }) => {
                let rebound = client.rebind_device(&self.identity)?;
                let registration = client.register_device(&self.identity, &idempotency_key)?;
                if registration.user_id != rebound.user_id {
                    return Err(SyncClientError::DeviceTrust {
                        reason: "device rebind account does not match registration",
                    });
                }
                registration
            }
            Err(error) => return Err(error),
        };
        Ok((client, registration))
    }

    fn upload_payload(
        &self,
        client: &SyncApiClient,
        user_id: &str,
        vault: &ResolvedVault,
        bytes: Vec<u8>,
        base: Option<&AuthenticatedSnapshotHead>,
    ) -> Result<SyncOutcome, SyncClientError> {
        let logical_clock_floor = base.map_or(0, AuthenticatedSnapshotHead::logical_clock);
        let logical_clock = current_logical_clock().max(logical_clock_floor.saturating_add(1));
        let snapshot_id = snapshot_id_for_user(&self.identity);
        let head_revision = match base {
            Some(base) => base.next_revision()?,
            None => 1,
        };
        let context = SnapshotCryptoContext {
            user_id,
            vault_generation: vault.generation,
            snapshot_id: &snapshot_id,
            schema_rev: SNAPSHOT_SCHEMA_REV,
            logical_clock,
            device_id: &self.identity.device_id,
            head_revision,
            base_head: base.map(AuthenticatedSnapshotHead::head_ref),
        };
        let encrypted = vault.account_key.encrypt(&context, &bytes)?;
        let payload = SnapshotPayload::new(encrypted.bytes().to_vec())?;
        let request = SnapshotUploadRequest::new(
            self.api_config.region(),
            &context,
            base,
            &encrypted,
            &payload,
        )?;
        match client.upload_snapshot(&request)? {
            SnapshotUploadResult::Committed(document) => {
                if document.version != 3
                    || document.user_id != user_id
                    || document.device_id != self.identity.device_id
                    || document.snapshot.snapshot_id != snapshot_id
                    || document.snapshot.head_revision != head_revision
                    || document.snapshot.base_head.as_ref()
                        != base.map(AuthenticatedSnapshotHead::head_ref)
                    || document.snapshot.payload_hash != payload.payload_hash()
                    || document.snapshot.encryption_version != SNAPSHOT_ENCRYPTION_VERSION
                    || document.snapshot.key_id != vault.account_key.key_id()
                    || document.snapshot.vault_generation != vault.generation
                    || document.snapshot.content_hash != encrypted.content_hash()
                    || document.snapshot.schema_rev != SNAPSHOT_SCHEMA_REV
                    || document.snapshot.logical_clock != logical_clock
                    || document.snapshot.size_bytes
                        != u64::try_from(payload.bytes().len()).map_err(|_| {
                            SyncClientError::SnapshotEncryption {
                                reason: "snapshot payload size is invalid",
                            }
                        })?
                {
                    return Err(SyncClientError::SnapshotEncryption {
                        reason: "snapshot upload response does not match request",
                    });
                }
                Ok(SyncOutcome::Uploaded {
                    snapshot_id: document.snapshot.snapshot_id,
                    logical_clock: document.snapshot.logical_clock,
                    payload_bytes: document.snapshot.size_bytes,
                    device_id: document.device_id,
                })
            }
            SnapshotUploadResult::Conflict(conflict) => {
                let head = conflict_head(conflict)?;
                self.download_remote_snapshot(client, user_id, vault, head)
                    .map(|remote| remote.into_outcome(true))
            }
        }
    }

    fn download_remote_snapshot(
        &self,
        client: &SyncApiClient,
        user_id: &str,
        vault: &ResolvedVault,
        mut latest: SyncLatestSnapshotDocument,
    ) -> Result<AuthenticatedRemote, SyncClientError> {
        for _ in 0..3 {
            let account_key = self.key_for_snapshot(client, user_id, vault, &latest)?;
            let expected_head = latest.head_ref()?;
            match client.download_snapshot(&expected_head)? {
                SnapshotDownloadResult::Downloaded(download) => {
                    let (bytes, merge_base) =
                        download.authenticate(&expected_head, &account_key)?.into_parts();
                    return Ok(AuthenticatedRemote { bytes, merge_base });
                }
                SnapshotDownloadResult::Conflict(conflict) => {
                    latest = conflict_head(conflict)?;
                }
            }
        }
        Err(SyncClientError::SnapshotBusy)
    }

    fn key_for_snapshot(
        &self,
        client: &SyncApiClient,
        user_id: &str,
        vault: &ResolvedVault,
        snapshot: &SyncLatestSnapshotDocument,
    ) -> Result<AccountKey, SyncClientError> {
        if !matches!(snapshot.encryption_version, 1 | SNAPSHOT_ENCRYPTION_VERSION) {
            return Err(SyncClientError::SnapshotEncryption {
                reason: "remote snapshot encryption version is unsupported",
            });
        }
        ensure_remote_generation_is_available(snapshot.vault_generation, vault.generation)?;
        if snapshot.vault_generation == vault.generation {
            if snapshot.key_id != vault.account_key.key_id() {
                return Err(SyncClientError::AccountKeyUnavailable);
            }
            return Ok(vault.account_key.clone());
        }
        self.resolve_historical_key(client, user_id, snapshot)
    }
}

struct ResolvedVault {
    account_key: AccountKey,
    generation: u64,
}

struct AuthenticatedRemote {
    bytes: Vec<u8>,
    merge_base: AuthenticatedSnapshotHead,
}

impl AuthenticatedRemote {
    fn into_outcome(self, cas_conflict: bool) -> SyncOutcome {
        SyncOutcome::RemoteSnapshot {
            snapshot_id: self.merge_base.snapshot_id().to_string(),
            logical_clock: self.merge_base.logical_clock(),
            payload_bytes: self.merge_base.size_bytes(),
            device_id: self.merge_base.device_id().to_string(),
            bytes: self.bytes,
            merge_base: self.merge_base,
            cas_conflict,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncOutcome {
    SignedOut,
    AwaitingDeviceApproval {
        device_id: String,
    },
    AlreadyCurrent {
        snapshot_id: String,
        logical_clock: u64,
        payload_bytes: u64,
        device_id: String,
    },
    RemoteSnapshot {
        snapshot_id: String,
        logical_clock: u64,
        payload_bytes: u64,
        device_id: String,
        bytes: Vec<u8>,
        merge_base: AuthenticatedSnapshotHead,
        cas_conflict: bool,
    },
    Uploaded {
        snapshot_id: String,
        logical_clock: u64,
        payload_bytes: u64,
        device_id: String,
    },
}

fn validate_sync_status(
    status: &ely_sync_client::SyncStatusDocument,
    user_id: &str,
    device_id: &str,
) -> Result<(), SyncClientError> {
    if status.version != 2
        || status.user_id != user_id
        || status.device_id != device_id
        || status.devices.current_device_id != device_id
        || !status.devices.current_device_approved
        || (status.snapshots.total_snapshots == 0) != status.snapshots.head.is_none()
    {
        return Err(SyncClientError::DeviceTrust { reason: "sync status identity is invalid" });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyncSnapshotApplySummary {
    imported: usize,
    updated: usize,
    skipped: usize,
}

impl SyncSnapshotApplySummary {
    #[must_use]
    pub fn imported(self) -> usize {
        self.imported
    }

    #[must_use]
    pub fn updated(self) -> usize {
        self.updated
    }

    #[must_use]
    pub fn skipped(self) -> usize {
        self.skipped
    }

    pub(crate) fn record_imported(&mut self) {
        self.imported += 1;
    }

    pub(crate) fn record_updated(&mut self) {
        self.updated += 1;
    }

    pub(crate) fn record_skipped(&mut self) {
        self.skipped += 1;
    }
}

fn current_logical_clock() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}

fn snapshot_id_for_user(identity: &DeviceIdentity) -> String {
    identity.device_id.clone()
}

fn device_registration_idempotency_key(identity: &DeviceIdentity) -> String {
    format!("device-register:{}", identity.device_id)
}

#[derive(Debug)]
pub struct SyncEngineBuilder {
    pub profile_data_dir: PathBuf,
    pub device_name: String,
    pub platform: String,
}

impl SyncEngineBuilder {
    pub fn build(self) -> Result<SyncEngine, SyncClientError> {
        SyncEngine::for_profile_dir(&self.profile_data_dir, self.device_name, self.platform)
    }
}

impl BrowserCore {
    /// Build the JSON byte payload the sync engine expects. Lives on
    /// `BrowserCore` so the snapshot reads the live state and so the
    /// UI thread does the (synchronous, cheap) serialization before
    /// handing bytes off to the worker thread.
    pub fn build_sync_snapshot_bytes(&self) -> Result<Vec<u8>, SyncClientError> {
        self.ensure_active_profile_allows_sync()?;
        let body = SyncSnapshotBody::from_core(self);
        serde_json::to_vec(&body).map_err(|error| SyncClientError::Json {
            endpoint: "snapshot".to_string(),
            source: error,
        })
    }

    pub fn apply_sync_snapshot_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<SyncSnapshotApplySummary, SyncClientError> {
        self.ensure_active_profile_allows_sync()?;
        let body: SyncSnapshotBody = serde_json::from_slice(bytes).map_err(|error| {
            SyncClientError::Json { endpoint: "snapshot".to_string(), source: error }
        })?;
        if body.schema_rev != SNAPSHOT_SCHEMA_REV {
            return Err(SyncClientError::SnapshotSchema(format!(
                "unsupported schema_rev {}",
                body.schema_rev
            )));
        }
        self.apply_sync_snapshot_body(body)
    }

    fn ensure_active_profile_allows_sync(&self) -> Result<(), SyncClientError> {
        if self.active_profile_allows_sync() {
            return Ok(());
        }
        Err(SyncClientError::SyncPolicy { reason: "active profile is private".to_string() })
    }
}
