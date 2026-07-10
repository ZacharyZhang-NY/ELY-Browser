const SNAPSHOT_COLUMNS = `
    snapshots.snapshot_id AS snapshot_id,
    snapshots.r2_key AS r2_key,
    snapshots.payload_hash AS payload_hash,
    encryption.encryption_version AS encryption_version,
    encryption.vault_generation AS vault_generation,
    encryption.key_id AS key_id,
    encryption.content_hash AS content_hash,
    snapshots.schema_rev AS schema_rev,
    snapshots.logical_clock AS logical_clock,
    snapshots.head_revision AS head_revision,
    snapshots.base_head_revision AS base_head_revision,
    snapshots.base_snapshot_id AS base_snapshot_id,
    snapshots.base_payload_hash AS base_payload_hash,
    snapshots.device_id AS device_id,
    snapshots.size_bytes AS size_bytes,
    snapshots.created_at AS created_at
`;

export const SYNC_SNAPSHOT_HEAD_QUERY = `
  SELECT ${SNAPSHOT_COLUMNS}
  FROM sync_snapshot_heads AS head
  INNER JOIN sync_snapshots AS snapshots
    ON snapshots.user_id = head.user_id
    AND snapshots.snapshot_id = head.snapshot_id
    AND snapshots.head_revision = head.head_revision
    AND snapshots.payload_hash = head.payload_hash
  LEFT JOIN sync_snapshot_encryption AS encryption
    ON encryption.user_id = snapshots.user_id
    AND encryption.snapshot_id = snapshots.snapshot_id
  WHERE head.user_id = ?
`;

export const SYNC_SNAPSHOT_BY_TOKEN_QUERY = `
  SELECT ${SNAPSHOT_COLUMNS}
  FROM sync_snapshot_heads AS head
  INNER JOIN sync_snapshots AS snapshots
    ON snapshots.user_id = head.user_id
    AND snapshots.snapshot_id = head.snapshot_id
    AND snapshots.head_revision = head.head_revision
    AND snapshots.payload_hash = head.payload_hash
  INNER JOIN sync_snapshot_encryption AS encryption
    ON encryption.user_id = snapshots.user_id
    AND encryption.snapshot_id = snapshots.snapshot_id
  WHERE head.user_id = ?
    AND head.snapshot_id = ?
    AND head.head_revision = ?
    AND head.payload_hash = ?
    AND encryption.encryption_version IN (1, 2)
`;

export const SYNC_SNAPSHOT_CANDIDATE_UPSERT_QUERY = `
  WITH candidate (
    user_id, snapshot_id, r2_key, payload_hash, schema_rev,
    logical_clock, device_id, size_bytes, created_at, head_revision,
    base_head_revision, base_snapshot_id, base_payload_hash, key_id, vault_generation,
    write_token, lease_now
  ) AS (VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?))
  INSERT INTO sync_snapshots (
    user_id, snapshot_id, r2_key, payload_hash, schema_rev,
    logical_clock, device_id, size_bytes, created_at, head_revision,
    base_head_revision, base_snapshot_id, base_payload_hash
  )
  SELECT
    user_id, snapshot_id, r2_key, payload_hash, schema_rev,
    logical_clock, device_id, size_bytes, created_at, head_revision,
    base_head_revision, base_snapshot_id, base_payload_hash
  FROM candidate
  WHERE (
      (
        candidate.head_revision = 1
        AND candidate.base_head_revision IS NULL
        AND candidate.base_snapshot_id IS NULL
        AND candidate.base_payload_hash IS NULL
        AND NOT EXISTS (
          SELECT 1 FROM sync_snapshot_heads AS head
          WHERE head.user_id = candidate.user_id
        )
      )
      OR
      (
        candidate.base_head_revision IS NOT NULL
        AND candidate.base_snapshot_id IS NOT NULL
        AND candidate.base_payload_hash IS NOT NULL
        AND candidate.head_revision = candidate.base_head_revision + 1
        AND EXISTS (
          SELECT 1
          FROM sync_snapshot_heads AS head
          INNER JOIN sync_snapshots AS base
            ON base.user_id = head.user_id
            AND base.snapshot_id = head.snapshot_id
            AND base.payload_hash = head.payload_hash
          WHERE head.user_id = candidate.user_id
            AND head.head_revision = candidate.base_head_revision
            AND head.snapshot_id = candidate.base_snapshot_id
            AND head.payload_hash = candidate.base_payload_hash
            AND candidate.logical_clock > base.logical_clock
        )
      )
    )
    AND EXISTS (
      SELECT 1 FROM sync_vault_accounts AS account
      WHERE account.user_id = candidate.user_id
        AND account.current_key_id = candidate.key_id
        AND account.current_generation = candidate.vault_generation
    )
    AND EXISTS (
      SELECT 1 FROM sync_r2_gc_candidates AS ledger
      WHERE ledger.r2_key = candidate.r2_key
        AND ledger.user_id = candidate.user_id
        AND ledger.object_kind = 'snapshot'
        AND ledger.state = 'pending'
        AND ledger.write_token = candidate.write_token
        AND ledger.lease_expires_at >= candidate.lease_now
    )
    AND EXISTS (
      SELECT 1
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id
        AND keys.device_id = device.device_id
      WHERE device.user_id = candidate.user_id
        AND device.device_id = candidate.device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
    )
    AND NOT EXISTS (
      SELECT 1
      FROM sync_vault_rotation_r2_objects AS staged
      INNER JOIN sync_vault_rotations AS rotation
        ON rotation.user_id = staged.user_id
        AND rotation.idempotency_key = staged.rotation_idempotency_key
      WHERE staged.user_id = candidate.user_id
        AND staged.r2_key = candidate.r2_key
        AND rotation.cleanup_started_at IS NOT NULL
    )
  ON CONFLICT(user_id, snapshot_id) DO UPDATE SET
    r2_key = excluded.r2_key,
    payload_hash = excluded.payload_hash,
    schema_rev = excluded.schema_rev,
    logical_clock = excluded.logical_clock,
    device_id = excluded.device_id,
    size_bytes = excluded.size_bytes,
    created_at = excluded.created_at,
    head_revision = excluded.head_revision,
    base_head_revision = excluded.base_head_revision,
    base_snapshot_id = excluded.base_snapshot_id,
    base_payload_hash = excluded.base_payload_hash
`;

export const SYNC_SNAPSHOT_ENCRYPTION_UPSERT_QUERY = `
  WITH candidate (
    user_id, snapshot_id, r2_key, payload_hash, schema_rev,
    logical_clock, device_id, size_bytes, created_at, head_revision,
    base_head_revision, base_snapshot_id, base_payload_hash,
    encryption_version, vault_generation, key_id, content_hash, write_token, lease_now
  ) AS (VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?))
  INSERT INTO sync_snapshot_encryption (
    user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash
  )
  SELECT
    candidate.user_id,
    candidate.snapshot_id,
    candidate.encryption_version,
    candidate.vault_generation,
    candidate.key_id,
    candidate.content_hash
  FROM candidate
  INNER JOIN sync_snapshots AS snapshot
    ON snapshot.user_id = candidate.user_id
    AND snapshot.snapshot_id = candidate.snapshot_id
    AND snapshot.r2_key = candidate.r2_key
    AND snapshot.payload_hash = candidate.payload_hash
    AND snapshot.schema_rev = candidate.schema_rev
    AND snapshot.logical_clock = candidate.logical_clock
    AND snapshot.device_id = candidate.device_id
    AND snapshot.size_bytes = candidate.size_bytes
    AND snapshot.created_at = candidate.created_at
    AND snapshot.head_revision = candidate.head_revision
    AND snapshot.base_head_revision IS candidate.base_head_revision
    AND snapshot.base_snapshot_id IS candidate.base_snapshot_id
    AND snapshot.base_payload_hash IS candidate.base_payload_hash
  INNER JOIN sync_vault_accounts AS account
    ON account.user_id = candidate.user_id
    AND account.current_key_id = candidate.key_id
    AND account.current_generation = candidate.vault_generation
  INNER JOIN user_devices AS device
    ON device.user_id = candidate.user_id
    AND device.device_id = candidate.device_id
    AND device.approval_status = 'approved'
    AND device.revoked_at IS NULL
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id
    AND keys.device_id = device.device_id
    AND keys.key_protocol_version = 2
    AND keys.wrapping_public_key IS NOT NULL
  WHERE candidate.encryption_version = 2
    AND EXISTS (
      SELECT 1 FROM sync_r2_gc_candidates AS ledger
      WHERE ledger.r2_key = candidate.r2_key
        AND ledger.user_id = candidate.user_id
        AND ledger.object_kind = 'snapshot'
        AND ledger.state = 'pending'
        AND ledger.write_token = candidate.write_token
        AND ledger.lease_expires_at >= candidate.lease_now
    )
    AND NOT EXISTS (
      SELECT 1
      FROM sync_vault_rotation_r2_objects AS staged
      INNER JOIN sync_vault_rotations AS rotation
        ON rotation.user_id = staged.user_id
        AND rotation.idempotency_key = staged.rotation_idempotency_key
      WHERE staged.user_id = candidate.user_id
        AND staged.r2_key = candidate.r2_key
        AND rotation.cleanup_started_at IS NOT NULL
    )
  ON CONFLICT(user_id, snapshot_id) DO UPDATE SET
    encryption_version = excluded.encryption_version,
    vault_generation = excluded.vault_generation,
    key_id = excluded.key_id,
    content_hash = excluded.content_hash
`;

export const SYNC_SNAPSHOT_HEAD_INSERT_QUERY = `
  INSERT INTO sync_snapshot_heads (
    user_id, head_revision, snapshot_id, payload_hash, updated_at
  )
  SELECT ?, ?, ?, ?, ?
  WHERE EXISTS (
    SELECT 1 FROM sync_r2_gc_candidates
    WHERE r2_key = ? AND user_id = ? AND object_kind = 'snapshot'
      AND state = 'pending' AND write_token = ? AND lease_expires_at >= ?
  )
`;

export const SYNC_SNAPSHOT_HEAD_UPDATE_QUERY = `
  UPDATE sync_snapshot_heads
  SET head_revision = ?, snapshot_id = ?, payload_hash = ?, updated_at = ?
  WHERE user_id = ?
    AND EXISTS (
      SELECT 1 FROM sync_r2_gc_candidates
      WHERE r2_key = ? AND user_id = ? AND object_kind = 'snapshot'
        AND state = 'pending' AND write_token = ? AND lease_expires_at >= ?
    )
`;

export function syncSnapshotStatements(
  database: ElyD1DatabaseSession,
  userId: string,
  deviceId: string,
  upload: SyncSnapshotWrite,
  writeLease: SyncR2WriteLease,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  const snapshotValues = [
    userId,
    upload.snapshotId,
    upload.r2Key,
    upload.payloadHash,
    upload.schemaRev,
    upload.logicalClock,
    deviceId,
    upload.sizeBytes,
    nowSeconds,
    upload.headRevision,
    upload.baseHead?.revision ?? null,
    upload.baseHead?.snapshot_id ?? null,
    upload.baseHead?.payload_hash ?? null,
  ];
  const headStatement = upload.baseHead === null
    ? database.prepare(SYNC_SNAPSHOT_HEAD_INSERT_QUERY).bind(
      userId,
      upload.headRevision,
      upload.snapshotId,
      upload.payloadHash,
      nowSeconds,
      upload.r2Key,
      userId,
      writeLease.writeToken,
      nowSeconds,
    )
    : database.prepare(SYNC_SNAPSHOT_HEAD_UPDATE_QUERY).bind(
      upload.headRevision,
      upload.snapshotId,
      upload.payloadHash,
      nowSeconds,
      userId,
      upload.r2Key,
      userId,
      writeLease.writeToken,
      nowSeconds,
    );
  return [
    database.prepare(SYNC_SNAPSHOT_CANDIDATE_UPSERT_QUERY).bind(
      ...snapshotValues,
      upload.keyId,
      upload.vaultGeneration,
      writeLease.writeToken,
      nowSeconds,
    ),
    database.prepare(SYNC_SNAPSHOT_ENCRYPTION_UPSERT_QUERY).bind(
      ...snapshotValues,
      upload.encryptionVersion,
      upload.vaultGeneration,
      upload.keyId,
      upload.contentHash,
      writeLease.writeToken,
      nowSeconds,
    ),
    headStatement,
    syncR2MarkReferencedStatement(database, userId, upload.r2Key, writeLease, nowSeconds),
    database.prepare(SYNC_SNAPSHOT_HEAD_QUERY).bind(userId),
  ];
}
import type { ElyD1DatabaseSession, ElyD1PreparedStatement } from "./bindings.js";
import { type SyncR2WriteLease, syncR2MarkReferencedStatement } from "./sync_r2_gc.js";
import type { SnapshotHeadRefDocument } from "./sync_snapshot_head.js";

export interface SyncSnapshotWrite {
  snapshotId: string;
  r2Key: string;
  payloadHash: string;
  encryptionVersion: 2;
  vaultGeneration: number;
  keyId: string;
  contentHash: string;
  schemaRev: number;
  logicalClock: number;
  headRevision: number;
  baseHead: SnapshotHeadRefDocument | null;
  sizeBytes: number;
}
