import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession, ElyD1Result, Env } from "./bindings.js";
import { primaryD1Session } from "./bindings.js";
import { StorageObjectError, getVerifiedObject } from "./storage.js";
import {
  SyncSnapshotRequestError,
  assertOnlyFields,
  assertOnlyQueryParams,
  assertPayloadHash,
  base64FromBytes,
  deviceIdValue,
  exactInteger,
  integer,
  payloadBytes,
  regionValue,
  requestBody,
  sha256HexValue,
  snapshotIdValue,
} from "./sync_snapshot_codec.js";
import {
  type SnapshotHeadRefDocument,
  type SyncSnapshotDocument,
  type SyncSnapshotRow,
  SyncSnapshotConflictError,
  SyncSnapshotHeadSchemaError,
  currentSyncSnapshotHead,
  sameSnapshotHead,
  snapshotDocumentFromResult,
  snapshotHeadRef,
  snapshotHeadRefValue,
  syncSnapshotByToken,
} from "./sync_snapshot_head.js";
import {
  syncSnapshotStatements,
} from "./sync_snapshot_sql.js";
import {
  SyncR2WriteFenceError,
} from "./sync_r2_gc.js";
import {
  type SyncR2WriteLease,
  claimSnapshotStorageWrite,
  persistClaimedSnapshot,
  releaseFailedSnapshotWrite,
  snapshotStorageKey,
} from "./sync_snapshot_write.js";
import {
  SyncVaultConflictError,
  SyncVaultNotFoundError,
  assertCurrentSyncVaultKey,
} from "./sync_vault.js";
import {
  SyncVaultRotationCleanupError,
  cleanupRotatedVaultStorage,
} from "./sync_vault_rotation_cleanup.js";

export { SyncSnapshotRequestError } from "./sync_snapshot_codec.js";
export { SyncSnapshotConflictError } from "./sync_snapshot_head.js";
export type { SyncSnapshotDocument } from "./sync_snapshot_head.js";

const MAX_SNAPSHOT_BYTES = 10 * 1024 * 1024;

export interface SyncSnapshotUploadDocument {
  version: 3;
  user_id: string;
  device_id: string;
  snapshot: SyncSnapshotDocument;
}

export interface SyncSnapshotDownloadDocument extends SyncSnapshotUploadDocument {
  data_base64: string;
}

interface SyncSnapshotUploadRequest {
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
  bytes: ArrayBuffer;
}

export class SyncSnapshotNotFoundError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotNotFoundError";
  }
}

export class SyncSnapshotPersistenceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotPersistenceError";
  }
}

export async function syncSnapshotUploadDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<SyncSnapshotUploadDocument> {
  const deviceId = currentDeviceId(context);
  const upload = await syncSnapshotUploadRequest(request, context.userId);
  const database = primaryD1Session(env.ELY_DB);
  const currentHead = await readCurrentHead(database, context.userId);
  if (currentHead !== null && snapshotMatchesUpload(upload, deviceId, currentHead)) {
    try {
      await assertCurrentSyncVaultKey(
        env,
        context.userId,
        upload.keyId,
        upload.vaultGeneration,
        database,
      );
    } catch (error) {
      if (error instanceof SyncVaultConflictError || error instanceof SyncVaultNotFoundError) {
        return uploadDocument(context.userId, deviceId, currentHead);
      }
      throw error;
    }
    await cleanupAfterSnapshot(env, context.userId, currentHead, nowSeconds);
    return uploadDocument(context.userId, deviceId, currentHead);
  }
  try {
    await assertCurrentSyncVaultKey(
      env,
      context.userId,
      upload.keyId,
      upload.vaultGeneration,
      database,
    );
  } catch (error) {
    if (error instanceof SyncVaultConflictError || error instanceof SyncVaultNotFoundError) {
      throw new SyncSnapshotConflictError("sync_vault_key_not_current", currentHead);
    }
    throw error;
  }

  assertUploadBase(upload, currentHead);
  let writeLease: SyncR2WriteLease;
  try {
    writeLease = await claimSnapshotStorageWrite(
      env,
      database,
      context.userId,
      deviceId,
      upload,
      nowSeconds,
    );
  } catch (error) {
    if (error instanceof SyncR2WriteFenceError) {
      const head = await readCurrentHead(database, context.userId);
      throw new SyncSnapshotConflictError("sync_snapshot_head_conflict", head);
    }
    throw error;
  }
  try {
    await persistClaimedSnapshot(env, upload);
  } catch (error) {
    await releaseFailedSnapshotWrite(
      env,
      database,
      context.userId,
      upload.r2Key,
      writeLease,
      nowSeconds,
    );
    throw error;
  }

  let results: ElyD1Result<SyncSnapshotRow>[];
  try {
    results = await database.batch<ElyD1Result<SyncSnapshotRow>>(syncSnapshotStatements(
      database,
      context.userId,
      deviceId,
      { ...upload, sizeBytes: upload.bytes.byteLength },
      writeLease,
      nowSeconds,
    ));
  } catch (error) {
    await releaseFailedSnapshotWrite(
      env,
      database,
      context.userId,
      upload.r2Key,
      writeLease,
      nowSeconds,
    );
    if (!isSnapshotHeadConflict(error)) {
      throw error;
    }
    return concurrentUploadResult(env, database, context.userId, deviceId, upload, nowSeconds);
  }
  if (
    results.length !== 5 ||
    results.slice(0, 4).some((result) => changedRowCount(result) !== 1)
  ) {
    await releaseFailedSnapshotWrite(
      env,
      database,
      context.userId,
      upload.r2Key,
      writeLease,
      nowSeconds,
    );
    return concurrentUploadResult(env, database, context.userId, deviceId, upload, nowSeconds);
  }

  let saved: SyncSnapshotDocument;
  try {
    saved = snapshotDocumentFromResult(results[4]);
  } catch (error) {
    if (error instanceof SyncSnapshotHeadSchemaError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
  if (!snapshotMatchesUpload(upload, deviceId, saved)) {
    throw new SyncSnapshotPersistenceError("sync_snapshot_mismatch");
  }
  await cleanupAfterSnapshot(env, context.userId, saved, nowSeconds);
  return uploadDocument(context.userId, deviceId, saved);
}

export async function syncSnapshotDownloadDocument(
  url: URL,
  env: Env,
  context: AuthContext,
): Promise<SyncSnapshotDownloadDocument> {
  const deviceId = currentDeviceId(context);
  const database = primaryD1Session(env.ELY_DB);
  const token = syncSnapshotDownloadQuery(url);
  const snapshot = await readSnapshotByToken(database, context.userId, token);
  if (snapshot === null) {
    const currentHead = await readCurrentHead(database, context.userId);
    if (currentHead === null) {
      throw new SyncSnapshotNotFoundError("sync_snapshot_missing");
    }
    throw new SyncSnapshotConflictError("sync_snapshot_download_token_stale", currentHead);
  }

  let payload: ArrayBuffer | null;
  try {
    payload = await getVerifiedObject(env.ELY_STORAGE, snapshot.r2_key, snapshot.payload_hash);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      await throwDownloadStorageFailure(env, context.userId, token, error.message);
    }
    throw error;
  }
  if (payload === null) {
    return throwDownloadStorageFailure(
      env,
      context.userId,
      token,
      "sync_snapshot_payload_missing",
    );
  }
  return {
    ...uploadDocument(context.userId, deviceId, snapshot),
    data_base64: base64FromBytes(payload),
  };
}

async function syncSnapshotUploadRequest(
  request: Request,
  userId: string,
): Promise<SyncSnapshotUploadRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, [
    "version",
    "snapshot_id",
    "region",
    "payload_hash",
    "encryption_version",
    "vault_generation",
    "key_id",
    "content_hash",
    "schema_rev",
    "logical_clock",
    "head_revision",
    "base_head",
    "data_base64",
  ]);
  if (body.version !== 3) {
    throw new SyncSnapshotRequestError("version_invalid");
  }

  const snapshotId = snapshotIdValue(body.snapshot_id);
  const payloadHash = sha256HexValue(body.payload_hash, "payload_hash");
  const headRevision = integer(
    body.head_revision,
    "head_revision",
    1,
    Number.MAX_SAFE_INTEGER,
  );
  const baseHead = snapshotHeadRefValue(body.base_head, headRevision);
  const bytes = payloadBytes(body.data_base64, "data_base64", MAX_SNAPSHOT_BYTES);
  await assertPayloadHash(bytes, payloadHash);
  return {
    snapshotId,
    r2Key: await snapshotStorageKey(
      regionValue(body.region),
      userId,
      snapshotId,
      payloadHash,
    ),
    payloadHash,
    encryptionVersion: exactInteger(body.encryption_version, "encryption_version", 2),
    vaultGeneration: integer(
      body.vault_generation,
      "vault_generation",
      1,
      Number.MAX_SAFE_INTEGER,
    ),
    keyId: sha256HexValue(body.key_id, "key_id"),
    contentHash: sha256HexValue(body.content_hash, "content_hash"),
    schemaRev: integer(body.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
    logicalClock: integer(body.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    headRevision,
    baseHead,
    bytes,
  };
}

function syncSnapshotDownloadQuery(url: URL): SnapshotHeadRefDocument {
  assertOnlyQueryParams(url, ["snapshot_id", "head_revision", "payload_hash"]);
  return {
    snapshot_id: snapshotIdValue(url.searchParams.get("snapshot_id")),
    revision: integer(
      numberQueryValue(url.searchParams.get("head_revision")),
      "head_revision",
      1,
      Number.MAX_SAFE_INTEGER,
    ),
    payload_hash: sha256HexValue(url.searchParams.get("payload_hash"), "payload_hash"),
  };
}

function assertUploadBase(
  upload: SyncSnapshotUploadRequest,
  currentHead: SyncSnapshotDocument | null,
): void {
  const currentRef = currentHead === null ? null : snapshotHeadRef(currentHead);
  if (!sameSnapshotHead(upload.baseHead, currentRef)) {
    throw new SyncSnapshotConflictError("sync_snapshot_head_conflict", currentHead);
  }
  if (currentHead !== null && upload.logicalClock <= currentHead.logical_clock) {
    throw new SyncSnapshotConflictError("logical_clock_stale", currentHead);
  }
}

function snapshotMatchesUpload(
  upload: SyncSnapshotUploadRequest,
  deviceId: string,
  snapshot: SyncSnapshotDocument,
): boolean {
  return snapshot.snapshot_id === upload.snapshotId &&
    snapshot.r2_key === upload.r2Key &&
    snapshot.payload_hash === upload.payloadHash &&
    snapshot.encryption_version === upload.encryptionVersion &&
    snapshot.vault_generation === upload.vaultGeneration &&
    snapshot.key_id === upload.keyId &&
    snapshot.content_hash === upload.contentHash &&
    snapshot.schema_rev === upload.schemaRev &&
    snapshot.logical_clock === upload.logicalClock &&
    snapshot.head_revision === upload.headRevision &&
    sameSnapshotHead(snapshot.base_head, upload.baseHead) &&
    snapshot.device_id === deviceId &&
    snapshot.size_bytes === upload.bytes.byteLength;
}

async function readCurrentHead(
  database: ElyD1DatabaseSession,
  userId: string,
): Promise<SyncSnapshotDocument | null> {
  try {
    return await currentSyncSnapshotHead(database, userId);
  } catch (error) {
    if (error instanceof SyncSnapshotHeadSchemaError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
}

async function readSnapshotByToken(
  database: ElyD1DatabaseSession,
  userId: string,
  token: SnapshotHeadRefDocument,
): Promise<SyncSnapshotDocument | null> {
  try {
    return await syncSnapshotByToken(database, userId, token);
  } catch (error) {
    if (error instanceof SyncSnapshotHeadSchemaError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
}

async function concurrentUploadResult(
  env: Env,
  database: ElyD1DatabaseSession,
  userId: string,
  deviceId: string,
  upload: SyncSnapshotUploadRequest,
  nowSeconds: number,
): Promise<SyncSnapshotUploadDocument> {
  const currentHead = await readCurrentHead(database, userId);
  if (currentHead !== null && snapshotMatchesUpload(upload, deviceId, currentHead)) {
    await cleanupAfterSnapshot(env, userId, currentHead, nowSeconds);
    return uploadDocument(userId, deviceId, currentHead);
  }
  throw new SyncSnapshotConflictError("sync_snapshot_head_conflict", currentHead);
}

async function throwDownloadStorageFailure(
  env: Env,
  userId: string,
  token: SnapshotHeadRefDocument,
  message: string,
): Promise<never> {
  const currentHead = await readCurrentHead(primaryD1Session(env.ELY_DB), userId);
  if (currentHead === null || !sameSnapshotHead(token, snapshotHeadRef(currentHead))) {
    throw new SyncSnapshotConflictError("sync_snapshot_download_token_stale", currentHead);
  }
  throw new SyncSnapshotPersistenceError(message);
}

async function cleanupAfterSnapshot(
  env: Env,
  userId: string,
  snapshot: SyncSnapshotDocument,
  nowSeconds: number,
): Promise<void> {
  try {
    await cleanupRotatedVaultStorage(
      env,
      userId,
      snapshot.snapshot_id,
      snapshot.key_id,
      snapshot.vault_generation,
      nowSeconds,
    );
  } catch (error) {
    if (error instanceof SyncVaultRotationCleanupError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
}

function uploadDocument(
  userId: string,
  deviceId: string,
  snapshot: SyncSnapshotDocument,
): SyncSnapshotUploadDocument {
  return { version: 3, user_id: userId, device_id: deviceId, snapshot };
}

function currentDeviceId(context: AuthContext): string {
  return deviceIdValue(context.deviceId);
}

function numberQueryValue(value: string | null): number {
  if (value === null || !/^[1-9][0-9]*$/.test(value)) {
    throw new SyncSnapshotRequestError("head_revision_invalid");
  }
  return Number(value);
}

function changedRowCount(result: ElyD1Result): number {
  const changes = result.meta?.changes;
  return typeof changes === "number" && Number.isSafeInteger(changes) && changes >= 0 ? changes : -1;
}

function isSnapshotHeadConflict(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return error.message.includes("sync_snapshot_head_cas_failed") ||
    error.message.includes("sync_r2_write_fenced") ||
    error.message.includes("sync_r2_reference_commit_invalid") ||
    error.message.includes("UNIQUE constraint failed: sync_snapshot_heads.user_id") ||
    error.message.includes("sync_snapshots.user_id, sync_snapshots.head_revision");
}
