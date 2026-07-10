import type {
  ElyD1DatabaseSession,
  ElyD1PreparedStatement,
  ElyD1Result,
  Env,
} from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import { collectSyncR2Garbage } from "./sync_r2_gc.js";

const MARK_CLEANUP_READY_QUERY = `
  UPDATE sync_vault_rotations
  SET cleanup_snapshot_id = ?, cleanup_started_at = ?
  WHERE user_id = ? AND completed_at IS NOT NULL AND storage_cleaned_at IS NULL
    AND cleanup_snapshot_id IS NULL AND new_generation <= ?
    AND EXISTS (
      SELECT 1 FROM sync_vault_accounts AS account
      WHERE account.user_id = sync_vault_rotations.user_id
        AND account.current_key_id = ? AND account.current_generation = ?
    )
    AND EXISTS (
      SELECT 1
      FROM sync_snapshots AS snapshot
      INNER JOIN sync_snapshot_encryption AS encryption
        ON encryption.user_id = snapshot.user_id
        AND encryption.snapshot_id = snapshot.snapshot_id
      INNER JOIN sync_snapshot_heads AS head
        ON head.user_id = snapshot.user_id
        AND head.snapshot_id = snapshot.snapshot_id
        AND head.head_revision = snapshot.head_revision
        AND head.payload_hash = snapshot.payload_hash
      WHERE snapshot.user_id = sync_vault_rotations.user_id
        AND snapshot.snapshot_id = ?
        AND encryption.key_id = ? AND encryption.vault_generation = ?
    )
`;
const READY_ROTATIONS_QUERY = `
  SELECT idempotency_key, new_key_id, new_generation
  FROM sync_vault_rotations
  WHERE user_id = ? AND cleanup_snapshot_id IS NOT NULL AND storage_cleaned_at IS NULL
  ORDER BY new_generation ASC, idempotency_key ASC
`;
const DELETE_CHANGE_LOG_QUERY = `
  DELETE FROM sync_change_log
  WHERE user_id = ? AND object_id IN (
    SELECT object.object_id
    FROM sync_objects AS object
    INNER JOIN sync_vault_rotation_r2_objects AS staged
      ON staged.user_id = object.user_id AND staged.r2_key = object.payload_r2_key
    WHERE object.user_id = ? AND staged.rotation_idempotency_key = ?
  )
`;
const FENCE_ROTATION_R2_QUERY = `
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    lease_expires_at = ?,
    updated_at = MAX(updated_at, ?),
    ready_at = COALESCE(ready_at, ?)
  WHERE state = 'referenced' AND r2_key IN (
    SELECT r2_key FROM sync_vault_rotation_r2_objects
    WHERE user_id = ? AND rotation_idempotency_key = ?
  )
`;
const DELETE_TOMBSTONES_QUERY = `
  DELETE FROM sync_tombstones
  WHERE user_id = ? AND object_id IN (
    SELECT object.object_id
    FROM sync_objects AS object
    INNER JOIN sync_vault_rotation_r2_objects AS staged
      ON staged.user_id = object.user_id AND staged.r2_key = object.payload_r2_key
    WHERE object.user_id = ? AND staged.rotation_idempotency_key = ?
  )
`;
const DELETE_OBJECTS_QUERY = `
  DELETE FROM sync_objects
  WHERE user_id = ? AND payload_r2_key IN (
    SELECT r2_key FROM sync_vault_rotation_r2_objects
    WHERE user_id = ? AND rotation_idempotency_key = ?
  )
`;
const DELETE_SNAPSHOT_ENCRYPTION_QUERY = `
  DELETE FROM sync_snapshot_encryption
  WHERE user_id = ?
    AND NOT (key_id = ? AND vault_generation = ?)
    AND NOT EXISTS (
      SELECT 1 FROM sync_snapshot_heads AS head
      WHERE head.user_id = sync_snapshot_encryption.user_id
        AND head.snapshot_id = sync_snapshot_encryption.snapshot_id
    )
    AND snapshot_id IN (
      SELECT snapshot.snapshot_id
      FROM sync_snapshots AS snapshot
      INNER JOIN sync_vault_rotation_r2_objects AS staged
        ON staged.user_id = snapshot.user_id AND staged.r2_key = snapshot.r2_key
      WHERE snapshot.user_id = ? AND staged.rotation_idempotency_key = ?
    )
`;
const DELETE_SNAPSHOTS_QUERY = `
  DELETE FROM sync_snapshots
  WHERE user_id = ?
    AND NOT EXISTS (
      SELECT 1 FROM sync_snapshot_heads AS head
      WHERE head.user_id = sync_snapshots.user_id
        AND head.snapshot_id = sync_snapshots.snapshot_id
    )
    AND r2_key IN (
      SELECT r2_key FROM sync_vault_rotation_r2_objects
      WHERE user_id = ? AND rotation_idempotency_key = ?
    )
    AND NOT EXISTS (
      SELECT 1 FROM sync_snapshot_encryption AS encryption
      WHERE encryption.user_id = sync_snapshots.user_id
        AND encryption.snapshot_id = sync_snapshots.snapshot_id
    )
`;
const MARK_STORAGE_CLEAN_QUERY = `
  UPDATE sync_vault_rotations
  SET storage_cleaned_at = ?
  WHERE user_id = ? AND idempotency_key = ?
    AND cleanup_snapshot_id IS NOT NULL AND storage_cleaned_at IS NULL
    AND NOT EXISTS (
      SELECT 1
      FROM sync_vault_rotation_r2_objects AS staged
      WHERE staged.user_id = sync_vault_rotations.user_id
        AND staged.rotation_idempotency_key = sync_vault_rotations.idempotency_key
        AND NOT EXISTS (
          SELECT 1 FROM sync_objects AS object
          WHERE object.payload_r2_key = staged.r2_key
        )
        AND NOT EXISTS (
          SELECT 1 FROM sync_snapshots AS snapshot
          WHERE snapshot.r2_key = staged.r2_key
        )
        AND NOT EXISTS (
          SELECT 1 FROM sync_r2_gc_candidates AS candidate
          WHERE candidate.r2_key = staged.r2_key AND candidate.state = 'deleted'
        )
    )
`;
const FINALIZE_STORAGE_CLEAN_QUERY = `
  UPDATE sync_vault_rotations
  SET storage_cleaned_at = MAX(cleanup_started_at, ?)
  WHERE cleanup_snapshot_id IS NOT NULL AND storage_cleaned_at IS NULL
    AND NOT EXISTS (
      SELECT 1
      FROM sync_vault_rotation_r2_objects AS staged
      WHERE staged.user_id = sync_vault_rotations.user_id
        AND staged.rotation_idempotency_key = sync_vault_rotations.idempotency_key
        AND NOT EXISTS (
          SELECT 1 FROM sync_objects AS object
          WHERE object.payload_r2_key = staged.r2_key
        )
        AND NOT EXISTS (
          SELECT 1 FROM sync_snapshots AS snapshot
          WHERE snapshot.r2_key = staged.r2_key
        )
        AND NOT EXISTS (
          SELECT 1 FROM sync_r2_gc_candidates AS candidate
          WHERE candidate.r2_key = staged.r2_key AND candidate.state = 'deleted'
        )
    )
`;

interface RotationRow {
  idempotency_key: unknown;
  new_key_id: unknown;
  new_generation: unknown;
}

export class SyncVaultRotationCleanupError extends Error {}

export async function cleanupRotatedVaultStorage(
  env: Env,
  userId: string,
  snapshotId: string,
  keyId: string,
  generation: number,
  nowSeconds: number,
): Promise<void> {
  const database = primaryD1Session(env.ELY_DB);
  await database.prepare(MARK_CLEANUP_READY_QUERY).bind(
    snapshotId,
    nowSeconds,
    userId,
    generation,
    keyId,
    generation,
    snapshotId,
    keyId,
    generation,
  ).run();
  const ready = await database.prepare(READY_ROTATIONS_QUERY)
    .bind(userId)
    .all<RotationRow>();
  for (const row of ready.results) {
    const idempotencyKey = storedIdempotencyKey(row.idempotency_key);
    const rotationKeyId = storedKeyId(row.new_key_id);
    const rotationGeneration = storedGeneration(row.new_generation);
    await database.batch(cleanupMetadataStatements(
      database,
      userId,
      idempotencyKey,
      rotationKeyId,
      rotationGeneration,
      nowSeconds,
    ));
    await collectSyncR2Garbage(env, nowSeconds, { userId, limit: 100, database });
    const result = await database.prepare(MARK_STORAGE_CLEAN_QUERY)
      .bind(nowSeconds, userId, idempotencyKey)
      .run() as ElyD1Result;
    const changes = result.meta?.changes;
    if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes > 1 || changes < 0) {
      throw new SyncVaultRotationCleanupError("sync_vault_rotation_cleanup_write_invalid");
    }
  }
}

export async function finalizeCleanedVaultRotations(
  env: Env,
  nowSeconds: number,
): Promise<number> {
  const result = await primaryD1Session(env.ELY_DB)
    .prepare(FINALIZE_STORAGE_CLEAN_QUERY)
    .bind(nowSeconds)
    .run() as ElyD1Result;
  const changes = result.meta?.changes;
  if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes < 0) {
    throw new SyncVaultRotationCleanupError("sync_vault_rotation_cleanup_write_invalid");
  }
  return changes;
}

function cleanupMetadataStatements(
  database: ElyD1DatabaseSession,
  userId: string,
  idempotencyKey: string,
  keyId: string,
  generation: number,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  return [
    database.prepare(FENCE_ROTATION_R2_QUERY).bind(
      nowSeconds,
      nowSeconds,
      nowSeconds,
      userId,
      idempotencyKey,
    ),
    database.prepare(DELETE_CHANGE_LOG_QUERY).bind(userId, userId, idempotencyKey),
    database.prepare(DELETE_TOMBSTONES_QUERY).bind(userId, userId, idempotencyKey),
    database.prepare(DELETE_OBJECTS_QUERY).bind(userId, userId, idempotencyKey),
    database.prepare(DELETE_SNAPSHOT_ENCRYPTION_QUERY).bind(
      userId,
      keyId,
      generation,
      userId,
      idempotencyKey,
    ),
    database.prepare(DELETE_SNAPSHOTS_QUERY).bind(userId, userId, idempotencyKey),
  ];
}

function storedIdempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9._:-]{16,128}$/.test(value)) {
    throw new SyncVaultRotationCleanupError("sync_vault_rotation_idempotency_key_invalid");
  }
  return value;
}

function storedKeyId(value: unknown): string {
  if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) {
    throw new SyncVaultRotationCleanupError("sync_vault_rotation_key_id_invalid");
  }
  return value;
}

function storedGeneration(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 2) {
    throw new SyncVaultRotationCleanupError("sync_vault_rotation_generation_invalid");
  }
  return value;
}
