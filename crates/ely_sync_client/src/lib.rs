//! HTTP client for the `ely-browser-cloud` Cloudflare worker.
//!
//! The worker exposes the device-bound sync API documented in
//! `cloudflare/src/sync_*.ts`. This crate is the renderer-side counterpart:
//! it owns the Better Auth bearer token, the locally-generated device
//! identity, and the JSON wire types needed to push and pull the user's
//! browser state.
//!
//! Scope today:
//! - Bearer-token authenticated requests via `ureq`.
//! - Device registration (`POST /api/devices/register`) and listing
//!   (`GET /api/devices`).
//! - Sync status (`GET /api/sync/status`), snapshot upload
//!   (`POST /api/sync/snapshot`), and snapshot download
//!   (`GET /api/sync/snapshot?snapshot_id=…`).
//! - The full Better Auth handshake (email + OTP / OAuth). Callers obtain
//!   the bearer token out-of-band and hand it to the client.

pub mod auth;
mod auth_files;
pub mod client;
mod credential_store;
pub mod device;
mod device_api;
mod device_proof;
mod device_revocation;
pub mod device_secret_store;
pub mod email_otp;
pub mod encryption;
pub mod error;
pub mod key_store;
pub mod snapshot;
pub mod vault;
mod vault_bootstrap;

pub use auth::{BearerToken, BearerTokenStore};
pub use client::{
    ApiClientConfig, SnapshotDownloadResult, SnapshotUploadResult, SyncApiClient,
    SyncLatestSnapshotDocument, SyncSnapshotHeadConflictDocument, SyncStatusDocument,
};
pub use device::{DeviceIdentity, DeviceListResponse, DeviceRecord, DeviceRegistration};
pub use device_api::{DeviceApprovalDocument, DeviceApprovalRequest, DeviceRebindDocument};
pub use device_revocation::{DeviceRevocationDocument, DeviceRevocationRequest};
pub use device_secret_store::DeviceSecretStore;
pub use email_otp::{send_email_otp, verify_email_otp};
pub use encryption::{
    AccountKey, EncryptedSnapshot, SNAPSHOT_ENCRYPTION_VERSION, SnapshotCryptoContext,
    SnapshotHeadRef,
};
pub use error::SyncClientError;
pub use key_store::{AccountKeyStore, StoredAccountKeys};
pub use snapshot::{
    AuthenticatedSnapshot, AuthenticatedSnapshotHead, SnapshotDownload, SnapshotPayload,
    SnapshotUploadRequest,
};
pub use vault::{
    ACCOUNT_KEY_WRAP_SUITE, ACCOUNT_KEY_WRAP_VERSION, SyncVaultDocument, VaultContext,
    WrappedAccountKey,
};
pub use vault_bootstrap::SyncVaultBootstrapRequest;
