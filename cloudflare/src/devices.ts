import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";

const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const PUBLIC_KEY_PATTERN = /^[a-fA-F0-9]{64,256}$/;
const DEVICE_TEXT_PATTERN = /^[^\p{Cc}\p{Cs}]{1,128}$/u;
const APPROVAL_STATUS = new Set(["pending", "approved", "revoked"]);

const DEVICE_LIST_QUERY = `
  SELECT
    device_id,
    public_key,
    device_name,
    platform,
    approval_status,
    created_at,
    approved_at,
    last_active_at,
    revoked_at
  FROM user_devices
  WHERE user_id = ?
  ORDER BY
    revoked_at IS NOT NULL,
    COALESCE(last_active_at, approved_at, created_at) DESC,
    device_id ASC
`;
const DEVICE_REGISTER_QUERY = `
  INSERT INTO user_devices (
    user_id,
    device_id,
    public_key,
    device_name,
    platform,
    approval_status,
    created_at,
    approved_at,
    last_active_at,
    revoked_at,
    idempotency_key
  ) VALUES (?, ?, ?, ?, ?, 'pending', ?, NULL, ?, NULL, ?)
  ON CONFLICT(user_id, idempotency_key) DO NOTHING
`;
const DEVICE_BY_IDEMPOTENCY_KEY_QUERY = `
  SELECT
    device_id,
    public_key,
    device_name,
    platform,
    approval_status,
    created_at,
    approved_at,
    last_active_at,
    revoked_at
  FROM user_devices
  WHERE user_id = ? AND idempotency_key = ?
`;

export interface DeviceListDocument {
  version: 1;
  user_id: string;
  devices: DeviceDocument[];
}

export interface DeviceRegistrationDocument {
  version: 1;
  user_id: string;
  device: DeviceDocument;
}

export interface DeviceDocument {
  device_id: string;
  public_key: string;
  device_name: string;
  platform: string;
  approval_status: "pending" | "approved" | "revoked";
  created_at: number;
  approved_at: number | null;
  last_active_at: number | null;
  revoked_at: number | null;
  current: boolean;
}

interface DeviceRow {
  device_id: unknown;
  public_key: unknown;
  device_name: unknown;
  platform: unknown;
  approval_status: unknown;
  created_at: unknown;
  approved_at: unknown;
  last_active_at: unknown;
  revoked_at: unknown;
}

interface DeviceRegistrationRequest {
  deviceId: string;
  publicKey: string;
  deviceName: string;
  platform: string;
  idempotencyKey: string;
}

type DeviceRegistrationBody = Record<string, unknown>;

export class DeviceSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DeviceSchemaError";
  }
}

export class DevicePermissionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DevicePermissionError";
  }
}

export class DevicePersistenceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DevicePersistenceError";
  }
}

export async function deviceListDocument(
  env: Env,
  context: AuthContext,
): Promise<DeviceListDocument> {
  const result = await env.ELY_DB.prepare(DEVICE_LIST_QUERY).bind(context.userId).all<DeviceRow>();
  return {
    version: 1,
    user_id: context.userId,
    devices: result.results.map((row) => deviceDocument(row, context.deviceId)),
  };
}

export async function registerDeviceDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceRegistrationDocument> {
  const registration = await deviceRegistrationRequest(request);
  if (context.deviceId !== undefined && context.deviceId !== registration.deviceId) {
    throw new DevicePermissionError("device_context_mismatch");
  }

  await env.ELY_DB.prepare(DEVICE_REGISTER_QUERY)
    .bind(
      context.userId,
      registration.deviceId,
      registration.publicKey,
      registration.deviceName,
      registration.platform,
      nowSeconds,
      nowSeconds,
      registration.idempotencyKey,
    )
    .run();
  const row = await env.ELY_DB.prepare(DEVICE_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, registration.idempotencyKey)
    .first<DeviceRow>();
  if (row === null) {
    throw new DevicePersistenceError("device_registration_missing");
  }

  return {
    version: 1,
    user_id: context.userId,
    device: deviceDocument(row, registration.deviceId),
  };
}

async function deviceRegistrationRequest(request: Request): Promise<DeviceRegistrationRequest> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new DeviceSchemaError("device_registration_json_invalid");
  }
  if (!isRecord(value)) {
    throw new DeviceSchemaError("device_registration_must_be_object");
  }
  assertOnlyFields(value, [
    "version",
    "device_id",
    "public_key",
    "device_name",
    "platform",
    "idempotency_key",
  ]);
  if (value.version !== 1) {
    throw new DeviceSchemaError("device_registration_version_invalid");
  }

  return {
    deviceId: deviceIdValue(value.device_id, "device_id"),
    publicKey: publicKeyValue(value.public_key),
    deviceName: deviceText(value.device_name, "device_name"),
    platform: deviceText(value.platform, "platform"),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
  };
}

function deviceDocument(row: DeviceRow, currentDeviceId: string | undefined): DeviceDocument {
  const deviceId = deviceIdValue(row.device_id, "device_id");
  return {
    device_id: deviceId,
    public_key: publicKeyValue(row.public_key),
    device_name: deviceText(row.device_name, "device_name"),
    platform: deviceText(row.platform, "platform"),
    approval_status: approvalStatus(row.approval_status),
    created_at: timestamp(row.created_at, "created_at"),
    approved_at: nullableTimestamp(row.approved_at, "approved_at"),
    last_active_at: nullableTimestamp(row.last_active_at, "last_active_at"),
    revoked_at: nullableTimestamp(row.revoked_at, "revoked_at"),
    current: currentDeviceId === deviceId,
  };
}

function deviceIdValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

function publicKeyValue(value: unknown): string {
  if (typeof value !== "string" || !PUBLIC_KEY_PATTERN.test(value)) {
    throw new DeviceSchemaError("public_key_invalid");
  }
  return value.toLowerCase();
}

function deviceText(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  const trimmed = value.trim();
  if (!DEVICE_TEXT_PATTERN.test(trimmed)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return trimmed;
}

function approvalStatus(value: unknown): DeviceDocument["approval_status"] {
  if (typeof value !== "string" || !APPROVAL_STATUS.has(value)) {
    throw new DeviceSchemaError("approval_status_invalid");
  }
  return value as DeviceDocument["approval_status"];
}

function idempotencyKeyValue(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9._:-]{16,128}$/.test(value)) {
    throw new DeviceSchemaError("idempotency_key_invalid");
  }
  return value;
}

function nullableTimestamp(value: unknown, label: string): number | null {
  if (value === null) {
    return null;
  }
  return timestamp(value, label);
}

function timestamp(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

function assertOnlyFields(value: DeviceRegistrationBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new DeviceSchemaError(`unexpected_field:${field}`);
    }
  }
}

function isRecord(value: unknown): value is DeviceRegistrationBody {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
