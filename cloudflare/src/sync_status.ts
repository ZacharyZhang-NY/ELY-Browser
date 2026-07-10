import type { AuthContext } from "./auth.js";
import type { ElyD1Result, Env } from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import { StorageObjectError, assertSyncObjectType } from "./storage.js";
import type { SnapshotHeadRefDocument, SyncSnapshotRow } from "./sync_snapshot_head.js";
import {
  SyncSnapshotHeadSchemaError,
  snapshotDocumentFromRow,
} from "./sync_snapshot_head.js";
import { SYNC_SNAPSHOT_HEAD_QUERY } from "./sync_snapshot_sql.js";

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
  FROM sync_snapshots AS snapshots
  INNER JOIN sync_snapshot_encryption AS encryption
    ON encryption.user_id = snapshots.user_id
    AND encryption.snapshot_id = snapshots.snapshot_id
  WHERE snapshots.user_id = ? AND encryption.encryption_version IN (1, 2)
`;
const APPROVED_DEVICE_COUNT_QUERY = `
  SELECT COUNT(*) AS approved_devices
  FROM user_devices
  WHERE user_id = ? AND approval_status = 'approved' AND revoked_at IS NULL
`;

export interface SyncStatusDocument {
  version: 2;
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
  head: SyncSnapshotHeadStatusDocument | null;
}

export interface SyncSnapshotHeadStatusDocument {
  snapshot_id: string;
  payload_hash: string;
  encryption_version: 1 | 2;
  vault_generation: number;
  key_id: string;
  content_hash: string;
  logical_clock: number;
  head_revision: number;
  base_head: SnapshotHeadRefDocument | null;
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
  const database = primaryD1Session(env.ELY_DB);
  const results = await database.batch<ElyD1Result>([
    database.prepare(CHANGE_CURSOR_QUERY).bind(context.userId),
    database.prepare(OBJECT_STATUS_QUERY).bind(context.userId),
    database.prepare(SNAPSHOT_COUNT_QUERY).bind(context.userId),
    database.prepare(SYNC_SNAPSHOT_HEAD_QUERY).bind(context.userId),
    database.prepare(APPROVED_DEVICE_COUNT_QUERY).bind(context.userId),
  ]);
  if (results.length !== 5) {
    throw new SyncStatusSchemaError("sync_status_batch_invalid");
  }
  const cursorRow = oneRow<ChangeCursorRow>(results[0], "sync_cursor_status_missing");
  const objectRows = rows<ObjectStatusRow>(results[1]);
  const snapshotCountRow = oneRow<SnapshotCountRow>(
    results[2],
    "sync_snapshot_status_missing",
  );
  const snapshotHeadRow = optionalRow<SyncSnapshotRow>(results[3]);
  const deviceStatusRow = oneRow<DeviceStatusRow>(results[4], "sync_device_status_missing");

  return {
    version: 2,
    user_id: context.userId,
    device_id: deviceId,
    cursor: cursorStatus(cursorRow),
    objects: objectRows.map(objectStatus),
    snapshots: snapshotStatus(snapshotCountRow, snapshotHeadRow),
    devices: deviceStatus(deviceStatusRow, deviceId),
  };
}

function rows<T>(result: ElyD1Result | undefined): T[] {
  if (result === undefined || !Array.isArray(result.results)) {
    throw new SyncStatusSchemaError("sync_status_batch_invalid");
  }
  return result.results as T[];
}

function oneRow<T>(result: ElyD1Result | undefined, message: string): T {
  const values = rows<T>(result);
  if (values.length !== 1) {
    throw new SyncStatusSchemaError(message);
  }
  return values[0] as T;
}

function optionalRow<T>(result: ElyD1Result | undefined): T | null {
  const values = rows<T>(result);
  if (values.length > 1) {
    throw new SyncStatusSchemaError("sync_snapshot_head_rows_invalid");
  }
  return values[0] ?? null;
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
  headRow: SyncSnapshotRow | null,
): SyncSnapshotStatusDocument {
  if (countRow === null) {
    throw new SyncStatusSchemaError("sync_snapshot_status_missing");
  }
  const totalSnapshots = integer(countRow.total_snapshots, "total_snapshots");
  if ((totalSnapshots === 0) !== (headRow === null)) {
    throw new SyncStatusSchemaError("sync_snapshot_head_missing");
  }
  return {
    total_snapshots: totalSnapshots,
    head: headRow === null ? null : snapshotHeadStatus(headRow),
  };
}

function snapshotHeadStatus(row: SyncSnapshotRow): SyncSnapshotHeadStatusDocument {
  try {
    const snapshot = snapshotDocumentFromRow(row);
    return {
      snapshot_id: snapshot.snapshot_id,
      payload_hash: snapshot.payload_hash,
      encryption_version: snapshot.encryption_version,
      vault_generation: snapshot.vault_generation,
      key_id: snapshot.key_id,
      content_hash: snapshot.content_hash,
      logical_clock: snapshot.logical_clock,
      head_revision: snapshot.head_revision,
      base_head: snapshot.base_head,
      device_id: snapshot.device_id,
      size_bytes: snapshot.size_bytes,
      created_at: snapshot.created_at,
    };
  } catch (error) {
    if (error instanceof SyncSnapshotHeadSchemaError) {
      throw new SyncStatusSchemaError(error.message);
    }
    throw error;
  }
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

function integer(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new SyncStatusSchemaError(`${label}_invalid`);
  }
  return value;
}
