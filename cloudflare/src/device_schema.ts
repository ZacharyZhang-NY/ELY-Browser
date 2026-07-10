import type { AuthContext } from "./auth.js";
import {
  type WrappedAccountKeyDocument,
  SyncVaultRequestError,
  parseWrappedAccountKey,
} from "./sync_vault.js";

const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const PUBLIC_KEY_PATTERN = /^[a-f0-9]{64}$/;
const SIGNATURE_PATTERN = /^[a-f0-9]{128}$/;
const DEVICE_TEXT_PATTERN = /^[^\p{Cc}\p{Cs}]{1,128}$/u;
const APPROVAL_STATUS = new Set(["pending", "approved", "revoked"]);

export interface DeviceListDocument {
  version: 1;
  user_id: string;
  devices: DeviceDocument[];
}

export interface DeviceRegistrationDocument {
  version: 2;
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

interface DeviceRevocationDocumentBase {
  version: 2;
  user_id: string;
  revoked_by_device_id: string;
  revoked_at: number;
  device: DeviceDocument;
}

export interface ApprovedDeviceRevocationDocument extends DeviceRevocationDocumentBase {
  mode: "approved_rotate";
  key_id: string;
  generation: number;
}

export interface PendingDeviceRevocationDocument extends DeviceRevocationDocumentBase {
  mode: "pending_revoke";
}

export type DeviceRevocationDocument =
  | ApprovedDeviceRevocationDocument
  | PendingDeviceRevocationDocument;

export interface DeviceDocument {
  device_id: string;
  public_key: string;
  wrapping_public_key?: string;
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
  wrapping_public_key?: unknown;
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
  wrappingPublicKey: string;
  registrationProof: string;
  deviceName: string;
  platform: string;
  idempotencyKey: string;
}

export interface DeviceApprovalRequest {
  deviceId: string;
  keyId: string;
  generation: number;
  envelope: WrappedAccountKeyDocument;
  idempotencyKey: string;
  proofCreatedAt: number;
  approvalProof: string;
}
export interface DeviceApprovalRow {
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

export class DeviceConflictError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DeviceConflictError";
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
    "wrapping_public_key",
    "registration_proof",
    "device_name",
    "platform",
    "idempotency_key",
  ]);
  if (value.version !== 2) {
    throw new DeviceSchemaError("device_registration_version_invalid");
  }

  return {
    deviceId: deviceIdValue(value.device_id, "device_id"),
    publicKey: publicKeyValue(value.public_key, "public_key"),
    wrappingPublicKey: publicKeyValue(value.wrapping_public_key, "wrapping_public_key"),
    registrationProof: signatureValue(value.registration_proof, "registration_proof"),
    deviceName: deviceText(value.device_name, "device_name"),
    platform: deviceText(value.platform, "platform"),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
  };
}

export async function deviceApprovalRequest(request: Request): Promise<DeviceApprovalRequest> {
  const value = await deviceRequestBody(request, "device_approval");
  assertOnlyFields(value, [
    "version",
    "device_id",
    "key_id",
    "generation",
    "envelope",
    "idempotency_key",
    "proof_created_at",
    "approval_proof",
  ]);
  if (value.version !== 2) {
    throw new DeviceSchemaError("device_approval_version_invalid");
  }

  return {
    deviceId: deviceIdValue(value.device_id, "device_id"),
    keyId: keyIdValue(value.key_id),
    generation: positiveInteger(value.generation, "generation"),
    envelope: wrappedAccountKey(value.envelope),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
    proofCreatedAt: positiveInteger(value.proof_created_at, "proof_created_at"),
    approvalProof: signatureValue(value.approval_proof, "approval_proof"),
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
  const wrappingPublicKey = optionalPublicKeyValue(
    row.wrapping_public_key,
    "wrapping_public_key",
  );
  return {
    device_id: deviceId,
    public_key: publicKeyValue(row.public_key, "public_key"),
    ...(wrappingPublicKey === undefined
      ? {}
      : { wrapping_public_key: wrappingPublicKey }),
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

export function publicKeyValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !PUBLIC_KEY_PATTERN.test(value)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

export function signatureValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !SIGNATURE_PATTERN.test(value)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

function optionalPublicKeyValue(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  return publicKeyValue(value, label);
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

export function idempotencyKeyValue(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9._:-]{16,128}$/.test(value)) {
    throw new DeviceSchemaError("idempotency_key_invalid");
  }
  return value;
}

export function keyIdValue(value: unknown): string {
  if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) {
    throw new DeviceSchemaError("key_id_invalid");
  }
  return value;
}

export function positiveInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value;
}

export function wrappedAccountKey(value: unknown): WrappedAccountKeyDocument {
  try {
    return parseWrappedAccountKey(value);
  } catch (error) {
    if (error instanceof SyncVaultRequestError) {
      throw new DeviceSchemaError("envelope_invalid");
    }
    throw error;
  }
}

function nullableTimestamp(value: unknown, label: string): number | null {
  if (value === null) {
    return null;
  }
  return timestamp(value, label);
}

export function assertOnlyFields(value: DeviceRequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new DeviceSchemaError(`unexpected_field:${field}`);
    }
  }
}

export async function deviceRequestBody(
  request: Request,
  label: string,
): Promise<DeviceRequestBody> {
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
