import type { ElyD1DatabaseSession, ElyD1Result } from "./bindings.js";
import { StorageObjectError, assertKnownObjectKeyHash } from "./storage.js";
import {
  SyncSnapshotRequestError,
  assertOnlyFields,
  deviceIdValue,
  integer,
  sha256HexValue,
  snapshotIdValue,
  text,
} from "./sync_snapshot_codec.js";
import {
  SYNC_SNAPSHOT_BY_TOKEN_QUERY,
  SYNC_SNAPSHOT_HEAD_QUERY,
} from "./sync_snapshot_sql.js";

const MAX_SNAPSHOT_BYTES = 10 * 1024 * 1024;

export interface SnapshotHeadRefDocument {
  revision: number;
  snapshot_id: string;
  payload_hash: string;
}

export interface SyncSnapshotDocument {
  snapshot_id: string;
  r2_key: string;
  payload_hash: string;
  encryption_version: 1 | 2;
  vault_generation: number;
  key_id: string;
  content_hash: string;
  schema_rev: number;
  logical_clock: number;
  head_revision: number;
  base_head: SnapshotHeadRefDocument | null;
  device_id: string;
  size_bytes: number;
  created_at: number;
}

export interface SyncSnapshotRow {
  snapshot_id: unknown;
  r2_key: unknown;
  payload_hash: unknown;
  encryption_version: unknown;
  vault_generation: unknown;
  key_id: unknown;
  content_hash: unknown;
  schema_rev: unknown;
  logical_clock: unknown;
  head_revision: unknown;
  base_head_revision: unknown;
  base_snapshot_id: unknown;
  base_payload_hash: unknown;
  device_id: unknown;
  size_bytes: unknown;
  created_at: unknown;
}

export interface SyncSnapshotHeadConflictDocument {
  version: 1;
  error: "sync_snapshot_head_conflict";
  current_head: SyncSnapshotDocument | null;
}

export class SyncSnapshotHeadSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotHeadSchemaError";
  }
}

export class SyncSnapshotConflictError extends Error {
  readonly currentHead: SyncSnapshotDocument | null;

  constructor(message: string, currentHead: SyncSnapshotDocument | null) {
    super(message);
    this.name = "SyncSnapshotConflictError";
    this.currentHead = currentHead;
  }

  document(): SyncSnapshotHeadConflictDocument {
    return {
      version: 1,
      error: "sync_snapshot_head_conflict",
      current_head: this.currentHead,
    };
  }
}

export function snapshotHeadRefValue(
  value: unknown,
  headRevision: number,
): SnapshotHeadRefDocument | null {
  if (value === null) {
    if (headRevision !== 1) {
      throw new SyncSnapshotRequestError("base_head_invalid");
    }
    return null;
  }
  const row = record(value, "base_head");
  assertOnlyFields(row, ["revision", "snapshot_id", "payload_hash"]);
  const revision = integer(row.revision, "base_head.revision", 1, Number.MAX_SAFE_INTEGER);
  if (revision >= Number.MAX_SAFE_INTEGER || revision + 1 !== headRevision) {
    throw new SyncSnapshotRequestError("base_head_invalid");
  }
  return {
    revision,
    snapshot_id: snapshotIdValue(row.snapshot_id),
    payload_hash: sha256HexValue(row.payload_hash, "base_head.payload_hash"),
  };
}

export function snapshotDocumentFromRow(row: SyncSnapshotRow): SyncSnapshotDocument {
  try {
    const headRevision = integer(
      row.head_revision,
      "head_revision",
      1,
      Number.MAX_SAFE_INTEGER,
    );
    const payloadHash = sha256HexValue(row.payload_hash, "payload_hash");
    return {
      snapshot_id: snapshotIdValue(row.snapshot_id),
      r2_key: storedR2Key(row.r2_key, payloadHash),
      payload_hash: payloadHash,
      encryption_version: storedEncryptionVersion(row.encryption_version),
      vault_generation: integer(
        row.vault_generation,
        "vault_generation",
        1,
        Number.MAX_SAFE_INTEGER,
      ),
      key_id: sha256HexValue(row.key_id, "key_id"),
      content_hash: sha256HexValue(row.content_hash, "content_hash"),
      schema_rev: integer(row.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
      logical_clock: integer(
        row.logical_clock,
        "logical_clock",
        0,
        Number.MAX_SAFE_INTEGER,
      ),
      head_revision: headRevision,
      base_head: storedBaseHead(row, headRevision),
      device_id: deviceIdValue(row.device_id),
      size_bytes: integer(row.size_bytes, "size_bytes", 1, MAX_SNAPSHOT_BYTES),
      created_at: integer(row.created_at, "created_at", 0, Number.MAX_SAFE_INTEGER),
    };
  } catch (error) {
    if (error instanceof SyncSnapshotRequestError) {
      throw new SyncSnapshotHeadSchemaError(error.message);
    }
    throw error;
  }
}

export function snapshotDocumentFromResult(
  result: ElyD1Result<SyncSnapshotRow> | undefined,
): SyncSnapshotDocument {
  const row = result?.results.length === 1 ? result.results[0] : undefined;
  if (row === undefined) {
    throw new SyncSnapshotHeadSchemaError("sync_snapshot_missing");
  }
  return snapshotDocumentFromRow(row);
}

export function snapshotHeadRef(snapshot: SyncSnapshotDocument): SnapshotHeadRefDocument {
  return {
    revision: snapshot.head_revision,
    snapshot_id: snapshot.snapshot_id,
    payload_hash: snapshot.payload_hash,
  };
}

export function sameSnapshotHead(
  left: SnapshotHeadRefDocument | null,
  right: SnapshotHeadRefDocument | null,
): boolean {
  if (left === null || right === null) {
    return left === right;
  }
  return left.revision === right.revision &&
    left.snapshot_id === right.snapshot_id &&
    left.payload_hash === right.payload_hash;
}

export async function currentSyncSnapshotHead(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<SyncSnapshotDocument | null> {
  const row = await database.prepare(SYNC_SNAPSHOT_HEAD_QUERY)
    .bind(userId)
    .first<SyncSnapshotRow>();
  return row === null ? null : snapshotDocumentFromRow(row);
}

export async function syncSnapshotByToken(
  database: ElyD1DatabaseSession,
  userId: string,
  head: SnapshotHeadRefDocument,
): Promise<SyncSnapshotDocument | null> {
  const row = await database.prepare(SYNC_SNAPSHOT_BY_TOKEN_QUERY)
    .bind(userId, head.snapshot_id, head.revision, head.payload_hash)
    .first<SyncSnapshotRow>();
  return row === null ? null : snapshotDocumentFromRow(row);
}

function storedBaseHead(
  row: SyncSnapshotRow,
  headRevision: number,
): SnapshotHeadRefDocument | null {
  const values = [row.base_head_revision, row.base_snapshot_id, row.base_payload_hash];
  if (values.every((value) => value === null)) {
    if (headRevision !== 1) {
      throw new SyncSnapshotRequestError("base_head_invalid");
    }
    return null;
  }
  if (values.some((value) => value === null)) {
    throw new SyncSnapshotRequestError("base_head_invalid");
  }
  const revision = integer(
    row.base_head_revision,
    "base_head.revision",
    1,
    Number.MAX_SAFE_INTEGER,
  );
  if (revision >= Number.MAX_SAFE_INTEGER || revision + 1 !== headRevision) {
    throw new SyncSnapshotRequestError("base_head_invalid");
  }
  return {
    revision,
    snapshot_id: snapshotIdValue(row.base_snapshot_id),
    payload_hash: sha256HexValue(row.base_payload_hash, "base_head.payload_hash"),
  };
}

function storedEncryptionVersion(value: unknown): 1 | 2 {
  if (value !== 1 && value !== 2) {
    throw new SyncSnapshotRequestError("encryption_version_invalid");
  }
  return value;
}

function storedR2Key(value: unknown, payloadHash: string): string {
  const key = text(value, "r2_key");
  try {
    assertKnownObjectKeyHash(key, payloadHash);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncSnapshotRequestError(error.message);
    }
    throw error;
  }
  return key;
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value as Record<string, unknown>;
}
