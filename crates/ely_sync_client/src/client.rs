use std::time::Duration;

use serde::de::DeserializeOwned;
use ureq::{Agent, AgentBuilder};

use crate::{
    auth::BearerToken,
    device::{DeviceIdentity, DeviceListResponse, DeviceRegistration},
    error::SyncClientError,
    snapshot::{SnapshotDownload, SnapshotUploadRequest},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const USER_AGENT: &str = concat!("ELY Browser/", env!("CARGO_PKG_VERSION"));

/// Resolved configuration for talking to a worker deployment.
#[derive(Clone, Debug)]
pub struct ApiClientConfig {
    base_url: String,
    region: String,
}

impl ApiClientConfig {
    /// Build the config used by the production deployment. The base URL
    /// matches `wrangler.toml`'s `ELY_AUTH_BASE_URL`.
    pub fn production() -> Self {
        Self {
            base_url: "https://ely-browser-cloud.zhangyanghaha0407.workers.dev".to_string(),
            region: "auto".to_string(),
        }
    }

    pub fn custom(base_url: impl Into<String>, region: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), region: region.into() }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn region(&self) -> &str {
        &self.region
    }
}

/// Bearer-token authenticated HTTP client targeting `ely-browser-cloud`.
pub struct SyncApiClient {
    agent: Agent,
    config: ApiClientConfig,
    bearer: BearerToken,
}

impl SyncApiClient {
    pub fn new(config: ApiClientConfig, bearer: BearerToken) -> Result<Self, SyncClientError> {
        if !config.base_url.starts_with("https://") && !config.base_url.starts_with("http://") {
            return Err(SyncClientError::InvalidBaseUrl { url: config.base_url.clone() });
        }
        let agent = AgentBuilder::new().timeout(REQUEST_TIMEOUT).user_agent(USER_AGENT).build();
        Ok(Self { agent, config, bearer })
    }

    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }

    /// `POST /api/devices/register` — bind a freshly-generated device
    /// identity to the current session. The response carries the
    /// canonical device record from the worker's `user_devices` table,
    /// including the `approval_status` that gates `/api/sync/*`.
    pub fn register_device(
        &self,
        identity: &DeviceIdentity,
        idempotency_key: &str,
    ) -> Result<DeviceRecordDocument, SyncClientError> {
        let registration = DeviceRegistration {
            version: 1,
            device_id: &identity.device_id,
            public_key: &identity.public_key,
            device_name: &identity.device_name,
            platform: &identity.platform,
            idempotency_key,
        };
        let endpoint = self.endpoint("/api/devices/register");
        let response = self
            .agent
            .post(&endpoint)
            .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
            .set("Content-Type", "application/json")
            .send_json(serde_json::to_value(&registration).map_err(|error| {
                SyncClientError::Json { endpoint: endpoint.clone(), source: error }
            })?);
        let body = read_json_response::<DeviceRecordDocument>(&endpoint, response)?;
        Ok(body)
    }

    /// `GET /api/devices` — return every device bound to the user,
    /// approved or otherwise. Used by the UI to render the pending
    /// device-approval list.
    pub fn list_devices(&self) -> Result<DeviceListResponse, SyncClientError> {
        let endpoint = self.endpoint("/api/devices");
        let response = self
            .agent
            .get(&endpoint)
            .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
            .call();
        read_json_response::<DeviceListResponse>(&endpoint, response)
    }

    /// `GET /api/sync/status` — return the worker-side cursor,
    /// object, snapshot, and device summary for the authenticated
    /// approved device.
    pub fn sync_status(&self) -> Result<SyncStatusDocument, SyncClientError> {
        let endpoint = self.endpoint("/api/sync/status");
        let response = self
            .agent
            .get(&endpoint)
            .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
            .call();
        read_json_response::<SyncStatusDocument>(&endpoint, response)
    }

    /// `POST /api/sync/snapshot` — push the full per-user state. The
    /// worker enforces logical-clock monotonicity, so callers must
    /// pass a value strictly greater than the last accepted snapshot.
    pub fn upload_snapshot(
        &self,
        request: &SnapshotUploadRequest<'_>,
    ) -> Result<SnapshotUploadDocument, SyncClientError> {
        let endpoint = self.endpoint("/api/sync/snapshot");
        let response =
            self.agent
                .post(&endpoint)
                .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
                .set("Content-Type", "application/json")
                .send_json(serde_json::to_value(request).map_err(|error| {
                    SyncClientError::Json { endpoint: endpoint.clone(), source: error }
                })?);
        read_json_response::<SnapshotUploadDocument>(&endpoint, response)
    }

    /// `GET /api/sync/snapshot?snapshot_id=…` — fetch the snapshot for
    /// the named id. Returns the encoded payload plus the snapshot
    /// metadata; callers should verify the payload hash before trusting
    /// the bytes.
    pub fn download_snapshot(
        &self,
        snapshot_id: &str,
    ) -> Result<SnapshotDownload, SyncClientError> {
        let endpoint = self.endpoint(&format!("/api/sync/snapshot?snapshot_id={snapshot_id}"));
        let response = self
            .agent
            .get(&endpoint)
            .set("Authorization", &format!("Bearer {}", self.bearer.as_str()))
            .call();
        read_json_response::<SnapshotDownload>(&endpoint, response)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DeviceRecordDocument {
    pub version: u32,
    pub user_id: String,
    pub device: crate::device::DeviceRecord,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SnapshotUploadDocument {
    pub version: u32,
    pub user_id: String,
    pub device_id: String,
    pub snapshot: crate::snapshot::SnapshotDocument,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncStatusDocument {
    pub version: u32,
    pub user_id: String,
    pub device_id: String,
    pub cursor: SyncCursorStatusDocument,
    pub objects: Vec<SyncObjectStatusDocument>,
    pub snapshots: SyncSnapshotStatusDocument,
    pub devices: SyncDeviceStatusDocument,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncCursorStatusDocument {
    pub latest_change_id: u64,
    pub total_changes: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncObjectStatusDocument {
    pub object_type: String,
    pub active_count: u64,
    pub deleted_count: u64,
    pub latest_logical_clock: u64,
    pub latest_updated_at: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncSnapshotStatusDocument {
    pub total_snapshots: u64,
    pub latest: Option<SyncLatestSnapshotDocument>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncLatestSnapshotDocument {
    pub snapshot_id: String,
    pub payload_hash: String,
    pub logical_clock: u64,
    pub device_id: String,
    pub size_bytes: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SyncDeviceStatusDocument {
    pub approved_count: u64,
    pub current_device_id: String,
    pub current_device_approved: bool,
}

fn read_json_response<T: DeserializeOwned>(
    endpoint: &str,
    response: Result<ureq::Response, ureq::Error>,
) -> Result<T, SyncClientError> {
    match response {
        Ok(ok) => {
            let status = ok.status();
            let body = ok.into_string().map_err(|error| SyncClientError::HttpStatus {
                endpoint: endpoint.to_string(),
                status,
                body: error.to_string(),
            })?;
            serde_json::from_str::<T>(&body).map_err(|error| SyncClientError::Json {
                endpoint: endpoint.to_string(),
                source: error,
            })
        }
        Err(ureq::Error::Status(status, raw)) => {
            let body = raw.into_string().unwrap_or_default();
            Err(SyncClientError::HttpStatus { endpoint: endpoint.to_string(), status, body })
        }
        Err(other) => {
            Err(SyncClientError::Http { endpoint: endpoint.to_string(), source: Box::new(other) })
        }
    }
}
