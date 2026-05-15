use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ely_domain::BookmarkEntry;
use ely_sync_client::{
    ApiClientConfig, BearerToken, BearerTokenStore, DeviceIdentity, SnapshotPayload,
    SnapshotUploadRequest, SyncApiClient, SyncClientError,
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

    /// Ship a pre-serialised snapshot payload to the worker. Callers
    /// usually pair this with `BrowserCore::build_sync_snapshot_bytes`
    /// — building the bytes on the UI thread and only crossing the
    /// thread boundary with `Vec<u8>` keeps `BrowserCore` itself
    /// single-threaded.
    pub fn upload_bytes(&mut self, bytes: Vec<u8>) -> Result<SyncOutcome, SyncClientError> {
        let Some(bearer) = self.bearer_store.load()? else {
            let outcome = SyncOutcome::SignedOut;
            self.last_outcome = Some(outcome.clone());
            return Ok(outcome);
        };
        let payload = SnapshotPayload::new(bytes)?;
        let logical_clock = current_logical_clock();
        let snapshot_id = snapshot_id_for_user(&self.identity);

        let client = SyncApiClient::new(self.api_config.clone(), bearer)?;
        let request = SnapshotUploadRequest::new(
            &snapshot_id,
            self.api_config.region(),
            SNAPSHOT_SCHEMA_REV,
            logical_clock,
            &payload,
        );
        let document = client.upload_snapshot(&request)?;
        let outcome = SyncOutcome::Uploaded {
            snapshot_id: document.snapshot.snapshot_id,
            logical_clock: document.snapshot.logical_clock,
            payload_bytes: document.snapshot.size_bytes,
            device_id: document.device_id,
        };
        self.last_outcome = Some(outcome.clone());
        Ok(outcome)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncOutcome {
    SignedOut,
    Uploaded { snapshot_id: String, logical_clock: u64, payload_bytes: u64, device_id: String },
}

const SNAPSHOT_SCHEMA_REV: u32 = 1;

fn current_logical_clock() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0)
}

fn snapshot_id_for_user(identity: &DeviceIdentity) -> String {
    // The Cloudflare worker requires `^[a-z0-9][a-z0-9._-]{0,127}$`.
    // The device ID already satisfies the pattern (lowercased prefix
    // + UUIDv7 simple form) and is per-user-stable, so we reuse it as
    // the snapshot id. Future work can extend this to per-object-type
    // snapshots without disturbing the existing one.
    identity.device_id.clone()
}

#[derive(Serialize, Deserialize)]
struct SyncSnapshotBody {
    schema_rev: u32,
    bookmarks: Vec<BookmarkSyncRecord>,
}

impl SyncSnapshotBody {
    fn from_core(core: &BrowserCore) -> Self {
        Self {
            schema_rev: SNAPSHOT_SCHEMA_REV,
            bookmarks: core
                .visible_bookmarks_for_sync()
                .into_iter()
                .map(BookmarkSyncRecord::from)
                .collect(),
        }
    }
}

/// Wire representation of a bookmark. We keep this struct stable so a
/// future deserializer can read snapshots written by earlier app
/// versions; new fields must default-fill on read.
#[derive(Serialize, Deserialize)]
struct BookmarkSyncRecord {
    id: String,
    title: String,
    url: String,
    profile_id: String,
    space_id: String,
    collection_name: String,
    tags: Vec<String>,
    note: Option<String>,
    added_at_secs: u64,
}

impl From<&BookmarkEntry> for BookmarkSyncRecord {
    fn from(entry: &BookmarkEntry) -> Self {
        Self {
            id: entry.id().as_str().to_string(),
            title: entry.title().to_string(),
            url: entry.url().as_str().to_string(),
            profile_id: entry.profile_id().as_str().to_string(),
            space_id: entry.space_id().as_str().to_string(),
            collection_name: entry.collection_name().to_string(),
            tags: entry.tags().to_vec(),
            note: entry.note().map(str::to_string),
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
}
