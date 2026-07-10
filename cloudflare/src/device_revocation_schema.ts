import {
  DeviceSchemaError,
  assertOnlyFields,
  deviceIdValue,
  deviceRequestBody,
  idempotencyKeyValue,
  keyIdValue,
  positiveInteger,
  signatureValue,
  wrappedAccountKey,
} from "./device_schema.js";
import type { WrappedAccountKeyDocument } from "./sync_vault.js";

const MAX_ROTATION_ENVELOPES = 128;

export function compareDeviceIds(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

export interface DeviceRevocationEnvelopeRequest {
  recipientDeviceId: string;
  envelope: WrappedAccountKeyDocument;
}

interface DeviceRevocationBaseRequest {
  deviceId: string;
  idempotencyKey: string;
}

export interface ApprovedDeviceRevocationRequest extends DeviceRevocationBaseRequest {
  mode: "approved_rotate";
  previousKeyId: string;
  previousGeneration: number;
  newKeyId: string;
  newGeneration: number;
  envelopes: DeviceRevocationEnvelopeRequest[];
  rotationProof: string;
}

export interface PendingDeviceRevocationRequest extends DeviceRevocationBaseRequest {
  mode: "pending_revoke";
  pendingRevocationProof: string;
}

export type DeviceRevocationRequest =
  | ApprovedDeviceRevocationRequest
  | PendingDeviceRevocationRequest;

export async function deviceRevocationRequest(request: Request): Promise<DeviceRevocationRequest> {
  const value = await deviceRequestBody(request, "device_revocation");
  if (value.version !== 2) {
    throw new DeviceSchemaError("device_revocation_version_invalid");
  }
  if (value.mode === "pending_revoke") {
    assertOnlyFields(value, [
      "version",
      "mode",
      "device_id",
      "idempotency_key",
      "pending_revocation_proof",
    ]);
    return {
      mode: "pending_revoke",
      deviceId: deviceIdValue(value.device_id, "device_id"),
      idempotencyKey: idempotencyKeyValue(value.idempotency_key),
      pendingRevocationProof: signatureValue(
        value.pending_revocation_proof,
        "pending_revocation_proof",
      ),
    };
  }
  if (value.mode !== "approved_rotate") {
    throw new DeviceSchemaError("device_revocation_mode_invalid");
  }
  assertOnlyFields(value, [
    "version",
    "mode",
    "device_id",
    "previous_key_id",
    "previous_generation",
    "new_key_id",
    "new_generation",
    "envelopes",
    "idempotency_key",
    "rotation_proof",
  ]);
  const deviceId = deviceIdValue(value.device_id, "device_id");
  const previousKeyId = keyIdValue(value.previous_key_id);
  const previousGeneration = positiveInteger(value.previous_generation, "previous_generation");
  const newKeyId = keyIdValue(value.new_key_id);
  const newGeneration = positiveInteger(value.new_generation, "new_generation");
  if (
    previousGeneration === Number.MAX_SAFE_INTEGER ||
    newGeneration !== previousGeneration + 1
  ) {
    throw new DeviceSchemaError("new_generation_invalid");
  }
  if (newKeyId === previousKeyId) {
    throw new DeviceSchemaError("new_key_id_invalid");
  }

  return {
    mode: "approved_rotate",
    deviceId,
    previousKeyId,
    previousGeneration,
    newKeyId,
    newGeneration,
    envelopes: revocationEnvelopes(value.envelopes, deviceId),
    idempotencyKey: idempotencyKeyValue(value.idempotency_key),
    rotationProof: signatureValue(value.rotation_proof, "rotation_proof"),
  };
}

export async function deviceRevocationRequestHash(
  userId: string,
  approverDeviceId: string,
  revocation: ApprovedDeviceRevocationRequest,
): Promise<string> {
  return sha256Hex(JSON.stringify({
    version: 2,
    mode: revocation.mode,
    user_id: userId,
    approver_device_id: approverDeviceId,
    device_id: revocation.deviceId,
    previous_key_id: revocation.previousKeyId,
    previous_generation: revocation.previousGeneration,
    new_key_id: revocation.newKeyId,
    new_generation: revocation.newGeneration,
    envelopes: revocation.envelopes.map((item) => ({
      recipient_device_id: item.recipientDeviceId,
      envelope: item.envelope,
    })),
    idempotency_key: revocation.idempotencyKey,
    rotation_proof: revocation.rotationProof,
  }));
}

export function deviceRevocationProofBytes(
  userId: string,
  approverDeviceId: string,
  revocation: Omit<ApprovedDeviceRevocationRequest, "rotationProof">,
): Uint8Array {
  const values: (number | string)[] = [
    "elydora-device-revocation-v2",
    userId,
    approverDeviceId,
    revocation.deviceId,
    revocation.previousKeyId,
    revocation.previousGeneration,
    revocation.newKeyId,
    revocation.newGeneration,
    revocation.idempotencyKey,
    revocation.envelopes.length,
  ];
  for (const item of revocation.envelopes) {
    values.push(
      item.recipientDeviceId,
      item.envelope.version,
      item.envelope.suite,
      item.envelope.encapped_key,
      item.envelope.ciphertext,
    );
  }
  return canonicalBytes(values);
}

export async function pendingDeviceRevocationRequestHash(
  userId: string,
  approverDeviceId: string,
  revocation: PendingDeviceRevocationRequest,
): Promise<string> {
  return sha256Hex(JSON.stringify({
    version: 2,
    mode: revocation.mode,
    user_id: userId,
    approver_device_id: approverDeviceId,
    device_id: revocation.deviceId,
    idempotency_key: revocation.idempotencyKey,
    pending_revocation_proof: revocation.pendingRevocationProof,
  }));
}

export function pendingDeviceRevocationProofBytes(
  userId: string,
  approverDeviceId: string,
  revocation: Omit<PendingDeviceRevocationRequest, "pendingRevocationProof">,
): Uint8Array {
  return canonicalBytes([
    "elydora-pending-device-revocation-v2",
    userId,
    approverDeviceId,
    revocation.deviceId,
    revocation.idempotencyKey,
  ]);
}

export function rotationEnvelopeIdempotencyKey(
  userId: string,
  rotationIdempotencyKey: string,
  recipientDeviceId: string,
): Promise<string> {
  const encoder = new TextEncoder();
  return sha256Hex([userId, rotationIdempotencyKey, recipientDeviceId]
    .map((value) => `${encoder.encode(value).byteLength}:${value}`)
    .join(""));
}

function revocationEnvelopes(value: unknown, targetDeviceId: string): DeviceRevocationEnvelopeRequest[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_ROTATION_ENVELOPES) {
    throw new DeviceSchemaError("envelopes_invalid");
  }
  const seen = new Set<string>();
  const envelopes = value.map((item, index) => {
    const record = requestRecord(item, `envelopes[${index}]`);
    assertOnlyFields(record, ["recipient_device_id", "envelope"]);
    const recipientDeviceId = deviceIdValue(
      record.recipient_device_id,
      `envelopes[${index}].recipient_device_id`,
    );
    if (recipientDeviceId === targetDeviceId || seen.has(recipientDeviceId)) {
      throw new DeviceSchemaError("envelope_recipient_invalid");
    }
    seen.add(recipientDeviceId);
    return {
      recipientDeviceId,
      envelope: wrappedAccountKey(record.envelope),
    };
  });
  envelopes.sort((left, right) => compareDeviceIds(left.recipientDeviceId, right.recipientDeviceId));
  return envelopes;
}

function requestRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new DeviceSchemaError(`${label}_invalid`);
  }
  return value as Record<string, unknown>;
}

async function sha256Hex(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function canonicalBytes(values: (number | string)[]): Uint8Array {
  const encoder = new TextEncoder();
  return encoder.encode(values.map((value) => {
    const text = value.toString();
    return `${encoder.encode(text).byteLength}:${text}`;
  }).join(""));
}
