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
//! - Sync snapshot upload (`POST /api/sync/snapshot`) and download
//!   (`GET /api/sync/snapshot?snapshot_id=…`).
//!
//! Intentionally omitted (kept for follow-up work, not papered over here):
//! - The full Better Auth handshake (email + OTP / OAuth). Callers obtain
//!   the bearer token out-of-band and hand it to the client.
//! - First-device approval bootstrap. The Cloudflare API rejects sync from
//!   an unapproved device; the user must approve a freshly-registered
//!   device from another already-approved device (or via direct D1
//!   operation), exactly as the backend enforces.
//! - Incremental change-log push/pull (`/api/sync/push` and `/api/sync/pull`).
//!   The snapshot path is the simplest contract that round-trips the user's
//!   entire state, so we start there.

pub mod auth;
pub mod client;
pub mod device;
pub mod error;
pub mod snapshot;

pub use auth::{BearerToken, BearerTokenStore};
pub use client::{ApiClientConfig, SyncApiClient};
pub use device::{DeviceIdentity, DeviceListResponse, DeviceRecord, DeviceRegistration};
pub use error::SyncClientError;
pub use snapshot::{SnapshotDownload, SnapshotPayload, SnapshotUploadRequest};
