import type {
  ElyD1DatabaseSession,
  ElyD1PreparedStatement,
  ElyD1Result,
  Env,
} from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import { StorageObjectError, assertKnownObjectKey, deleteKnownObject } from "./storage.js";

const WRITE_LEASE_SECONDS = 10 * 60;
const DELETE_RETRY_SECONDS = 60;
const DEFAULT_GC_LIMIT = 25;

const CLAIM_NEW_SNAPSHOT_QUERY = `
  WITH write (
    user_id, device_id, r2_key, owner_hash, key_id, generation,
    head_revision, base_revision, base_snapshot_id, base_payload_hash,
    write_token, now_seconds, lease_expires_at
  ) AS (VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?))
  INSERT INTO sync_r2_gc_candidates (
    r2_key, user_id, owner_hash, object_kind, state, write_token,
    lease_expires_at, gc_token, created_at, updated_at, referenced_at,
    ready_at, delete_started_at, deleted_at
  )
  SELECT
    write.r2_key, write.user_id, write.owner_hash, 'snapshot', 'pending',
    write.write_token, write.lease_expires_at, NULL, write.now_seconds,
    write.now_seconds, NULL, NULL, NULL, NULL
  FROM write
  WHERE EXISTS (
      SELECT 1
      FROM sync_vault_accounts AS account
      INNER JOIN user_devices AS device
        ON device.user_id = account.user_id
        AND device.device_id = write.device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id
        AND keys.device_id = device.device_id
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
      WHERE account.user_id = write.user_id
        AND account.current_key_id = write.key_id
        AND account.current_generation = write.generation
    )
    AND (
      (
        write.head_revision = 1
        AND write.base_revision IS NULL
        AND write.base_snapshot_id IS NULL
        AND write.base_payload_hash IS NULL
        AND NOT EXISTS (
          SELECT 1 FROM sync_snapshot_heads AS head
          WHERE head.user_id = write.user_id
        )
      )
      OR
      (
        write.base_revision IS NOT NULL
        AND write.base_snapshot_id IS NOT NULL
        AND write.base_payload_hash IS NOT NULL
        AND write.head_revision = write.base_revision + 1
        AND EXISTS (
          SELECT 1 FROM sync_snapshot_heads AS head
          WHERE head.user_id = write.user_id
            AND head.head_revision = write.base_revision
            AND head.snapshot_id = write.base_snapshot_id
            AND head.payload_hash = write.base_payload_hash
        )
      )
    )
  ON CONFLICT(r2_key) DO UPDATE SET
    write_token = excluded.write_token,
    lease_expires_at = excluded.lease_expires_at,
    updated_at = excluded.updated_at
  WHERE sync_r2_gc_candidates.user_id = excluded.user_id
    AND sync_r2_gc_candidates.owner_hash = excluded.owner_hash
    AND sync_r2_gc_candidates.object_kind = 'snapshot'
    AND sync_r2_gc_candidates.state = 'pending'
`;

export const SYNC_R2_MARK_REFERENCED_QUERY = `
  UPDATE sync_r2_gc_candidates
  SET state = 'referenced', lease_expires_at = ?, updated_at = ?, referenced_at = ?
  WHERE r2_key = ? AND user_id = ?
    AND object_kind = 'snapshot'
    AND state = 'pending'
    AND write_token = ?
    AND lease_expires_at >= ?
`;

export const SYNC_R2_FENCE_USER_QUERY = `
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    lease_expires_at = CASE WHEN state = 'pending' THEN lease_expires_at ELSE ? END,
    updated_at = MAX(updated_at, ?),
    ready_at = COALESCE(ready_at, ?)
  WHERE user_id = ? AND state IN ('pending', 'referenced')
`;

export const SYNC_R2_ANONYMIZE_USER_QUERY = `
  UPDATE sync_r2_gc_candidates
  SET user_id = NULL, updated_at = MAX(updated_at, ?)
  WHERE user_id = ? AND owner_hash = ?
`;

const ABANDON_WRITE_QUERY = `
  UPDATE sync_r2_gc_candidates AS candidate
  SET
    state = 'ready',
    lease_expires_at = ?,
    updated_at = MAX(updated_at, ?),
    ready_at = COALESCE(ready_at, ?)
  WHERE candidate.r2_key = ?
    AND candidate.owner_hash = ?
    AND (candidate.user_id = ? OR candidate.user_id IS NULL)
    AND candidate.state IN ('pending', 'ready')
    AND candidate.write_token = ?
    AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = candidate.r2_key)
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = candidate.r2_key)
`;

const CLAIMED_CANDIDATES_QUERY = `
  SELECT r2_key
  FROM sync_r2_gc_candidates
  WHERE state = 'deleting' AND gc_token = ?
  ORDER BY r2_key ASC
`;

const MARK_DELETED_QUERY = `
  UPDATE sync_r2_gc_candidates
  SET state = 'deleted', updated_at = ?, deleted_at = ?
  WHERE r2_key = ? AND state = 'deleting' AND gc_token = ?
`;

interface SnapshotHeadRef {
  revision: number;
  snapshotId: string;
  payloadHash: string;
}

export interface SyncR2SnapshotWriteClaim {
  userId: string;
  deviceId: string;
  r2Key: string;
  ownerHash: string;
  keyId: string;
  generation: number;
  headRevision: number;
  baseHead: SnapshotHeadRef | null;
}

export interface SyncR2WriteLease {
  writeToken: string;
  leaseExpiresAt: number;
}

interface CandidateRow { r2_key: unknown }

export class SyncR2GcError extends Error {}
export class SyncR2WriteFenceError extends Error {}

export async function claimSyncR2SnapshotWrite(
  env: Env,
  claim: SyncR2SnapshotWriteClaim,
  nowSeconds: number,
  writeToken = randomToken(),
  database: ElyD1DatabaseSession = primaryD1Session(env.ELY_DB),
): Promise<SyncR2WriteLease> {
  const leaseExpiresAt = nowSeconds + WRITE_LEASE_SECONDS;
  const result = await database.prepare(CLAIM_NEW_SNAPSHOT_QUERY).bind(
    claim.userId,
    claim.deviceId,
    claim.r2Key,
    claim.ownerHash,
    claim.keyId,
    claim.generation,
    claim.headRevision,
    claim.baseHead?.revision ?? null,
    claim.baseHead?.snapshotId ?? null,
    claim.baseHead?.payloadHash ?? null,
    writeToken,
    nowSeconds,
    leaseExpiresAt,
  ).run();
  if (changedRows(result) !== 1) {
    throw new SyncR2WriteFenceError("sync_r2_write_fenced");
  }
  return { writeToken, leaseExpiresAt };
}

export function syncR2MarkReferencedStatement(
  database: ElyD1DatabaseSession,
  userId: string,
  r2Key: string,
  lease: SyncR2WriteLease,
  nowSeconds: number,
): ElyD1PreparedStatement {
  return database.prepare(SYNC_R2_MARK_REFERENCED_QUERY).bind(
    nowSeconds,
    nowSeconds,
    nowSeconds,
    r2Key,
    userId,
    lease.writeToken,
    nowSeconds,
  );
}

export async function abandonSyncR2Write(
  env: Env,
  userId: string,
  ownerHash: string,
  r2Key: string,
  writeToken: string,
  nowSeconds: number,
  database: ElyD1DatabaseSession = primaryD1Session(env.ELY_DB),
): Promise<void> {
  await database.prepare(ABANDON_WRITE_QUERY).bind(
    nowSeconds,
    nowSeconds,
    nowSeconds,
    r2Key,
    ownerHash,
    userId,
    writeToken,
  ).run();
}

export async function collectSyncR2Garbage(
  env: Env,
  nowSeconds: number,
  options: {
    userId?: string;
    ownerHash?: string;
    limit?: number;
    database?: ElyD1DatabaseSession;
  } = {},
): Promise<number> {
  const database = options.database ?? primaryD1Session(env.ELY_DB);
  const gcToken = randomToken();
  const retryBefore = Math.max(0, nowSeconds - DELETE_RETRY_SECONDS);
  const scope = options.userId === undefined
    ? options.ownerHash === undefined ? "global" : "owner"
    : "user";
  const scopeValue = options.userId ?? options.ownerHash;
  await database.prepare(claimGarbageQuery(scope)).bind(
    gcToken,
    nowSeconds,
    nowSeconds,
    nowSeconds,
    nowSeconds,
    retryBefore,
    ...(scopeValue === undefined ? [] : [scopeValue]),
    options.limit ?? DEFAULT_GC_LIMIT,
  ).run();
  const claimed = await database.prepare(CLAIMED_CANDIDATES_QUERY)
    .bind(gcToken)
    .all<CandidateRow>();
  for (const row of claimed.results) {
    const key = storedR2Key(row.r2_key);
    await deleteKnownObject(env.ELY_STORAGE, key);
    const result = await database.prepare(MARK_DELETED_QUERY)
      .bind(nowSeconds, nowSeconds, key, gcToken)
      .run();
    if (changedRows(result) !== 1) {
      throw new SyncR2GcError("sync_r2_gc_finalize_failed");
    }
  }
  return claimed.results.length;
}

function claimGarbageQuery(scope: "global" | "user" | "owner"): string {
  const scopeSql = scope === "user"
    ? "AND candidate.user_id = ?"
    : scope === "owner" ? "AND candidate.owner_hash = ?" : "";
  return `
    UPDATE sync_r2_gc_candidates
    SET
      state = 'deleting',
      gc_token = ?,
      delete_started_at = ?,
      updated_at = MAX(updated_at, ?),
      ready_at = COALESCE(ready_at, ?)
    WHERE r2_key IN (
      SELECT candidate.r2_key
      FROM sync_r2_gc_candidates AS candidate
      WHERE (
          (candidate.state IN ('pending', 'ready') AND candidate.lease_expires_at <= ?)
          OR (candidate.state = 'deleting' AND candidate.delete_started_at <= ?)
        )
        ${scopeSql}
        AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = candidate.r2_key)
        AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = candidate.r2_key)
        AND NOT EXISTS (
          SELECT 1
          FROM sync_snapshot_heads AS head
          INNER JOIN sync_snapshots AS snapshot
            ON snapshot.user_id = head.user_id
            AND snapshot.snapshot_id = head.snapshot_id
            AND snapshot.head_revision = head.head_revision
            AND snapshot.payload_hash = head.payload_hash
          WHERE snapshot.r2_key = candidate.r2_key
        )
        AND NOT EXISTS (
          SELECT 1
          FROM sync_vault_rotation_r2_objects AS staged
          INNER JOIN sync_vault_rotations AS rotation
            ON rotation.user_id = staged.user_id
            AND rotation.idempotency_key = staged.rotation_idempotency_key
          WHERE staged.r2_key = candidate.r2_key
            AND rotation.cleanup_started_at IS NULL
        )
      ORDER BY candidate.updated_at ASC, candidate.r2_key ASC
      LIMIT ?
    )
  `;
}

function storedR2Key(value: unknown): string {
  if (typeof value !== "string") throw new SyncR2GcError("sync_r2_key_invalid");
  try {
    assertKnownObjectKey(value);
  } catch (error) {
    if (error instanceof StorageObjectError) throw new SyncR2GcError(error.message);
    throw error;
  }
  return value;
}

function changedRows(result: unknown): number {
  if (typeof result !== "object" || result === null || !("meta" in result)) return -1;
  const changes = (result as ElyD1Result).meta?.changes;
  return typeof changes === "number" && Number.isSafeInteger(changes) ? changes : -1;
}

function randomToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
