use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ely_domain::BookmarkEntry;
use ely_sync_client::{
    ApiClientConfig, BearerToken, BearerTokenStore, DeviceIdentity, SnapshotPayload,
    SnapshotUploadRequest, SyncApiClient, SyncClientError, SyncLatestSnapshotDocument,
};
use serde::{Deserialize, Serialize};

use crate::state::BrowserCore;

/// Per-profile sync scaffolding. Holds the persisted device identity
/// and the bearer-token store; the API client is only constructed at
/// the moment the user runs a manual sync (so a logged-out user
/// doesn't pay TLS handshake cost on startup).
#[derive(Debug)]
pub struct SyncEngine {
    api_config: ApiClientConfig,
    bearer_store: BearerTokenStore,
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
        let identity =
            DeviceIdentity::load_or_create(&sync_dir.join("device.json"), device_name, platform)?;
        let bearer_store = BearerTokenStore::new(sync_dir.join("bearer.token"));
        Ok(Self {
            api_config: ApiClientConfig::production(),
            bearer_store,
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

    /// Run the snapshot sync plan for a pre-serialised local payload.
    /// The engine registers the device, checks the worker's latest
    /// snapshot, downloads a newer remote payload when another device
    /// wrote one, and uploads when the local payload is ready to win.
    pub fn sync_bytes(&mut self, bytes: Vec<u8>) -> Result<SyncOutcome, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            let outcome = SyncOutcome::SignedOut;
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let payload = SnapshotPayload::new(bytes)?;
        let client = SyncApiClient::new(self.api_config.clone(), bearer)?;
        let Some(client) = self.approved_client(client)? else {
            let outcome =
                SyncOutcome::AwaitingDeviceApproval { device_id: self.identity.device_id.clone() };
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };

        let status = client.sync_status()?;
        let outcome = match status.snapshots.latest {
            Some(latest) if latest.payload_hash == payload.payload_hash() => {
                SyncOutcome::AlreadyCurrent {
                    snapshot_id: latest.snapshot_id,
                    logical_clock: latest.logical_clock,
                    payload_bytes: latest.size_bytes,
                    device_id: latest.device_id,
                }
            }
            Some(latest) if latest.device_id != self.identity.device_id => {
                self.download_remote_snapshot(&client, latest)?
            }
            Some(latest) => self.upload_payload(&client, payload, latest.logical_clock)?,
            None => self.upload_payload(&client, payload, 0)?,
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
        logical_clock_floor: u64,
    ) -> Result<SyncOutcome, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            let outcome = SyncOutcome::SignedOut;
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let payload = SnapshotPayload::new(bytes)?;
        let client = SyncApiClient::new(self.api_config.clone(), bearer)?;
        let Some(client) = self.approved_client(client)? else {
            let outcome =
                SyncOutcome::AwaitingDeviceApproval { device_id: self.identity.device_id.clone() };
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let outcome = self.upload_payload(&client, payload, logical_clock_floor)?;
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }

    fn approved_client(
        &self,
        client: SyncApiClient,
    ) -> Result<Option<SyncApiClient>, SyncClientError> {
        let registration = client.register_device(
            &self.identity,
            &device_registration_idempotency_key(&self.identity),
        )?;
        if registration.device.is_approved() {
            return Ok(Some(client));
        }
        if registration.device.approval_status == "pending" {
            return Ok(None);
        }
        Err(SyncClientError::DeviceApprovalStatus {
            device_id: registration.device.device_id,
            status: registration.device.approval_status,
        })
    }

    fn upload_payload(
        &self,
        client: &SyncApiClient,
        payload: SnapshotPayload,
        logical_clock_floor: u64,
    ) -> Result<SyncOutcome, SyncClientError> {
        let logical_clock = current_logical_clock().max(logical_clock_floor.saturating_add(1));
        let snapshot_id = snapshot_id_for_user(&self.identity);
        let request = SnapshotUploadRequest::new(
            &snapshot_id,
            self.api_config.region(),
            SNAPSHOT_SCHEMA_REV,
            logical_clock,
            &payload,
        );
        let document = client.upload_snapshot(&request)?;
        Ok(SyncOutcome::Uploaded {
            snapshot_id: document.snapshot.snapshot_id,
            logical_clock: document.snapshot.logical_clock,
            payload_bytes: document.snapshot.size_bytes,
            device_id: document.device_id,
        })
    }

    fn download_remote_snapshot(
        &self,
        client: &SyncApiClient,
        latest: SyncLatestSnapshotDocument,
    ) -> Result<SyncOutcome, SyncClientError> {
        let download = client.download_snapshot(&latest.snapshot_id)?;
        let payload = download.payload()?;
        Ok(SyncOutcome::RemoteSnapshot {
            snapshot_id: latest.snapshot_id,
            logical_clock: latest.logical_clock,
            payload_bytes: latest.size_bytes,
            device_id: latest.device_id,
            bytes: payload.into_bytes(),
        })
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
    },
    Uploaded {
        snapshot_id: String,
        logical_clock: u64,
        payload_bytes: u64,
        device_id: String,
    },
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

const SNAPSHOT_SCHEMA_REV: u32 = 1;

fn current_logical_clock() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}

fn snapshot_id_for_user(identity: &DeviceIdentity) -> String {
    identity.device_id.clone()
}

fn device_registration_idempotency_key(identity: &DeviceIdentity) -> String {
    format!("device-register:{}", identity.device_id)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SyncSnapshotBody {
    pub(crate) schema_rev: u32,
    pub(crate) bookmarks: Vec<BookmarkSyncRecord>,
}

impl SyncSnapshotBody {
    fn from_core(core: &BrowserCore) -> Self {
        Self {
            schema_rev: SNAPSHOT_SCHEMA_REV,
            bookmarks: core
                .visible_bookmarks_for_sync()
                .into_iter()
                .map(|entry| {
                    BookmarkSyncRecord::from_entry(
                        entry,
                        core.sync_space_name_for(entry.space_id()),
                    )
                })
                .collect(),
        }
    }
}

/// Wire representation of a bookmark. We keep this struct stable so a
/// future deserializer can read snapshots written by earlier app
/// versions; new fields must default-fill on read.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct BookmarkSyncRecord {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) profile_id: String,
    pub(crate) space_id: String,
    #[serde(default)]
    pub(crate) space_name: Option<String>,
    pub(crate) collection_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) note: Option<String>,
    #[serde(default)]
    pub(crate) thumbnail_key: Option<String>,
    pub(crate) added_at_secs: u64,
}

impl BookmarkSyncRecord {
    fn from_entry(entry: &BookmarkEntry, space_name: Option<String>) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            title: entry.title().to_string(),
            url: entry.url().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            space_name,
            collection_name: entry.collection_name().to_string(),
            tags: entry.tags().to_vec(),
            note: entry.note().map(str::to_string),
            thumbnail_key: entry.thumbnail_key().map(str::to_string),
            added_at_secs: entry
                .added_at()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0),
        }
    }
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
}
