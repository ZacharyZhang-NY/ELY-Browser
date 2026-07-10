import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession, ElyD1PreparedStatement, ElyD1Result, Env } from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import {
  assertDestructiveActionGateResult,
  destructiveActionGateIsLive,
  destructiveActionGateStatement,
} from "./destructive_action_gate.js";
import {
  type RecentDeviceActionProof,
  RecentDeviceActionPermissionError,
  RecentDeviceActionRequestError,
  assertFreshDeviceActionProof,
  assertRecentDeviceActionProof,
  recentDeviceActionProof,
} from "./recent_device_action_proof.js";
import { SYNC_R2_FENCE_USER_QUERY, collectSyncR2Garbage } from "./sync_r2_gc.js";

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
  SELECT r2_key FROM sync_r2_gc_candidates
  WHERE user_id = ? AND state <> 'deleted'
  ORDER BY r2_key ASC
`;
const SYNC_OBJECTS_DELETE_QUERY = "DELETE FROM sync_objects WHERE user_id = ?";
const SYNC_CHANGE_LOG_DELETE_QUERY = "DELETE FROM sync_change_log WHERE user_id = ?";
const SYNC_SNAPSHOT_HEADS_DELETE_QUERY = "DELETE FROM sync_snapshot_heads WHERE user_id = ?";
const SYNC_SNAPSHOT_ENCRYPTION_DELETE_QUERY =
  "DELETE FROM sync_snapshot_encryption WHERE user_id = ?";
const SYNC_SNAPSHOTS_DELETE_QUERY = "DELETE FROM sync_snapshots WHERE user_id = ?";
const SYNC_TOMBSTONES_DELETE_QUERY = "DELETE FROM sync_tombstones WHERE user_id = ?";
const START_ROTATION_CLEANUP_QUERY = `
  UPDATE sync_vault_rotations
  SET cleanup_snapshot_id = 'sync-reset', cleanup_started_at = ?
  WHERE user_id = ? AND completed_at IS NOT NULL
    AND cleanup_started_at IS NULL AND storage_cleaned_at IS NULL
`;

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

interface SyncResetRequest extends RecentDeviceActionProof {
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
  const database = primaryD1Session(env.ELY_DB);
  const signingPublicKey = await assertRecentDeviceActionProof(
    database,
    context,
    "sync.reset",
    RESET_CONFIRMATION,
    reset.idempotencyKey,
    reset,
  );
  const existingEvent = await database.prepare(SYNC_RESET_EVENT_QUERY)
    .bind(context.userId, eventId)
    .first<SyncResetEventRow>();
  assertFreshDeviceActionProof(reset, nowSeconds, existingEvent === null);
  if (existingEvent !== null) {
    await collectResetGarbage(env, context.userId, nowSeconds, 100);
    return existingResetDocument(context, deviceId, reset, existingEvent);
  }

  const counts = await syncResetCounts(database, context.userId);
  const r2Keys = await syncResetR2Keys(database, context.userId);
  let results: ElyD1Result[];
  try {
    results = await database.batch<ElyD1Result>(syncResetStatements(
      database,
      context,
      signingPublicKey,
      eventId,
      nowSeconds,
    ));
  } catch (error) {
    const replayDatabase = primaryD1Session(env.ELY_DB);
    const racedEvent = await replayDatabase.prepare(SYNC_RESET_EVENT_QUERY)
      .bind(context.userId, eventId)
      .first<SyncResetEventRow>();
    if (racedEvent !== null) {
      return existingResetDocument(context, deviceId, reset, racedEvent);
    }
    if (!(await destructiveActionGateIsLive(
      replayDatabase,
      context,
      signingPublicKey,
      nowSeconds,
    ))) {
      throw new RecentDeviceActionPermissionError("device_action_gate_failed");
    }
    throw error;
  }
  assertDestructiveActionGateResult(results[0]);
  await collectResetGarbage(env, context.userId, nowSeconds, r2Keys.length);

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
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<Omit<SyncResetDeletedDocument, "r2_objects">> {
  const row = await database.prepare(SYNC_RESET_COUNTS_QUERY)
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

async function syncResetR2Keys(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<string[]> {
  const result = await database.prepare(SYNC_RESET_R2_KEYS_QUERY)
    .bind(userId)
    .all<SyncResetR2KeyRow>();
  return result.results.map(r2Key);
}

function syncResetStatements(
  database: ElyD1DatabaseSession,
  context: AuthContext,
  signingPublicKey: string,
  eventId: string,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  const userId = context.userId;
  return [
    destructiveActionGateStatement(database, context, signingPublicKey, {
      eventId,
      auditUserId: userId,
      eventType: "sync.reset",
      subjectType: "sync",
      subjectId: userId,
      metadataHash: null,
    }, nowSeconds),
    database.prepare(SYNC_R2_FENCE_USER_QUERY).bind(
      nowSeconds,
      nowSeconds,
      nowSeconds,
      userId,
    ),
    database.prepare(START_ROTATION_CLEANUP_QUERY).bind(nowSeconds, userId),
    database.prepare(SYNC_CHANGE_LOG_DELETE_QUERY).bind(userId),
    database.prepare(SYNC_TOMBSTONES_DELETE_QUERY).bind(userId),
    database.prepare(SYNC_SNAPSHOT_HEADS_DELETE_QUERY).bind(userId),
    database.prepare(SYNC_SNAPSHOT_ENCRYPTION_DELETE_QUERY).bind(userId),
    database.prepare(SYNC_SNAPSHOTS_DELETE_QUERY).bind(userId),
    database.prepare(SYNC_OBJECTS_DELETE_QUERY).bind(userId),
  ];
}

async function collectResetGarbage(
  env: Env,
  userId: string,
  nowSeconds: number,
  candidateCount: number,
): Promise<void> {
  try {
    const maxBatches = Math.ceil(candidateCount / 100) + 1;
    for (let batch = 0; batch < maxBatches; batch += 1) {
      if (await collectSyncR2Garbage(env, nowSeconds, { userId, limit: 100 }) < 100) break;
    }
  } catch {
    // Scheduled maintenance drains the durable GC ledger.
  }
}

async function syncResetRequest(request: Request): Promise<SyncResetRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, [
    "version",
    "confirmation",
    "idempotency_key",
    "proof_created_at",
    "action_proof",
  ]);
  if (body.version !== 2) {
    throw new SyncResetRequestError("version_invalid");
  }
  if (body.confirmation !== RESET_CONFIRMATION) {
    throw new SyncResetRequestError("confirmation_invalid");
  }
  try {
    return {
      idempotencyKey: idempotencyKey(body.idempotency_key),
      ...recentDeviceActionProof(body.proof_created_at, body.action_proof),
    };
  } catch (error) {
    if (error instanceof RecentDeviceActionRequestError) {
      throw new SyncResetRequestError(error.message);
    }
    throw error;
  }
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
