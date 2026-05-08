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

export interface DeviceListDocument {
  version: 1;
  user_id: string;
  devices: DeviceDocument[];
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

export class DeviceSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DeviceSchemaError";
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
    devices: result.results.map((row) => deviceDocument(row, context)),
  };
}

function deviceDocument(row: DeviceRow, context: AuthContext): DeviceDocument {
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
    current: context.deviceId === deviceId,
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
