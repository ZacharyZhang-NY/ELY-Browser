import type { Env } from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import { StorageObjectError, assertKnownObjectKey } from "./storage.js";

const INVENTORY_RESCAN_SECONDS = 24 * 60 * 60;
const DEFAULT_INVENTORY_LIMIT = 100;
const SHA256_HEX = /^[a-f0-9]{64}$/;

const INVENTORY_CURSOR_QUERY = `
  SELECT prefix, cursor
  FROM sync_r2_inventory_cursors
  WHERE next_scan_at <= ?
  ORDER BY next_scan_at ASC, updated_at ASC, prefix ASC
  LIMIT 1
`;

const INVENTORY_CANDIDATE_QUERY = `
  INSERT INTO sync_r2_gc_candidates (
    r2_key, user_id, owner_hash, object_kind, state, write_token,
    lease_expires_at, gc_token, created_at, updated_at, referenced_at,
    ready_at, delete_started_at, deleted_at
  )
  SELECT ?, NULL, ?, ?, 'ready', NULL, ?, NULL, ?, ?, NULL, ?, NULL, NULL
  WHERE NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = ?)
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = ?)
    AND NOT EXISTS (
      SELECT 1
      FROM sync_snapshot_heads AS head
      INNER JOIN sync_snapshots AS snapshot
        ON snapshot.user_id = head.user_id
        AND snapshot.snapshot_id = head.snapshot_id
        AND snapshot.head_revision = head.head_revision
        AND snapshot.payload_hash = head.payload_hash
      WHERE snapshot.r2_key = ?
    )
    AND NOT EXISTS (
      SELECT 1
      FROM sync_vault_rotation_r2_objects AS staged
      INNER JOIN sync_vault_rotations AS rotation
        ON rotation.user_id = staged.user_id
        AND rotation.idempotency_key = staged.rotation_idempotency_key
      WHERE staged.r2_key = ? AND rotation.cleanup_started_at IS NULL
    )
  ON CONFLICT(r2_key) DO UPDATE SET
    state = 'ready',
    lease_expires_at = excluded.lease_expires_at,
    gc_token = NULL,
    updated_at = excluded.updated_at,
    ready_at = excluded.ready_at,
    delete_started_at = NULL,
    deleted_at = NULL
  WHERE sync_r2_gc_candidates.state = 'deleted'
`;

const INVENTORY_CURSOR_UPDATE_QUERY = `
  UPDATE sync_r2_inventory_cursors
  SET cursor = ?, updated_at = ?, next_scan_at = ?
  WHERE prefix = ?
`;

interface InventoryCursorRow { prefix: unknown; cursor: unknown }
interface R2ListObject { key: string }
interface R2ListResult {
  objects: R2ListObject[];
  truncated: boolean;
  cursor?: string;
}
interface ListableBucket {
  list(options: { prefix: string; cursor?: string; limit: number }): Promise<R2ListResult>;
}

export class SyncR2InventoryError extends Error {}

export async function inventorySyncR2Objects(
  env: Env,
  nowSeconds: number,
  limit = DEFAULT_INVENTORY_LIMIT,
): Promise<number> {
  const database = primaryD1Session(env.ELY_DB);
  const cursorRow = await database.prepare(INVENTORY_CURSOR_QUERY)
    .bind(nowSeconds)
    .first<InventoryCursorRow>();
  if (cursorRow === null) return 0;
  const prefix = inventoryPrefix(cursorRow.prefix);
  const cursor = inventoryCursor(cursorRow.cursor);
  const bucket = env.ELY_STORAGE as Env["ELY_STORAGE"] & Partial<ListableBucket>;
  if (typeof bucket.list !== "function") {
    throw new SyncR2InventoryError("sync_r2_inventory_unavailable");
  }
  const result = await bucket.list({ prefix, ...(cursor === null ? {} : { cursor }), limit });
  const statements = result.objects.flatMap((object) => {
    const candidate = inventoryCandidate(object.key, prefix);
    if (candidate === null) return [];
    return [database.prepare(INVENTORY_CANDIDATE_QUERY).bind(
      candidate.r2Key,
      candidate.ownerHash,
      candidate.objectKind,
      nowSeconds,
      nowSeconds,
      nowSeconds,
      nowSeconds,
      candidate.r2Key,
      candidate.r2Key,
      candidate.r2Key,
      candidate.r2Key,
    )];
  });
  const nextCursor = result.truncated ? result.cursor : null;
  if (result.truncated && typeof nextCursor !== "string") {
    throw new SyncR2InventoryError("sync_r2_inventory_cursor_invalid");
  }
  statements.push(database.prepare(INVENTORY_CURSOR_UPDATE_QUERY).bind(
    nextCursor,
    nowSeconds,
    result.truncated ? nowSeconds : nowSeconds + INVENTORY_RESCAN_SECONDS,
    prefix,
  ));
  await database.batch(statements);
  return statements.length - 1;
}

function inventoryCandidate(
  key: string,
  prefix: string,
): { r2Key: string; ownerHash: string; objectKind: "payload" | "snapshot" } | null {
  try {
    assertKnownObjectKey(key);
  } catch (error) {
    if (error instanceof StorageObjectError) return null;
    throw error;
  }
  if (!key.startsWith(prefix)) return null;
  const ownerHash = key.split("/")[2] ?? "";
  if (!SHA256_HEX.test(ownerHash)) return null;
  return {
    r2Key: key,
    ownerHash,
    objectKind: prefix === "sync-payloads/" ? "payload" : "snapshot",
  };
}

function inventoryPrefix(value: unknown): "sync-payloads/" | "sync-snapshots/" {
  if (value !== "sync-payloads/" && value !== "sync-snapshots/") {
    throw new SyncR2InventoryError("sync_r2_inventory_prefix_invalid");
  }
  return value;
}

function inventoryCursor(value: unknown): string | null {
  if (value !== null && typeof value !== "string") {
    throw new SyncR2InventoryError("sync_r2_inventory_cursor_invalid");
  }
  return value;
}
