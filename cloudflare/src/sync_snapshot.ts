import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import { StorageObjectError, getVerifiedObject, putVerifiedObject, syncSnapshotKey } from "./storage.js";

const MAX_SNAPSHOT_BYTES = 10 * 1024 * 1024;
const SNAPSHOT_ID_PATTERN = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const REGION = /^[a-z0-9][a-z0-9-]{1,31}$/;
const SYNC_SNAPSHOT_BY_ID_QUERY = `
  SELECT
    snapshot_id,
    r2_key,
    payload_hash,
    schema_rev,
    logical_clock,
    device_id,
    size_bytes,
    created_at
  FROM sync_snapshots
  WHERE user_id = ? AND snapshot_id = ?
`;
const SYNC_SNAPSHOT_UPSERT_QUERY = `
  INSERT INTO sync_snapshots (
    user_id,
    snapshot_id,
    r2_key,
    payload_hash,
    schema_rev,
    logical_clock,
    device_id,
    size_bytes,
    created_at
  ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
  ON CONFLICT(user_id, snapshot_id) DO UPDATE SET
    r2_key = excluded.r2_key,
    payload_hash = excluded.payload_hash,
    schema_rev = excluded.schema_rev,
    logical_clock = excluded.logical_clock,
    device_id = excluded.device_id,
    size_bytes = excluded.size_bytes,
    created_at = excluded.created_at
  WHERE excluded.logical_clock > sync_snapshots.logical_clock
    OR (
      excluded.logical_clock = sync_snapshots.logical_clock
      AND sync_snapshots.r2_key = excluded.r2_key
      AND sync_snapshots.payload_hash = excluded.payload_hash
      AND sync_snapshots.schema_rev = excluded.schema_rev
      AND sync_snapshots.device_id = excluded.device_id
      AND sync_snapshots.size_bytes = excluded.size_bytes
    )
`;

export interface SyncSnapshotUploadDocument {
  version: 1;
  user_id: string;
  device_id: string;
  snapshot: SyncSnapshotDocument;
}

export interface SyncSnapshotDownloadDocument extends SyncSnapshotUploadDocument {
  data_base64: string;
}

export interface SyncSnapshotDocument {
  snapshot_id: string;
  r2_key: string;
  payload_hash: string;
  schema_rev: number;
  logical_clock: number;
  device_id: string;
  size_bytes: number;
  created_at: number;
}

interface SyncSnapshotUploadRequest {
  snapshotId: string;
  r2Key: string;
  payloadHash: string;
  schemaRev: number;
  logicalClock: number;
  bytes: ArrayBuffer;
}

interface SyncSnapshotRow {
  snapshot_id: unknown;
  r2_key: unknown;
  payload_hash: unknown;
  schema_rev: unknown;
  logical_clock: unknown;
  device_id: unknown;
  size_bytes: unknown;
  created_at: unknown;
}

type RequestBody = Record<string, unknown>;

export class SyncSnapshotRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotRequestError";
  }
}

export class SyncSnapshotConflictError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotConflictError";
  }
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
  const snapshot = await syncSnapshotUploadRequest(request, context.userId);
  const existingRow = await snapshotRow(env, context.userId, snapshot.snapshotId);
  if (existingRow !== null) {
    assertSnapshotCanReplaceExisting(snapshot, deviceId, syncSnapshotDocumentFromRow(existingRow));
  }

  await persistSnapshot(env, snapshot);
  await env.ELY_DB.batch([
    env.ELY_DB.prepare(SYNC_SNAPSHOT_UPSERT_QUERY).bind(
      context.userId,
      snapshot.snapshotId,
      snapshot.r2Key,
      snapshot.payloadHash,
      snapshot.schemaRev,
      snapshot.logicalClock,
      deviceId,
      snapshot.bytes.byteLength,
      nowSeconds,
    ),
  ]);

  const savedRow = await snapshotRow(env, context.userId, snapshot.snapshotId);
  if (savedRow === null) {
    throw new SyncSnapshotPersistenceError("sync_snapshot_missing");
  }
  const savedSnapshot = syncSnapshotDocumentFromRow(savedRow);
  assertSavedSnapshotMatchesUpload(snapshot, deviceId, savedSnapshot);

  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    snapshot: savedSnapshot,
  };
}

export async function syncSnapshotDownloadDocument(
  url: URL,
  env: Env,
  context: AuthContext,
): Promise<SyncSnapshotDownloadDocument> {
  const deviceId = currentDeviceId(context);
  const snapshotId = syncSnapshotDownloadQuery(url);
  const row = await snapshotRow(env, context.userId, snapshotId);
  if (row === null) {
    throw new SyncSnapshotNotFoundError("sync_snapshot_missing");
  }

  const snapshot = syncSnapshotDocumentFromRow(row);
  let payload: ArrayBuffer | null;
  try {
    payload = await getVerifiedObject(env.ELY_STORAGE, snapshot.r2_key, snapshot.payload_hash);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
  if (payload === null) {
    throw new SyncSnapshotPersistenceError("sync_snapshot_payload_missing");
  }

  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    snapshot,
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
    "schema_rev",
    "logical_clock",
    "data_base64",
  ]);
  if (body.version !== 1) {
    throw new SyncSnapshotRequestError("version_invalid");
  }

  const snapshotId = snapshotIdValue(body.snapshot_id);
  const region = regionValue(body.region);
  const payloadHash = sha256HexValue(body.payload_hash, "payload_hash");
  const bytes = payloadBytes(body.data_base64, "data_base64", MAX_SNAPSHOT_BYTES);
  await assertPayloadHash(bytes, payloadHash);
  return {
    snapshotId,
    r2Key: await snapshotStorageKey(region, userId, snapshotId),
    payloadHash,
    schemaRev: integer(body.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
    logicalClock: integer(body.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    bytes,
  };
}

function syncSnapshotDownloadQuery(url: URL): string {
  assertOnlyQueryParams(url, ["snapshot_id"]);
  return snapshotIdValue(url.searchParams.get("snapshot_id"));
}

async function snapshotStorageKey(
  region: string,
  userId: string,
  snapshotId: string,
): Promise<string> {
  try {
    return syncSnapshotKey({
      region,
      userHash: await sha256Hex(arrayBufferFromBytes(new TextEncoder().encode(userId))),
      snapshotId,
    });
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncSnapshotRequestError(error.message);
    }
    throw error;
  }
}

async function snapshotRow(
  env: Env,
  userId: string,
  snapshotId: string,
): Promise<SyncSnapshotRow | null> {
  return env.ELY_DB.prepare(SYNC_SNAPSHOT_BY_ID_QUERY).bind(userId, snapshotId).first();
}

function syncSnapshotDocumentFromRow(row: SyncSnapshotRow): SyncSnapshotDocument {
  try {
    return syncSnapshotDocument(row);
  } catch (error) {
    if (error instanceof SyncSnapshotRequestError) {
      throw new SyncSnapshotPersistenceError(error.message);
    }
    throw error;
  }
}

function syncSnapshotDocument(row: SyncSnapshotRow): SyncSnapshotDocument {
  return {
    snapshot_id: snapshotIdValue(row.snapshot_id),
    r2_key: text(row.r2_key, "r2_key"),
    payload_hash: sha256HexValue(row.payload_hash, "payload_hash"),
    schema_rev: integer(row.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
    logical_clock: integer(row.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    device_id: deviceIdValue(row.device_id),
    size_bytes: integer(row.size_bytes, "size_bytes", 1, MAX_SNAPSHOT_BYTES),
    created_at: integer(row.created_at, "created_at", 0, Number.MAX_SAFE_INTEGER),
  };
}

function assertSnapshotCanReplaceExisting(
  snapshot: SyncSnapshotUploadRequest,
  deviceId: string,
  existing: SyncSnapshotDocument,
): void {
  if (existing.logical_clock > snapshot.logicalClock) {
    throw new SyncSnapshotConflictError("logical_clock_stale");
  }
  if (existing.logical_clock < snapshot.logicalClock) {
    return;
  }
  if (
    existing.payload_hash !== snapshot.payloadHash ||
    existing.schema_rev !== snapshot.schemaRev ||
    existing.device_id !== deviceId ||
    existing.size_bytes !== snapshot.bytes.byteLength
  ) {
    throw new SyncSnapshotConflictError("logical_clock_conflict");
  }
}

function assertSavedSnapshotMatchesUpload(
  upload: SyncSnapshotUploadRequest,
  deviceId: string,
  snapshot: SyncSnapshotDocument,
): void {
  if (snapshot.logical_clock > upload.logicalClock) {
    throw new SyncSnapshotConflictError("logical_clock_stale");
  }
  if (
    snapshot.logical_clock === upload.logicalClock &&
    (snapshot.r2_key !== upload.r2Key ||
      snapshot.payload_hash !== upload.payloadHash ||
      snapshot.schema_rev !== upload.schemaRev ||
      snapshot.device_id !== deviceId ||
      snapshot.size_bytes !== upload.bytes.byteLength)
  ) {
    throw new SyncSnapshotConflictError("logical_clock_conflict");
  }
  if (
    snapshot.snapshot_id !== upload.snapshotId ||
    snapshot.r2_key !== upload.r2Key ||
    snapshot.payload_hash !== upload.payloadHash ||
    snapshot.schema_rev !== upload.schemaRev ||
    snapshot.logical_clock !== upload.logicalClock ||
    snapshot.device_id !== deviceId ||
    snapshot.size_bytes !== upload.bytes.byteLength
  ) {
    throw new SyncSnapshotPersistenceError("sync_snapshot_mismatch");
  }
}

async function persistSnapshot(env: Env, snapshot: SyncSnapshotUploadRequest): Promise<void> {
  try {
    await putVerifiedObject(
      env.ELY_STORAGE,
      snapshot.r2Key,
      snapshot.bytes,
      snapshot.payloadHash,
      "application/octet-stream",
    );
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncSnapshotRequestError(error.message);
    }
    throw error;
  }
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncSnapshotRequestError("device_context_required");
  }
  return context.deviceId;
}

async function requestBody(request: Request): Promise<RequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new SyncSnapshotRequestError("json_invalid");
  }
  return record(value, "body");
}

function record(value: unknown, label: string): RequestBody {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value as RequestBody;
}

function assertOnlyFields(value: RequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new SyncSnapshotRequestError(`unexpected_field:${field}`);
    }
  }
}

function assertOnlyQueryParams(url: URL, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of url.searchParams.keys()) {
    if (!allowed.has(field)) {
      throw new SyncSnapshotRequestError(`unexpected_query:${field}`);
    }
  }
}

function snapshotIdValue(value: unknown): string {
  if (typeof value !== "string" || !SNAPSHOT_ID_PATTERN.test(value)) {
    throw new SyncSnapshotRequestError("snapshot_id_invalid");
  }
  return value;
}

function deviceIdValue(value: unknown): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new SyncSnapshotRequestError("device_id_invalid");
  }
  return value;
}

function regionValue(value: unknown): string {
  if (typeof value !== "string" || !REGION.test(value)) {
    throw new SyncSnapshotRequestError("region_invalid");
  }
  return value;
}

function sha256HexValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

function integer(value: unknown, label: string, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

function text(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

function payloadBytes(value: unknown, label: string, maxBytes: number): ArrayBuffer {
  const encoded = text(value, label);
  if (!BASE64.test(encoded)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  const bytes = bytesFromBase64(encoded);
  if (bytes.byteLength === 0 || bytes.byteLength > maxBytes) {
    throw new SyncSnapshotRequestError(`${label}_size_invalid`);
  }
  return bytes;
}

function bytesFromBase64(value: string): ArrayBuffer {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes.buffer;
}

function base64FromBytes(payload: ArrayBuffer): string {
  const bytes = new Uint8Array(payload);
  const parts: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    parts.push(String.fromCharCode(...bytes.subarray(offset, offset + 0x8000)));
  }
  return btoa(parts.join(""));
}

function arrayBufferFromBytes(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

async function assertPayloadHash(payload: ArrayBuffer, expectedHash: string): Promise<void> {
  const actualHash = await sha256Hex(payload);
  if (actualHash !== expectedHash) {
    throw new SyncSnapshotRequestError("payload_hash_mismatch");
  }
}

async function sha256Hex(payload: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", payload);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
