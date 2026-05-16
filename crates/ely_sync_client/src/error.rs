use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncClientError {
    #[error("API base URL is invalid: {url}")]
    InvalidBaseUrl { url: String },

    #[error("Bearer token storage is unavailable: {0}")]
    TokenStorage(String),

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

    #[error("Device {device_id} cannot sync with approval status {status}")]
    DeviceApprovalStatus { device_id: String, status: String },
}
