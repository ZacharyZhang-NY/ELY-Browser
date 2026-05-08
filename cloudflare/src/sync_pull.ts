import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";

const DEFAULT_SYNC_PULL_LIMIT = 100;
const MAX_SYNC_PULL_LIMIT = 500;
const SYNC_OBJECT_ID_PATTERN = /^[a-zA-Z0-9._:-]{1,128}$/;
const SYNC_OBJECT_TYPE_PATTERN = /^[a-z0-9][a-z0-9._:-]{0,127}$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;

const SYNC_CHANGE_LOG_QUERY = `
  SELECT
    change_id,
    object_id,
    object_type,
    operation,
    payload_hash,
    logical_clock,
    device_id,
    created_at
  FROM sync_change_log
  WHERE user_id = ? AND change_id > ?
  ORDER BY change_id ASC
  LIMIT ?
`;

export interface SyncPullDocument {
  version: 1;
  user_id: string;
  device_id: string;
  cursor: number;
  next_cursor: number;
  has_more: boolean;
  changes: SyncChangeDocument[];
}

export interface SyncChangeDocument {
  change_id: number;
  object_id: string;
  object_type: string;
  operation: "upsert" | "delete";
  payload_hash: string;
  logical_clock: number;
  device_id: string;
  created_at: number;
}

interface SyncChangeRow {
  change_id: unknown;
  object_id: unknown;
  object_type: unknown;
  operation: unknown;
  payload_hash: unknown;
  logical_clock: unknown;
  device_id: unknown;
  created_at: unknown;
}

interface SyncPullQuery {
  cursor: number;
  limit: number;
}

export class SyncSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSchemaError";
  }
}

export class SyncRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncRequestError";
  }
}

export async function syncPullDocument(
  url: URL,
  env: Env,
  context: AuthContext,
): Promise<SyncPullDocument> {
  const deviceId = currentDeviceId(context);
  const query = syncPullQuery(url);
  const result = await env.ELY_DB.prepare(SYNC_CHANGE_LOG_QUERY)
    .bind(context.userId, query.cursor, query.limit + 1)
    .all<SyncChangeRow>();
  const rows = result.results.slice(0, query.limit);
  const changes = rows.map(syncChangeDocument);
  return {
    version: 1,
    user_id: context.userId,
    device_id: deviceId,
    cursor: query.cursor,
    next_cursor: changes.at(-1)?.change_id ?? query.cursor,
    has_more: result.results.length > query.limit,
    changes,
  };
}

function syncPullQuery(url: URL): SyncPullQuery {
  assertOnlyQueryParams(url, ["cursor", "limit"]);
  const cursor = requiredQueryInteger(url, "cursor", 0, Number.MAX_SAFE_INTEGER);
  const limit = optionalQueryInteger(url, "limit", 1, MAX_SYNC_PULL_LIMIT) ?? DEFAULT_SYNC_PULL_LIMIT;
  return { cursor, limit };
}

function syncChangeDocument(row: SyncChangeRow): SyncChangeDocument {
  return {
    change_id: integerValue(row.change_id, "change_id", 0, Number.MAX_SAFE_INTEGER),
    object_id: objectId(row.object_id),
    object_type: objectType(row.object_type),
    operation: operation(row.operation),
    payload_hash: payloadHash(row.payload_hash),
    logical_clock: integerValue(row.logical_clock, "logical_clock", 0, Number.MAX_SAFE_INTEGER),
    device_id: objectId(row.device_id),
    created_at: integerValue(row.created_at, "created_at", 0, Number.MAX_SAFE_INTEGER),
  };
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncSchemaError("device_context_required");
  }
  return context.deviceId;
}

function assertOnlyQueryParams(url: URL, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of url.searchParams.keys()) {
    if (!allowed.has(field)) {
      throw new SyncRequestError(`unexpected_query:${field}`);
    }
  }
}

function requiredQueryInteger(url: URL, field: string, min: number, max: number): number {
  const value = url.searchParams.get(field);
  if (value === null) {
    throw new SyncRequestError(`${field}_required`);
  }
  return queryInteger(value, field, min, max);
}

function optionalQueryInteger(
  url: URL,
  field: string,
  min: number,
  max: number,
): number | undefined {
  const value = url.searchParams.get(field);
  if (value === null) {
    return undefined;
  }
  return queryInteger(value, field, min, max);
}

function queryInteger(value: string, field: string, min: number, max: number): number {
  if (!/^[0-9]+$/.test(value)) {
    throw new SyncRequestError(`${field}_invalid`);
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < min || parsed > max) {
    throw new SyncRequestError(`${field}_invalid`);
  }
  return parsed;
}

function integerValue(value: unknown, field: string, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new SyncSchemaError(`${field}_invalid`);
  }
  return value;
}

function objectId(value: unknown): string {
  if (typeof value !== "string" || !SYNC_OBJECT_ID_PATTERN.test(value)) {
    throw new SyncSchemaError("object_id_invalid");
  }
  return value;
}

function objectType(value: unknown): string {
  if (typeof value !== "string" || !SYNC_OBJECT_TYPE_PATTERN.test(value)) {
    throw new SyncSchemaError("object_type_invalid");
  }
  return value;
}

function operation(value: unknown): SyncChangeDocument["operation"] {
  if (value !== "upsert" && value !== "delete") {
    throw new SyncSchemaError("operation_invalid");
  }
  return value;
}

function payloadHash(value: unknown): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new SyncSchemaError("payload_hash_invalid");
  }
  return value;
}
