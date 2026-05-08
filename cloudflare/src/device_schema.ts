import type { AuthContext } from "./auth.js";

const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const PUBLIC_KEY_PATTERN = /^[a-fA-F0-9]{64,256}$/;
const DEVICE_TEXT_PATTERN = /^[^\p{Cc}\p{Cs}]{1,128}$/u;
const APPROVAL_STATUS = new Set(["pending", "approved", "revoked"]);

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

export interface DeviceApprovalDocument {
  version: 1;
  user_id: string;
  approved_by_device_id: string;
  approved_at: number;
  device: DeviceDocument;
}

export interface DeviceRevocationDocument {
  version: 1;
  user_id: string;
  revoked_by_device_id: string;
  revoked_at: number;
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

export interface DeviceRow {
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

export interface DeviceRegistrationRequest {
  deviceId: string;
  publicKey: string;
  deviceName: string;
  platform: string;
  idempotencyKey: string;
}

export type DeviceApprovalRequest = { deviceId: string; idempotencyKey: string };
export type DeviceRevocationRequest = { deviceId: string; idempotencyKey: string };

export interface DeviceApprovalRow {
  device_id: unknown;
  requester_device_id: unknown;
  status: unknown;
  decided_at: unknown;
}

export interface DeviceRevocationRow {
  actor_device_id: unknown;
  subject_id: unknown;
  outcome: unknown;
  created_at: unknown;
}

type DeviceRequestBody = Record<string, unknown>;

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

export async function deviceRegistrationRequest(
  request: Request,
): Promise<DeviceRegistrationRequest> {
  const value = await deviceRequestBody(request, "device_registration");
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

export async function deviceApprovalRequest(request: Request): Promise<DeviceApprovalRequest> {
  const value = await deviceRequestBody(request, "device_approval");
  assertOnlyFields(value, ["version", "device_id", "idempotency_key"]);
  if (value.version !== 1) {
    throw new DeviceSchemaError("device_approval_version_invalid");
  }

  return {
    deviceId: deviceIdValue(value.device_id, "device_id"),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
  };
}

export async function deviceRevocationRequest(request: Request): Promise<DeviceRevocationRequest> {
  const value = await deviceRequestBody(request, "device_revocation");
  assertOnlyFields(value, ["version", "device_id", "idempotency_key"]);
  if (value.version !== 1) {
    throw new DeviceSchemaError("device_revocation_version_invalid");
  }

  return {
    deviceId: deviceIdValue(value.device_id, "device_id"),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
  };
}

export function approvedDeviceDocument(
  userId: string,
  approvedByDeviceId: string,
  row: DeviceRow,
): DeviceApprovalDocument {
  const device = deviceDocument(row, approvedByDeviceId);
  if (device.approval_status !== "approved" || device.approved_at === null) {
    throw new DevicePersistenceError("device_approval_missing");
  }
  return {
    version: 1,
    user_id: userId,
    approved_by_device_id: approvedByDeviceId,
    approved_at: device.approved_at,
    device,
  };
}

export function revokedDeviceDocument(
  userId: string,
  revokedByDeviceId: string,
  row: DeviceRow,
): DeviceRevocationDocument {
  const device = deviceDocument(row, revokedByDeviceId);
  if (device.approval_status !== "revoked" || device.revoked_at === null) {
    throw new DevicePersistenceError("device_revocation_missing");
  }
  return {
    version: 1,
    user_id: userId,
    revoked_by_device_id: revokedByDeviceId,
    revoked_at: device.revoked_at,
    device,
  };
}

export function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new DevicePermissionError("device_context_required");
  }
  return context.deviceId;
}

export function deviceDocument(
  row: DeviceRow,
  currentDeviceIdValue: string | undefined,
): DeviceDocument {
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
    current: currentDeviceIdValue === deviceId,
  };
}

export function deviceIdValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

export function timestamp(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
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

function assertOnlyFields(value: DeviceRequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new DeviceSchemaError(`unexpected_field:${field}`);
    }
  }
}

async function deviceRequestBody(request: Request, label: string): Promise<DeviceRequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new DeviceSchemaError(`${label}_json_invalid`);
  }
  if (!isRecord(value)) {
    throw new DeviceSchemaError(`${label}_must_be_object`);
  }
  return value;
}

function isRecord(value: unknown): value is DeviceRequestBody {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
