import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession, ElyD1PreparedStatement, ElyD1Result, Env } from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import {
  assertDestructiveActionGateResult,
  destructiveActionGateIsLive,
  destructiveActionGateStatement,
} from "./destructive_action_gate.js";
import { deleteLegacySessionKeys } from "./legacy_auth_kv_cleanup.js";
import {
  type RecentDeviceActionProof,
  RecentDeviceActionPermissionError,
  RecentDeviceActionRequestError,
  assertFreshDeviceActionProof,
  assertRecentDeviceActionProof,
  recentDeviceActionProof,
  recentDeviceActionRequestHash,
} from "./recent_device_action_proof.js";
import {
  SYNC_R2_ANONYMIZE_USER_QUERY,
  SYNC_R2_FENCE_USER_QUERY,
  collectSyncR2Garbage,
} from "./sync_r2_gc.js";

const ACCOUNT_DELETION_CONFIRMATION = "delete-elydora-account";
const IDEMPOTENCY_KEY_PATTERN = /^[a-zA-Z0-9._:-]{16,128}$/;
const ACCOUNT_DELETION_EVENT_QUERY = `
  SELECT actor_device_id, outcome, subject_id, metadata_hash, created_at
  FROM audit_events
  WHERE event_id = ? AND event_type = 'account.delete'
`;
const ACCOUNT_DELETION_COUNTS_QUERY = `
  SELECT
    (SELECT COUNT(*) FROM user_devices WHERE user_id = ?) AS devices,
    (SELECT COUNT(*) FROM device_approvals WHERE user_id = ?) AS approvals,
    (SELECT COUNT(*) FROM sync_objects WHERE user_id = ?) AS sync_objects,
    (SELECT COUNT(*) FROM sync_change_log WHERE user_id = ?) AS sync_changes,
    (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = ?) AS sync_snapshots,
    (SELECT COUNT(*) FROM sync_tombstones WHERE user_id = ?) AS sync_tombstones,
    (SELECT COUNT(*) FROM audit_events WHERE user_id = ?) AS audit_events,
    (SELECT COUNT(*) FROM better_auth_session_device_context WHERE user_id = ?) AS session_device_contexts,
    (SELECT COUNT(*) FROM better_auth_session WHERE userId = ?) AS sessions,
    (SELECT COUNT(*) FROM better_auth_account WHERE userId = ?) AS accounts,
    (SELECT COUNT(*) FROM better_auth_user WHERE id = ?) AS users
`;
const ACCOUNT_DELETION_R2_KEYS_QUERY = `
  SELECT r2_key FROM sync_r2_gc_candidates
  WHERE user_id = ? AND state <> 'deleted'
  ORDER BY r2_key ASC
`;
const ACCOUNT_DELETION_SESSION_TOKENS_QUERY = `
  SELECT token FROM better_auth_session
  WHERE userId = ?
  ORDER BY id ASC
`;
const DELETE_SYNC_CHANGE_LOG_QUERY = "DELETE FROM sync_change_log WHERE user_id = ?";
const DELETE_SYNC_TOMBSTONES_QUERY = "DELETE FROM sync_tombstones WHERE user_id = ?";
const DELETE_SYNC_SNAPSHOT_HEADS_QUERY = "DELETE FROM sync_snapshot_heads WHERE user_id = ?";
const DELETE_SYNC_SNAPSHOT_ENCRYPTION_QUERY =
  "DELETE FROM sync_snapshot_encryption WHERE user_id = ?";
const DELETE_SYNC_SNAPSHOTS_QUERY = "DELETE FROM sync_snapshots WHERE user_id = ?";
const DELETE_SYNC_OBJECTS_QUERY = "DELETE FROM sync_objects WHERE user_id = ?";
const DELETE_SYNC_VAULT_ENVELOPES_QUERY = "DELETE FROM sync_vault_envelopes WHERE user_id = ?";
const DELETE_SYNC_VAULT_ACCOUNTS_QUERY = "DELETE FROM sync_vault_accounts WHERE user_id = ?";
const DELETE_DEVICE_APPROVALS_QUERY = "DELETE FROM device_approvals WHERE user_id = ?";
const DELETE_USER_DEVICES_QUERY = "DELETE FROM user_devices WHERE user_id = ?";
const DELETE_SESSION_DEVICE_CONTEXTS_QUERY =
  "DELETE FROM better_auth_session_device_context WHERE user_id = ?";
const DELETE_BETTER_AUTH_SESSIONS_QUERY = "DELETE FROM better_auth_session WHERE userId = ?";
const DELETE_BETTER_AUTH_ACCOUNTS_QUERY = "DELETE FROM better_auth_account WHERE userId = ?";
const DELETE_BETTER_AUTH_USER_QUERY = "DELETE FROM better_auth_user WHERE id = ?";
const DELETE_USER_AUDIT_EVENTS_QUERY = "DELETE FROM audit_events WHERE user_id = ?";

export interface AccountDeletionDocument {
  version: 1;
  account_hash: string;
  device_id: string;
  idempotency_key: string;
  deleted_at: number;
  deleted: AccountDeletionDeletedDocument;
}

export interface AccountDeletionDeletedDocument {
  devices: number;
  approvals: number;
  sync_objects: number;
  sync_changes: number;
  sync_snapshots: number;
  sync_tombstones: number;
  audit_events: number;
  session_device_contexts: number;
  sessions: number;
  accounts: number;
  users: number;
  r2_objects: number;
  kv_session_cache: number;
}

interface AccountDeletionRequest extends RecentDeviceActionProof {
  idempotencyKey: string;
}

interface AccountDeletionEventRow {
  actor_device_id: unknown;
  outcome: unknown;
  subject_id: unknown;
  metadata_hash: unknown;
  created_at: unknown;
}

interface AccountDeletionCountsRow {
  devices: unknown;
  approvals: unknown;
  sync_objects: unknown;
  sync_changes: unknown;
  sync_snapshots: unknown;
  sync_tombstones: unknown;
  audit_events: unknown;
  session_device_contexts: unknown;
  sessions: unknown;
  accounts: unknown;
  users: unknown;
}

interface AccountDeletionR2KeyRow {
  r2_key: unknown;
}
interface AccountDeletionSessionTokenRow { token: unknown }

type RequestBody = Record<string, unknown>;

export class AccountDeletionRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AccountDeletionRequestError";
  }
}

export class AccountDeletionPersistenceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AccountDeletionPersistenceError";
  }
}

export async function accountDeletionDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<AccountDeletionDocument> {
  const deviceId = currentDeviceId(context);
  const deletion = await accountDeletionRequest(request);
  const accountHash = await sha256Hex(textBytes(context.userId));
  const idempotencyHash = await sha256Hex(textBytes(deletion.idempotencyKey));
  const requestHash = await recentDeviceActionRequestHash({
    action: "account.delete",
    userId: context.userId,
    sessionId: context.sessionId,
    deviceId,
    confirmation: ACCOUNT_DELETION_CONFIRMATION,
    idempotencyKey: deletion.idempotencyKey,
    proofCreatedAt: deletion.proofCreatedAt,
    actionProof: deletion.actionProof,
  });
  const eventId = accountDeletionEventId(accountHash, idempotencyHash);
  const database = primaryD1Session(env.ELY_DB);
  const existingEvent = await database.prepare(ACCOUNT_DELETION_EVENT_QUERY)
    .bind(eventId)
    .first<AccountDeletionEventRow>();
  if (existingEvent !== null) {
    return existingDeletionDocument(accountHash, deviceId, requestHash, deletion, existingEvent);
  }
  const signingPublicKey = await assertRecentDeviceActionProof(
    database,
    context,
    "account.delete",
    ACCOUNT_DELETION_CONFIRMATION,
    deletion.idempotencyKey,
    deletion,
  );
  assertFreshDeviceActionProof(deletion, nowSeconds, true);

  const counts = await accountDeletionCounts(database, context.userId);
  const r2Keys = await accountDeletionR2Keys(database, context.userId);
  const sessionTokens = await accountDeletionSessionTokens(database, context.userId);
  let results: ElyD1Result[];
  try {
    results = await database.batch<ElyD1Result>(
    accountDeletionStatements(
      database,
      context,
      signingPublicKey,
      accountHash,
      requestHash,
      eventId,
      nowSeconds,
    ),
  );
  } catch (error) {
    const replayDatabase = primaryD1Session(env.ELY_DB);
    const racedEvent = await replayDatabase.prepare(ACCOUNT_DELETION_EVENT_QUERY)
      .bind(eventId)
      .first<AccountDeletionEventRow>();
    if (racedEvent !== null) {
      return existingDeletionDocument(accountHash, deviceId, requestHash, deletion, racedEvent);
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
  const kvSessionCache = await cleanupDeletedAccount(
    env,
    accountHash,
    sessionTokens,
    context.tokenHash,
    nowSeconds,
    r2Keys.length,
  );

  return {
    version: 1,
    account_hash: accountHash,
    device_id: deviceId,
    idempotency_key: deletion.idempotencyKey,
    deleted_at: nowSeconds,
    deleted: { ...counts, r2_objects: r2Keys.length, kv_session_cache: kvSessionCache },
  };
}

function existingDeletionDocument(
  accountHash: string,
  deviceId: string,
  requestHash: string,
  deletion: AccountDeletionRequest,
  row: AccountDeletionEventRow,
): AccountDeletionDocument {
  if (
    row.actor_device_id !== deviceId ||
    row.outcome !== "success" ||
    row.subject_id !== accountHash ||
    row.metadata_hash !== requestHash
  ) {
    throw new AccountDeletionRequestError("account_deletion_replay_mismatch");
  }
  return {
    version: 1,
    account_hash: accountHash,
    device_id: deviceId,
    idempotency_key: deletion.idempotencyKey,
    deleted_at: integer(row.created_at, "created_at"),
    deleted: emptyDeletedDocument(),
  };
}

async function accountDeletionCounts(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<Omit<AccountDeletionDeletedDocument, "r2_objects" | "kv_session_cache">> {
  const row = await database.prepare(ACCOUNT_DELETION_COUNTS_QUERY)
    .bind(userId, userId, userId, userId, userId, userId, userId, userId, userId, userId, userId)
    .first<AccountDeletionCountsRow>();
  if (row === null) {
    throw new AccountDeletionPersistenceError("account_deletion_counts_missing");
  }
  return {
    devices: integer(row.devices, "devices"),
    approvals: integer(row.approvals, "approvals"),
    sync_objects: integer(row.sync_objects, "sync_objects"),
    sync_changes: integer(row.sync_changes, "sync_changes"),
    sync_snapshots: integer(row.sync_snapshots, "sync_snapshots"),
    sync_tombstones: integer(row.sync_tombstones, "sync_tombstones"),
    audit_events: integer(row.audit_events, "audit_events"),
    session_device_contexts: integer(row.session_device_contexts, "session_device_contexts"),
    sessions: integer(row.sessions, "sessions"),
    accounts: integer(row.accounts, "accounts"),
    users: integer(row.users, "users"),
  };
}

async function accountDeletionR2Keys(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<string[]> {
  const result = await database.prepare(ACCOUNT_DELETION_R2_KEYS_QUERY)
    .bind(userId)
    .all<AccountDeletionR2KeyRow>();
  return result.results.map(r2Key);
}

async function accountDeletionSessionTokens(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<string[]> {
  const result = await database.prepare(ACCOUNT_DELETION_SESSION_TOKENS_QUERY)
    .bind(userId)
    .all<AccountDeletionSessionTokenRow>();
  return result.results.map((row) => {
    if (typeof row.token !== "string" || row.token.length === 0) {
      throw new AccountDeletionPersistenceError("session_token_invalid");
    }
    return row.token;
  });
}

function accountDeletionStatements(
  database: ElyD1DatabaseSession,
  context: AuthContext,
  signingPublicKey: string,
  accountHash: string,
  requestHash: string,
  eventId: string,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  const userId = context.userId;
  return [
    destructiveActionGateStatement(database, context, signingPublicKey, {
      eventId,
      auditUserId: null,
      eventType: "account.delete",
      subjectType: "account",
      subjectId: accountHash,
      metadataHash: requestHash,
    }, nowSeconds),
    database.prepare(SYNC_R2_FENCE_USER_QUERY).bind(
      nowSeconds,
      nowSeconds,
      nowSeconds,
      userId,
    ),
    database.prepare(DELETE_SYNC_CHANGE_LOG_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_TOMBSTONES_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_SNAPSHOT_HEADS_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_SNAPSHOT_ENCRYPTION_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_SNAPSHOTS_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_OBJECTS_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_VAULT_ENVELOPES_QUERY).bind(userId),
    database.prepare(DELETE_SYNC_VAULT_ACCOUNTS_QUERY).bind(userId),
    database.prepare(DELETE_DEVICE_APPROVALS_QUERY).bind(userId),
    database.prepare(DELETE_SESSION_DEVICE_CONTEXTS_QUERY).bind(userId),
    database.prepare(DELETE_USER_DEVICES_QUERY).bind(userId),
    database.prepare(DELETE_BETTER_AUTH_SESSIONS_QUERY).bind(userId),
    database.prepare(DELETE_BETTER_AUTH_ACCOUNTS_QUERY).bind(userId),
    database.prepare(DELETE_BETTER_AUTH_USER_QUERY).bind(userId),
    database.prepare(DELETE_USER_AUDIT_EVENTS_QUERY).bind(userId),
    database.prepare(SYNC_R2_ANONYMIZE_USER_QUERY).bind(nowSeconds, userId, accountHash),
  ];
}

async function cleanupDeletedAccount(
  env: Env,
  accountHash: string,
  sessionTokens: string[],
  currentTokenHash: string,
  nowSeconds: number,
  candidateCount: number,
): Promise<number> {
  try {
    const maxBatches = Math.ceil(candidateCount / 100) + 1;
    for (let batch = 0; batch < maxBatches; batch += 1) {
      if (await collectSyncR2Garbage(env, nowSeconds, { ownerHash: accountHash, limit: 100 }) < 100) {
        break;
      }
    }
  } catch {
    // Scheduled maintenance drains the durable GC ledger.
  }
  try {
    return await deleteLegacySessionKeys(env, sessionTokens, currentTokenHash);
  } catch {
    return 0;
  }
}

async function accountDeletionRequest(request: Request): Promise<AccountDeletionRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, [
    "version",
    "confirmation",
    "idempotency_key",
    "proof_created_at",
    "action_proof",
  ]);
  if (body.version !== 2) {
    throw new AccountDeletionRequestError("version_invalid");
  }
  if (body.confirmation !== ACCOUNT_DELETION_CONFIRMATION) {
    throw new AccountDeletionRequestError("confirmation_invalid");
  }
  try {
    return {
      idempotencyKey: idempotencyKey(body.idempotency_key),
      ...recentDeviceActionProof(body.proof_created_at, body.action_proof),
    };
  } catch (error) {
    if (error instanceof RecentDeviceActionRequestError) {
      throw new AccountDeletionRequestError(error.message);
    }
    throw error;
  }
}

async function requestBody(request: Request): Promise<RequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new AccountDeletionRequestError("json_invalid");
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new AccountDeletionRequestError("body_invalid");
  }
  return value as RequestBody;
}

function assertOnlyFields(value: RequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new AccountDeletionRequestError(`unexpected_field:${field}`);
    }
  }
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new AccountDeletionRequestError("device_context_required");
  }
  return context.deviceId;
}

function idempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !IDEMPOTENCY_KEY_PATTERN.test(value)) {
    throw new AccountDeletionRequestError("idempotency_key_invalid");
  }
  return value;
}

function r2Key(row: AccountDeletionR2KeyRow): string {
  if (typeof row.r2_key !== "string") {
    throw new AccountDeletionPersistenceError("r2_key_invalid");
  }
  return row.r2_key;
}

function integer(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new AccountDeletionPersistenceError(`${label}_invalid`);
  }
  return value;
}

function emptyDeletedDocument(): AccountDeletionDeletedDocument {
  return {
    devices: 0,
    approvals: 0,
    sync_objects: 0,
    sync_changes: 0,
    sync_snapshots: 0,
    sync_tombstones: 0,
    audit_events: 0,
    session_device_contexts: 0,
    sessions: 0,
    accounts: 0,
    users: 0,
    r2_objects: 0,
    kv_session_cache: 0,
  };
}

function accountDeletionEventId(accountHash: string, idempotencyHash: string): string {
  return `account-delete:${accountHash}:${idempotencyHash}`;
}

function textBytes(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

async function sha256Hex(payload: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", payload);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
