import type { AuthContext } from "./auth.js";
import type { ElyD1PreparedStatement, Env } from "./bindings.js";
import { StorageObjectError, deleteKnownObject } from "./storage.js";

const RESET_CONFIRMATION = "delete-cloud-sync-data";
const IDEMPOTENCY_KEY_PATTERN = /^[a-zA-Z0-9._:-]{16,128}$/;
const SYNC_RESET_EVENT_QUERY = `
  SELECT actor_device_id, outcome, created_at
  FROM audit_events
  WHERE user_id = ? AND event_id = ? AND event_type = 'sync.reset'
`;
const SYNC_RESET_COUNTS_QUERY = `
  SELECT
    (SELECT COUNT(*) FROM sync_objects WHERE user_id = ?) AS objects,
    (SELECT COUNT(*) FROM sync_change_log WHERE user_id = ?) AS changes,
    (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = ?) AS snapshots,
    (SELECT COUNT(*) FROM sync_tombstones WHERE user_id = ?) AS tombstones
`;
const SYNC_RESET_R2_KEYS_QUERY = `
  SELECT payload_r2_key AS r2_key
  FROM sync_objects
  WHERE user_id = ? AND payload_r2_key IS NOT NULL
  UNION
  SELECT r2_key
  FROM sync_snapshots
  WHERE user_id = ?
  ORDER BY r2_key ASC
`;
const SYNC_RESET_AUDIT_INSERT_QUERY = `
  INSERT INTO audit_events (
    event_id,
    user_id,
    actor_device_id,
    event_type,
    subject_type,
    subject_id,
    outcome,
    metadata_hash,
    created_at
  ) VALUES (?, ?, ?, 'sync.reset', 'sync', ?, 'success', NULL, ?)
  ON CONFLICT(event_id) DO NOTHING
`;
const SYNC_OBJECTS_DELETE_QUERY = "DELETE FROM sync_objects WHERE user_id = ?";
const SYNC_CHANGE_LOG_DELETE_QUERY = "DELETE FROM sync_change_log WHERE user_id = ?";
const SYNC_SNAPSHOTS_DELETE_QUERY = "DELETE FROM sync_snapshots WHERE user_id = ?";
const SYNC_TOMBSTONES_DELETE_QUERY = "DELETE FROM sync_tombstones WHERE user_id = ?";

export interface SyncResetDocument {
  version: 1;
  user_id: string;
  device_id: string;
  idempotency_key: string;
  reset_at: number;
  deleted: SyncResetDeletedDocument;
}

export interface SyncResetDeletedDocument {
  objects: number;
  changes: number;
  snapshots: number;
  tombstones: number;
  r2_objects: number;
}

interface SyncResetRequest {
  idempotencyKey: string;
}

interface SyncResetEventRow {
  actor_device_id: unknown;
  outcome: unknown;
  created_at: unknown;
}

interface SyncResetCountsRow {
  objects: unknown;
  changes: unknown;
  snapshots: unknown;
  tombstones: unknown;
}

interface SyncResetR2KeyRow {
  r2_key: unknown;
}

type RequestBody = Record<string, unknown>;

export class SyncResetRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncResetRequestError";
  }
}

export class SyncResetPersistenceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncResetPersistenceError";
  }
}

export async function syncResetDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<SyncResetDocument> {
  const deviceId = currentDeviceId(context);
  const reset = await syncResetRequest(request);
  const eventId = syncResetEventId(context.userId, reset.idempotencyKey);
  const existingEvent = await env.ELY_DB.prepare(SYNC_RESET_EVENT_QUERY)
    .bind(context.userId, eventId)
    .first<SyncResetEventRow>();
  if (existingEvent !== null) {
    return existingResetDocument(context, deviceId, reset, existingEvent);
  }

  const counts = await syncResetCounts(env, context.userId);
  const r2Keys = await syncResetR2Keys(env, context.userId);
  for (const key of r2Keys) {
    await deleteResetObject(env, key);
  }
  await env.ELY_DB.batch(syncResetStatements(env, context.userId, deviceId, eventId, nowSeconds));

  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    idempotency_key: reset.idempotencyKey,
    reset_at: nowSeconds,
    deleted: { ...counts, r2_objects: r2Keys.length },
  };
}

function existingResetDocument(
  context: AuthContext,
  deviceId: string,
  reset: SyncResetRequest,
  row: SyncResetEventRow,
): SyncResetDocument {
  if (row.actor_device_id !== deviceId || row.outcome !== "success") {
    throw new SyncResetRequestError("sync_reset_replay_mismatch");
  }
  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    idempotency_key: reset.idempotencyKey,
    reset_at: integer(row.created_at, "created_at"),
    deleted: emptyDeletedDocument(),
  };
}

async function syncResetCounts(
  env: Env,
  userId: string,
): Promise<Omit<SyncResetDeletedDocument, "r2_objects">> {
  const row = await env.ELY_DB.prepare(SYNC_RESET_COUNTS_QUERY)
    .bind(userId, userId, userId, userId)
    .first<SyncResetCountsRow>();
  if (row === null) {
    throw new SyncResetPersistenceError("sync_reset_counts_missing");
  }
  return {
    objects: integer(row.objects, "objects"),
    changes: integer(row.changes, "changes"),
    snapshots: integer(row.snapshots, "snapshots"),
    tombstones: integer(row.tombstones, "tombstones"),
  };
}

async function syncResetR2Keys(env: Env, userId: string): Promise<string[]> {
  const result = await env.ELY_DB.prepare(SYNC_RESET_R2_KEYS_QUERY)
    .bind(userId, userId)
    .all<SyncResetR2KeyRow>();
  return result.results.map(r2Key);
}

function syncResetStatements(
  env: Env,
  userId: string,
  deviceId: string,
  eventId: string,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  return [
    env.ELY_DB.prepare(SYNC_CHANGE_LOG_DELETE_QUERY).bind(userId),
    env.ELY_DB.prepare(SYNC_TOMBSTONES_DELETE_QUERY).bind(userId),
    env.ELY_DB.prepare(SYNC_SNAPSHOTS_DELETE_QUERY).bind(userId),
    env.ELY_DB.prepare(SYNC_OBJECTS_DELETE_QUERY).bind(userId),
    env.ELY_DB.prepare(SYNC_RESET_AUDIT_INSERT_QUERY).bind(
      eventId,
      userId,
      deviceId,
      userId,
      nowSeconds,
    ),
  ];
}

async function deleteResetObject(env: Env, key: string): Promise<void> {
  try {
    await deleteKnownObject(env.ELY_STORAGE, key);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncResetPersistenceError(error.message);
    }
    throw error;
  }
}

async function syncResetRequest(request: Request): Promise<SyncResetRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, ["version", "confirmation", "idempotency_key"]);
  if (body.version !== 1) {
    throw new SyncResetRequestError("version_invalid");
  }
  if (body.confirmation !== RESET_CONFIRMATION) {
    throw new SyncResetRequestError("confirmation_invalid");
  }
  return { idempotencyKey: idempotencyKey(body.idempotency_key) };
}

async function requestBody(request: Request): Promise<RequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new SyncResetRequestError("json_invalid");
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncResetRequestError("body_invalid");
  }
  return value as RequestBody;
}

function assertOnlyFields(value: RequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new SyncResetRequestError(`unexpected_field:${field}`);
    }
  }
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncResetRequestError("device_context_required");
  }
  return context.deviceId;
}

function idempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !IDEMPOTENCY_KEY_PATTERN.test(value)) {
    throw new SyncResetRequestError("idempotency_key_invalid");
  }
  return value;
}

function r2Key(row: SyncResetR2KeyRow): string {
  if (typeof row.r2_key !== "string") {
    throw new SyncResetPersistenceError("r2_key_invalid");
  }
  return row.r2_key;
}

function integer(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new SyncResetPersistenceError(`${label}_invalid`);
  }
  return value;
}

function emptyDeletedDocument(): SyncResetDeletedDocument {
  return { objects: 0, changes: 0, snapshots: 0, tombstones: 0, r2_objects: 0 };
}

function syncResetEventId(userId: string, idempotencyKey: string): string {
  return `sync-reset:${userId}:${idempotencyKey}`;
}
