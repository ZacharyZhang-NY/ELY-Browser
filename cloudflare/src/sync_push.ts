import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import type { ElyD1PreparedStatement } from "./bindings.js";
import { StorageObjectError, putVerifiedObject } from "./storage.js";
import {
  type SyncObjectRow,
  SyncPushConflictError,
  type SyncPushDocument,
  SyncPushPersistenceError,
  type SyncPushRequest,
  SyncPushRequestError,
  type SyncPushedObjectDocument,
  currentDeviceId,
  syncObjectDocument,
  syncPushRequest,
} from "./sync_push_schema.js";

export {
  SyncPushConflictError,
  SyncPushPersistenceError,
  SyncPushRequestError,
} from "./sync_push_schema.js";

const SYNC_OBJECT_BY_ID_QUERY = `
  SELECT
    object_id,
    object_type,
    payload_r2_key,
    payload_hash,
    schema_rev,
    logical_clock,
    device_id,
    created_at,
    updated_at,
    deleted_at
  FROM sync_objects
  WHERE user_id = ? AND object_id = ?
`;
const SYNC_OBJECT_UPSERT_QUERY = `
  INSERT INTO sync_objects (
    user_id,
    object_id,
    object_type,
    payload_inline,
    payload_r2_key,
    payload_hash,
    schema_rev,
    logical_clock,
    device_id,
    created_at,
    updated_at,
    deleted_at
  ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  ON CONFLICT(user_id, object_id) DO UPDATE SET
    object_type = excluded.object_type,
    payload_inline = excluded.payload_inline,
    payload_r2_key = excluded.payload_r2_key,
    payload_hash = excluded.payload_hash,
    schema_rev = excluded.schema_rev,
    logical_clock = excluded.logical_clock,
    device_id = excluded.device_id,
    updated_at = excluded.updated_at,
    deleted_at = excluded.deleted_at
  WHERE excluded.logical_clock >= sync_objects.logical_clock
`;
const SYNC_CHANGE_INSERT_QUERY = `
  INSERT INTO sync_change_log (
    user_id,
    object_id,
    object_type,
    operation,
    payload_hash,
    logical_clock,
    device_id,
    created_at
  )
  SELECT ?, ?, ?, ?, ?, ?, ?, ?
  WHERE EXISTS (
    SELECT 1
    FROM sync_objects
    WHERE user_id = ?
      AND object_id = ?
      AND object_type = ?
      AND payload_hash = ?
      AND logical_clock = ?
      AND device_id = ?
      AND ((? = 1 AND deleted_at IS NOT NULL) OR (? = 0 AND deleted_at IS NULL))
  )
  ON CONFLICT(user_id, object_id, logical_clock, device_id, operation) DO NOTHING
`;
const SYNC_TOMBSTONE_UPSERT_QUERY = `
  INSERT INTO sync_tombstones (
    user_id,
    object_id,
    object_type,
    logical_clock,
    device_id,
    deleted_at
  )
  SELECT user_id, object_id, object_type, logical_clock, device_id, deleted_at
  FROM sync_objects
  WHERE user_id = ? AND object_id = ? AND logical_clock = ? AND deleted_at IS NOT NULL
  ON CONFLICT(user_id, object_id) DO UPDATE SET
    object_type = excluded.object_type,
    logical_clock = excluded.logical_clock,
    device_id = excluded.device_id,
    deleted_at = excluded.deleted_at
  WHERE excluded.logical_clock >= sync_tombstones.logical_clock
`;

export async function syncPushDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<SyncPushDocument> {
  const deviceId = currentDeviceId(context);
  const push = await syncPushRequest(request, context.userId);
  const existingRow = await env.ELY_DB.prepare(SYNC_OBJECT_BY_ID_QUERY)
    .bind(context.userId, push.objectId)
    .first<SyncObjectRow>();
  if (existingRow !== null) {
    assertPushCanReplaceExisting(push, deviceId, syncObjectDocument(existingRow));
  }

  await persistR2PayloadIfNeeded(env, push);
  await env.ELY_DB.batch(syncPushStatements(env, context.userId, deviceId, push, nowSeconds));

  const savedRow = await env.ELY_DB.prepare(SYNC_OBJECT_BY_ID_QUERY)
    .bind(context.userId, push.objectId)
    .first<SyncObjectRow>();
  if (savedRow === null) {
    throw new SyncPushPersistenceError("sync_object_missing");
  }
  const object = syncObjectDocument(savedRow);
  assertSavedObjectMatchesPush(push, deviceId, object);

  return { version: 1, user_id: context.userId, device_id: deviceId, object };
}

async function persistR2PayloadIfNeeded(env: Env, push: SyncPushRequest): Promise<void> {
  if (push.payload.kind !== "r2") {
    return;
  }
  try {
    await putVerifiedObject(
      env.ELY_STORAGE,
      push.payload.r2Key,
      push.payload.bytes,
      push.payloadHash,
      "application/octet-stream",
    );
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncPushRequestError(error.message);
    }
    throw error;
  }
}

function syncPushStatements(
  env: Env,
  userId: string,
  deviceId: string,
  push: SyncPushRequest,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  const deletedAt = push.operation === "delete" ? nowSeconds : null;
  const isDelete = push.operation === "delete" ? 1 : 0;
  const statements = [
    env.ELY_DB.prepare(SYNC_OBJECT_UPSERT_QUERY).bind(
      userId,
      push.objectId,
      push.objectType,
      push.payload.kind === "inline" ? push.payload.bytes : null,
      push.payload.r2Key,
      push.payloadHash,
      push.schemaRev,
      push.logicalClock,
      deviceId,
      nowSeconds,
      nowSeconds,
      deletedAt,
    ),
    env.ELY_DB.prepare(SYNC_CHANGE_INSERT_QUERY).bind(
      userId,
      push.objectId,
      push.objectType,
      push.operation,
      push.payloadHash,
      push.logicalClock,
      deviceId,
      nowSeconds,
      userId,
      push.objectId,
      push.objectType,
      push.payloadHash,
      push.logicalClock,
      deviceId,
      isDelete,
      isDelete,
    ),
  ];
  if (push.operation === "delete") {
    statements.push(
      env.ELY_DB.prepare(SYNC_TOMBSTONE_UPSERT_QUERY).bind(
        userId,
        push.objectId,
        push.logicalClock,
      ),
    );
  }
  return statements;
}

function assertPushCanReplaceExisting(
  push: SyncPushRequest,
  deviceId: string,
  existing: SyncPushedObjectDocument,
): void {
  if (existing.logical_clock > push.logicalClock) {
    throw new SyncPushConflictError("logical_clock_stale");
  }
  if (existing.logical_clock < push.logicalClock) {
    return;
  }
  if (
    existing.operation !== push.operation ||
    existing.payload_hash !== push.payloadHash ||
    existing.device_id !== deviceId
  ) {
    throw new SyncPushConflictError("logical_clock_conflict");
  }
}

function assertSavedObjectMatchesPush(
  push: SyncPushRequest,
  deviceId: string,
  object: SyncPushedObjectDocument,
): void {
  if (object.logical_clock > push.logicalClock) {
    throw new SyncPushConflictError("logical_clock_stale");
  }
  if (
    object.object_id !== push.objectId ||
    object.object_type !== push.objectType ||
    object.operation !== push.operation ||
    object.payload_hash !== push.payloadHash ||
    object.schema_rev !== push.schemaRev ||
    object.logical_clock !== push.logicalClock ||
    object.device_id !== deviceId
  ) {
    throw new SyncPushPersistenceError("sync_object_mismatch");
  }
}
