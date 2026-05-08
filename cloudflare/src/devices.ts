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
const DEVICE_BY_ID_QUERY = `
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
  WHERE user_id = ? AND device_id = ?
`;
const APPROVED_DEVICE_QUERY = `
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
  WHERE user_id = ? AND device_id = ? AND approval_status = 'approved' AND revoked_at IS NULL
`;
const DEVICE_APPROVAL_BY_IDEMPOTENCY_KEY_QUERY = `
  SELECT
    device_id,
    requester_device_id,
    status,
    decided_at
  FROM device_approvals
  WHERE user_id = ? AND idempotency_key = ?
`;
const DEVICE_APPROVAL_INSERT_QUERY = `
  INSERT INTO device_approvals (
    user_id,
    approval_id,
    device_id,
    requester_device_id,
    status,
    requested_at,
    decided_at,
    expires_at,
    idempotency_key
  ) VALUES (?, ?, ?, ?, 'approved', ?, ?, ?, ?)
  ON CONFLICT(user_id, idempotency_key) DO NOTHING
`;
const DEVICE_APPROVE_QUERY = `
  UPDATE user_devices
  SET
    approval_status = 'approved',
    approved_at = COALESCE(approved_at, ?),
    last_active_at = ?
  WHERE user_id = ? AND device_id = ? AND approval_status = 'pending' AND revoked_at IS NULL
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

export interface DeviceApprovalDocument {
  version: 1;
  user_id: string;
  approved_by_device_id: string;
  approved_at: number;
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

type DeviceApprovalRequest = { deviceId: string; idempotencyKey: string };

interface DeviceApprovalRow {
  device_id: unknown;
  requester_device_id: unknown;
  status: unknown;
  decided_at: unknown;
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

export async function approveDeviceDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceApprovalDocument> {
  const approval = await deviceApprovalRequest(request);
  const requesterDeviceId = currentDeviceId(context);
  if (requesterDeviceId === approval.deviceId) {
    throw new DevicePermissionError("device_self_approval_forbidden");
  }

  const existingApproval = await env.ELY_DB.prepare(DEVICE_APPROVAL_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, approval.idempotencyKey)
    .first<DeviceApprovalRow>();
  if (existingApproval !== null) {
    return existingApprovalDocument(env, context, approval, requesterDeviceId, existingApproval);
  }

  const approver = await env.ELY_DB.prepare(APPROVED_DEVICE_QUERY)
    .bind(context.userId, requesterDeviceId)
    .first<DeviceRow>();
  if (approver === null) {
    throw new DevicePermissionError("approver_device_unapproved");
  }

  const pendingDevice = await deviceRowById(env, context.userId, approval.deviceId);
  if (pendingDevice === null) {
    throw new DevicePermissionError("device_not_found");
  }
  const pendingDocument = deviceDocument(pendingDevice, requesterDeviceId);
  if (pendingDocument.approval_status !== "pending" || pendingDocument.revoked_at !== null) {
    throw new DevicePermissionError("device_not_pending");
  }

  await env.ELY_DB.batch([
    env.ELY_DB.prepare(DEVICE_APPROVAL_INSERT_QUERY).bind(
      context.userId,
      approval.idempotencyKey,
      approval.deviceId,
      requesterDeviceId,
      nowSeconds,
      nowSeconds,
      nowSeconds,
      approval.idempotencyKey,
    ),
    env.ELY_DB.prepare(DEVICE_APPROVE_QUERY).bind(
      nowSeconds,
      nowSeconds,
      context.userId,
      approval.deviceId,
    ),
  ]);

  const approvedDevice = await deviceRowById(env, context.userId, approval.deviceId);
  if (approvedDevice === null) {
    throw new DevicePersistenceError("device_approval_missing");
  }
  return approvedDeviceDocument(context.userId, requesterDeviceId, approvedDevice);
}

async function deviceRegistrationRequest(request: Request): Promise<DeviceRegistrationRequest> {
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

async function deviceApprovalRequest(request: Request): Promise<DeviceApprovalRequest> {
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

async function existingApprovalDocument(
  env: Env,
  context: AuthContext,
  approval: DeviceApprovalRequest,
  requesterDeviceId: string,
  row: DeviceApprovalRow,
): Promise<DeviceApprovalDocument> {
  const approvedDeviceId = deviceIdValue(row.device_id, "device_id");
  const approvedByDeviceId = deviceIdValue(row.requester_device_id, "requester_device_id");
  if (
    approvedDeviceId !== approval.deviceId ||
    approvedByDeviceId !== requesterDeviceId ||
    row.status !== "approved"
  ) {
    throw new DevicePermissionError("device_approval_replay_mismatch");
  }

  const approvedDevice = await deviceRowById(env, context.userId, approvedDeviceId);
  if (approvedDevice === null) {
    throw new DevicePersistenceError("device_approval_missing");
  }

  return {
    ...approvedDeviceDocument(context.userId, requesterDeviceId, approvedDevice),
    approved_at: timestamp(row.decided_at, "decided_at"),
  };
}

function approvedDeviceDocument(
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

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new DevicePermissionError("device_context_required");
  }
  return context.deviceId;
}

async function deviceRowById(env: Env, userId: string, deviceId: string): Promise<DeviceRow | null> {
  return env.ELY_DB.prepare(DEVICE_BY_ID_QUERY).bind(userId, deviceId).first<DeviceRow>();
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

function assertOnlyFields(value: DeviceRequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new DeviceSchemaError(`unexpected_field:${field}`);
    }
  }
}

function isRecord(value: unknown): value is DeviceRequestBody {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
