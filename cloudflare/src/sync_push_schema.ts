import type { AuthContext } from "./auth.js";
import {
  StorageObjectError,
  assertSyncObjectType,
  syncPayloadKey,
} from "./storage.js";

const MAX_INLINE_PAYLOAD_BYTES = 64 * 1024;
const MAX_R2_PAYLOAD_BYTES = 10 * 1024 * 1024;
const SYNC_OBJECT_ID_PATTERN = /^[a-zA-Z0-9._:-]{1,128}$/;
const SYNC_OBJECT_TYPE_PATTERN = /^[a-z0-9][a-z0-9._:-]{0,127}$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const REGION = /^[a-z0-9][a-z0-9-]{1,31}$/;

export interface SyncPushDocument {
  version: 1;
  user_id: string;
  device_id: string;
  object: SyncPushedObjectDocument;
}

export interface SyncPushedObjectDocument {
  object_id: string;
  object_type: string;
  operation: "upsert" | "delete";
  payload_hash: string;
  schema_rev: number;
  logical_clock: number;
  device_id: string;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
  payload_storage: "inline" | "r2" | "tombstone";
  payload_r2_key: string | null;
}

export interface SyncPushRequest {
  objectId: string;
  objectType: string;
  operation: "upsert" | "delete";
  payloadHash: string;
  schemaRev: number;
  logicalClock: number;
  payload: SyncPushPayload;
}

export type SyncPushPayload =
  | { kind: "inline"; bytes: ArrayBuffer; r2Key: null }
  | { kind: "r2"; bytes: ArrayBuffer; region: string; r2Key: string }
  | { kind: "tombstone"; bytes: null; r2Key: null };

export interface SyncObjectRow {
  object_id: unknown;
  object_type: unknown;
  payload_r2_key: unknown;
  payload_hash: unknown;
  schema_rev: unknown;
  logical_clock: unknown;
  device_id: unknown;
  created_at: unknown;
  updated_at: unknown;
  deleted_at: unknown;
}

type RequestBody = Record<string, unknown>;

export class SyncPushRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncPushRequestError";
  }
}

export class SyncPushConflictError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncPushConflictError";
  }
}

export class SyncPushPersistenceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncPushPersistenceError";
  }
}

export function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncPushRequestError("device_context_required");
  }
  return context.deviceId;
}

export async function syncPushRequest(
  request: Request,
  userId: string,
): Promise<SyncPushRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, [
    "version",
    "object_id",
    "object_type",
    "operation",
    "payload_hash",
    "schema_rev",
    "logical_clock",
    "payload",
  ]);
  if (body.version !== 1) {
    throw new SyncPushRequestError("version_invalid");
  }

  const objectId = syncObjectId(body.object_id);
  const operation = syncOperation(body.operation);
  const objectType = syncObjectType(body.object_type);
  const payloadHash = sha256HexValue(body.payload_hash, "payload_hash");
  const payload =
    operation === "delete"
      ? tombstonePayload(body.payload)
      : await upsertPayload(body.payload, userId, objectType, objectId, payloadHash);

  return {
    objectId,
    objectType,
    operation,
    payloadHash,
    schemaRev: integer(body.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
    logicalClock: integer(body.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    payload,
  };
}

export function syncObjectDocument(row: SyncObjectRow): SyncPushedObjectDocument {
  const deletedAt = nullableInteger(row.deleted_at, "deleted_at", 0, Number.MAX_SAFE_INTEGER);
  const payloadR2Key = nullableText(row.payload_r2_key, "payload_r2_key");
  return {
    object_id: syncObjectId(row.object_id),
    object_type: syncObjectType(row.object_type),
    operation: deletedAt === null ? "upsert" : "delete",
    payload_hash: sha256HexValue(row.payload_hash, "payload_hash"),
    schema_rev: integer(row.schema_rev, "schema_rev", 1, Number.MAX_SAFE_INTEGER),
    logical_clock: integer(row.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    device_id: syncObjectId(row.device_id),
    created_at: integer(row.created_at, "created_at", 0, Number.MAX_SAFE_INTEGER),
    updated_at: integer(row.updated_at, "updated_at", 0, Number.MAX_SAFE_INTEGER),
    deleted_at: deletedAt,
    payload_storage: deletedAt !== null ? "tombstone" : payloadR2Key === null ? "inline" : "r2",
    payload_r2_key: payloadR2Key,
  };
}

async function upsertPayload(
  value: unknown,
  userId: string,
  objectType: string,
  objectId: string,
  payloadHash: string,
): Promise<SyncPushPayload> {
  const payload = record(value, "payload");
  const kind = text(payload.kind, "payload.kind");
  if (kind === "inline") {
    assertOnlyFields(payload, ["kind", "data_base64"]);
    const bytes = payloadBytes(payload.data_base64, "payload.data_base64", MAX_INLINE_PAYLOAD_BYTES);
    await assertPayloadHash(bytes, payloadHash);
    return { kind, bytes, r2Key: null };
  }
  if (kind === "r2") {
    assertOnlyFields(payload, ["kind", "region", "data_base64"]);
    const region = regionValue(payload.region);
    const bytes = payloadBytes(payload.data_base64, "payload.data_base64", MAX_R2_PAYLOAD_BYTES);
    await assertPayloadHash(bytes, payloadHash);
    return {
      kind,
      bytes,
      region,
      r2Key: await syncPayloadStorageKey(region, userId, objectType, objectId, payloadHash),
    };
  }
  throw new SyncPushRequestError("payload.kind_invalid");
}

async function syncPayloadStorageKey(
  region: string,
  userId: string,
  objectType: string,
  objectId: string,
  payloadHash: string,
): Promise<string> {
  try {
    return syncPayloadKey({
      region,
      userHash: await sha256Hex(arrayBufferFromBytes(new TextEncoder().encode(userId))),
      objectType,
      objectId,
      payloadHash,
    });
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncPushRequestError(error.message);
    }
    throw error;
  }
}

function tombstonePayload(value: unknown): SyncPushPayload {
  if (value !== undefined) {
    throw new SyncPushRequestError("payload_forbidden");
  }
  return { kind: "tombstone", bytes: null, r2Key: null };
}

async function requestBody(request: Request): Promise<RequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new SyncPushRequestError("json_invalid");
  }
  return record(value, "body");
}

function record(value: unknown, label: string): RequestBody {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncPushRequestError(`${label}_invalid`);
  }
  return value as RequestBody;
}

function assertOnlyFields(value: RequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new SyncPushRequestError(`unexpected_field:${field}`);
    }
  }
}

function syncOperation(value: unknown): SyncPushRequest["operation"] {
  if (value !== "upsert" && value !== "delete") {
    throw new SyncPushRequestError("operation_invalid");
  }
  return value;
}

function syncObjectId(value: unknown): string {
  if (typeof value !== "string" || !SYNC_OBJECT_ID_PATTERN.test(value)) {
    throw new SyncPushRequestError("object_id_invalid");
  }
  return value;
}

function syncObjectType(value: unknown): string {
  if (typeof value !== "string" || !SYNC_OBJECT_TYPE_PATTERN.test(value)) {
    throw new SyncPushRequestError("object_type_invalid");
  }
  try {
    assertSyncObjectType(value);
  } catch (error) {
    if (error instanceof StorageObjectError) {
      throw new SyncPushRequestError(error.message);
    }
    throw error;
  }
  return value;
}

function sha256HexValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new SyncPushRequestError(`${label}_invalid`);
  }
  return value;
}

function integer(value: unknown, label: string, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new SyncPushRequestError(`${label}_invalid`);
  }
  return value;
}

function nullableInteger(value: unknown, label: string, min: number, max: number): number | null {
  if (value === null) {
    return null;
  }
  return integer(value, label, min, max);
}

function text(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new SyncPushRequestError(`${label}_invalid`);
  }
  return value;
}

function nullableText(value: unknown, label: string): string | null {
  if (value === null) {
    return null;
  }
  return text(value, label);
}

function regionValue(value: unknown): string {
  if (typeof value !== "string" || !REGION.test(value)) {
    throw new SyncPushRequestError("payload.region_invalid");
  }
  return value;
}

function payloadBytes(value: unknown, label: string, maxBytes: number): ArrayBuffer {
  const encoded = text(value, label);
  if (!BASE64.test(encoded)) {
    throw new SyncPushRequestError(`${label}_invalid`);
  }
  const bytes = bytesFromBase64(encoded);
  if (bytes.byteLength === 0 || bytes.byteLength > maxBytes) {
    throw new SyncPushRequestError(`${label}_size_invalid`);
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

function arrayBufferFromBytes(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

async function assertPayloadHash(payload: ArrayBuffer, expectedHash: string): Promise<void> {
  const actualHash = await sha256Hex(payload);
  if (actualHash !== expectedHash) {
    throw new SyncPushRequestError("payload_hash_mismatch");
  }
}

async function sha256Hex(payload: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", payload);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
