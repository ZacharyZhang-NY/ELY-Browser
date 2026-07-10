use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncClientError {
    #[error("API base URL is invalid: {url}")]
    InvalidBaseUrl { url: String },

    #[error("Bearer token storage is unavailable: {0}")]
    TokenStorage(String),

    #[error("Bearer credential storage is unavailable: {0}")]
    BearerCredentialStorage(String),

    #[error("Session logout response is invalid")]
    SessionLogoutInvalid,

    #[error("Authenticated session ended")]
    SessionEnded,

    #[error("Authenticated session changed during reconciliation")]
    SessionChanged,

    #[error("Cloud Sync owner is unclaimed; sign in again to bind this browser data")]
    SyncOwnerUnclaimed,

    #[error("This browser data belongs to a different Ely account")]
    SyncOwnerMismatch,

    #[error("Cloud Sync owner storage is unavailable: {0}")]
    SyncOwnerStorage(String),

    #[error("HTTP request failed for {endpoint}: {source}")]
    Http {
        endpoint: String,
        #[source]
        source: Box<ureq::Error>,
    },

    #[error("HTTP {status} {endpoint}: {body}")]
    HttpStatus { endpoint: String, status: u16, body: String },

    #[error("JSON parsing failed for {endpoint}: {source}")]
    Json {
        endpoint: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Snapshot payload is too large: {bytes} bytes (max {limit})")]
    SnapshotTooLarge { bytes: usize, limit: usize },

    #[error("Snapshot base64 decode failed: {0}")]
    SnapshotBase64(String),

    #[error("Snapshot schema is invalid: {0}")]
    SnapshotSchema(String),

    #[error("Snapshot encryption failed: {reason}")]
    SnapshotEncryption { reason: &'static str },

    #[error("Cloud Sync snapshot head is changing; retry shortly")]
    SnapshotBusy,

    #[error("Sync account key storage is unavailable: {0}")]
    AccountKeyStorage(String),

    #[error("Sync account key is unavailable for encrypted cloud data")]
    AccountKeyUnavailable,

    #[error("Device private key storage is unavailable: {0}")]
    DeviceKeyStorage(String),

    #[error("Device private keys are unavailable for {device_id}")]
    DeviceKeyUnavailable { device_id: String },

    #[error("Device trust protocol failed: {reason}")]
    DeviceTrust { reason: &'static str },

    #[error("Sync account key vault operation failed: {reason}")]
    VaultCrypto { reason: &'static str },

    #[error("Sync policy blocks this operation: {reason}")]
    SyncPolicy { reason: String },

    #[error("Device {device_id} cannot sync with approval status {status}")]
    DeviceApprovalStatus { device_id: String, status: String },
}
