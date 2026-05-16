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
pub mod client;
pub mod device;
pub mod email_otp;
pub mod error;
pub mod snapshot;

pub use auth::{BearerToken, BearerTokenStore};
pub use client::{ApiClientConfig, SyncApiClient, SyncLatestSnapshotDocument, SyncStatusDocument};
pub use device::{DeviceIdentity, DeviceListResponse, DeviceRecord, DeviceRegistration};
pub use email_otp::{send_email_otp, verify_email_otp};
pub use error::SyncClientError;
pub use snapshot::{SnapshotDownload, SnapshotPayload, SnapshotUploadRequest};
