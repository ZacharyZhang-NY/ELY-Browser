import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import { StorageObjectError, assertSyncObjectType } from "./storage.js";

const CHANGE_CURSOR_QUERY = `
  SELECT
    COALESCE(MAX(change_id), 0) AS latest_change_id,
    COUNT(*) AS total_changes
  FROM sync_change_log
  WHERE user_id = ?
`;
const OBJECT_STATUS_QUERY = `
  SELECT
    object_type,
    SUM(CASE WHEN deleted_at IS NULL THEN 1 ELSE 0 END) AS active_count,
    SUM(CASE WHEN deleted_at IS NOT NULL THEN 1 ELSE 0 END) AS deleted_count,
    COALESCE(MAX(logical_clock), 0) AS latest_logical_clock,
    COALESCE(MAX(updated_at), 0) AS latest_updated_at
  FROM sync_objects
  WHERE user_id = ?
  GROUP BY object_type
  ORDER BY object_type ASC
`;
const SNAPSHOT_COUNT_QUERY = `
  SELECT COUNT(*) AS total_snapshots
  FROM sync_snapshots
  WHERE user_id = ?
`;
const LATEST_SNAPSHOT_QUERY = `
  SELECT
    snapshot_id,
    payload_hash,
    logical_clock,
    device_id,
    size_bytes,
    created_at
  FROM sync_snapshots
  WHERE user_id = ?
  ORDER BY created_at DESC, snapshot_id ASC
  LIMIT 1
`;
const APPROVED_DEVICE_COUNT_QUERY = `
  SELECT COUNT(*) AS approved_devices
  FROM user_devices
  WHERE user_id = ? AND approval_status = 'approved' AND revoked_at IS NULL
`;

const SNAPSHOT_ID_PATTERN = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;

export interface SyncStatusDocument {
  version: 1;
  user_id: string;
  device_id: string;
  cursor: SyncCursorStatusDocument;
  objects: SyncObjectStatusDocument[];
  snapshots: SyncSnapshotStatusDocument;
  devices: SyncDeviceStatusDocument;
}

export interface SyncCursorStatusDocument {
  latest_change_id: number;
  total_changes: number;
}

export interface SyncObjectStatusDocument {
  object_type: string;
  active_count: number;
  deleted_count: number;
  latest_logical_clock: number;
  latest_updated_at: number;
}

export interface SyncSnapshotStatusDocument {
  total_snapshots: number;
  latest: SyncLatestSnapshotDocument | null;
}

export interface SyncLatestSnapshotDocument {
  snapshot_id: string;
  payload_hash: string;
  logical_clock: number;
  device_id: string;
  size_bytes: number;
  created_at: number;
}

export interface SyncDeviceStatusDocument {
  approved_count: number;
  current_device_id: string;
  current_device_approved: true;
}

interface ChangeCursorRow {
  latest_change_id: unknown;
  total_changes: unknown;
}

interface ObjectStatusRow {
  object_type: unknown;
  active_count: unknown;
  deleted_count: unknown;
  latest_logical_clock: unknown;
  latest_updated_at: unknown;
}

interface SnapshotCountRow {
  total_snapshots: unknown;
}

interface LatestSnapshotRow {
  snapshot_id: unknown;
  payload_hash: unknown;
  logical_clock: unknown;
  device_id: unknown;
  size_bytes: unknown;
  created_at: unknown;
}

interface DeviceStatusRow {
  approved_devices: unknown;
}

export class SyncStatusSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncStatusSchemaError";
  }
}

export async function syncStatusDocument(
  env: Env,
  context: AuthContext,
): Promise<SyncStatusDocument> {
  const deviceId = currentDeviceId(context);
  const cursorRow = await env.ELY_DB.prepare(CHANGE_CURSOR_QUERY)
    .bind(context.userId)
    .first<ChangeCursorRow>();
  const objectRows = await env.ELY_DB.prepare(OBJECT_STATUS_QUERY)
    .bind(context.userId)
    .all<ObjectStatusRow>();
  const snapshotCountRow = await env.ELY_DB.prepare(SNAPSHOT_COUNT_QUERY)
    .bind(context.userId)
    .first<SnapshotCountRow>();
  const latestSnapshotRow = await env.ELY_DB.prepare(LATEST_SNAPSHOT_QUERY)
    .bind(context.userId)
    .first<LatestSnapshotRow>();
  const deviceStatusRow = await env.ELY_DB.prepare(APPROVED_DEVICE_COUNT_QUERY)
    .bind(context.userId)
    .first<DeviceStatusRow>();

  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    cursor: cursorStatus(cursorRow),
    objects: objectRows.results.map(objectStatus),
    snapshots: snapshotStatus(snapshotCountRow, latestSnapshotRow),
    devices: deviceStatus(deviceStatusRow, deviceId),
  };
}

function cursorStatus(row: ChangeCursorRow | null): SyncCursorStatusDocument {
  if (row === null) {
    throw new SyncStatusSchemaError("sync_cursor_status_missing");
  }
  return {
    latest_change_id: integer(row.latest_change_id, "latest_change_id"),
    total_changes: integer(row.total_changes, "total_changes"),
  };
}

function objectStatus(row: ObjectStatusRow): SyncObjectStatusDocument {
  return {
    object_type: objectType(row.object_type),
    active_count: integer(row.active_count, "active_count"),
    deleted_count: integer(row.deleted_count, "deleted_count"),
    latest_logical_clock: integer(row.latest_logical_clock, "latest_logical_clock"),
    latest_updated_at: integer(row.latest_updated_at, "latest_updated_at"),
  };
}

function snapshotStatus(
  countRow: SnapshotCountRow | null,
  latestRow: LatestSnapshotRow | null,
): SyncSnapshotStatusDocument {
  if (countRow === null) {
    throw new SyncStatusSchemaError("sync_snapshot_status_missing");
  }
  return {
    total_snapshots: integer(countRow.total_snapshots, "total_snapshots"),
    latest: latestRow === null ? null : latestSnapshot(latestRow),
  };
}

function latestSnapshot(row: LatestSnapshotRow): SyncLatestSnapshotDocument {
  return {
    snapshot_id: snapshotId(row.snapshot_id),
    payload_hash: payloadHash(row.payload_hash),
    logical_clock: integer(row.logical_clock, "logical_clock"),
    device_id: deviceId(row.device_id),
    size_bytes: integer(row.size_bytes, "size_bytes"),
    created_at: integer(row.created_at, "created_at"),
  };
}

function deviceStatus(
  row: DeviceStatusRow | null,
  currentDeviceId: string,
): SyncDeviceStatusDocument {
  if (row === null) {
    throw new SyncStatusSchemaError("sync_device_status_missing");
  }
  return {
    approved_count: integer(row.approved_devices, "approved_devices"),
    current_device_id: currentDeviceId,
    current_device_approved: true,
  };
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncStatusSchemaError("device_context_required");
  }
  return context.deviceId;
}

function objectType(value: unknown): string {
  if (typeof value !== "string") {
    throw new SyncStatusSchemaError("object_type_invalid");
  }
  try {
    assertSyncObjectType(value);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncStatusSchemaError(error.message);
    }
    throw error;
  }
  return value;
}

function snapshotId(value: unknown): string {
  if (typeof value !== "string" || !SNAPSHOT_ID_PATTERN.test(value)) {
    throw new SyncStatusSchemaError("snapshot_id_invalid");
  }
  return value;
}

function deviceId(value: unknown): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new SyncStatusSchemaError("device_id_invalid");
  }
  return value;
}

function payloadHash(value: unknown): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new SyncStatusSchemaError("payload_hash_invalid");
  }
  return value;
}

function integer(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new SyncStatusSchemaError(`${label}_invalid`);
  }
  return value;
}
