import { verifyEd25519Signature } from "./device_crypto.js";
import {
  type DeviceApprovalRequest,
  DevicePermissionError,
} from "./device_schema.js";

const PROOF_MAX_AGE_SECONDS = 5 * 60;
const PROOF_CLOCK_SKEW_SECONDS = 30;

export async function assertDeviceApprovalProof(
  signingPublicKey: string,
  userId: string,
  approverDeviceId: string,
  approval: DeviceApprovalRequest,
): Promise<void> {
  if (!(await verifyEd25519Signature(
    signingPublicKey,
    approval.approvalProof,
    deviceApprovalProofBytes(userId, approverDeviceId, approval),
  ))) {
    throw new DevicePermissionError("device_approval_proof_invalid");
  }
}

export function assertFreshDeviceApprovalProof(
  approval: DeviceApprovalRequest,
  nowSeconds: number,
  required: boolean,
): void {
  if (required && (
    approval.proofCreatedAt < nowSeconds - PROOF_MAX_AGE_SECONDS ||
    approval.proofCreatedAt > nowSeconds + PROOF_CLOCK_SKEW_SECONDS
  )) {
    throw new DevicePermissionError("device_approval_proof_expired");
  }
}

export function deviceApprovalProofBytes(
  userId: string,
  approverDeviceId: string,
  approval: Omit<DeviceApprovalRequest, "approvalProof">,
): Uint8Array {
  return canonicalBytes([
    "elydora-device-approval-v2",
    userId,
    approverDeviceId,
    approval.deviceId,
    approval.keyId,
    approval.generation,
    approval.envelope.version,
    approval.envelope.suite,
    approval.envelope.encapped_key,
    approval.envelope.ciphertext,
    approval.idempotencyKey,
    approval.proofCreatedAt,
  ]);
}

function canonicalBytes(values: (number | string)[]): Uint8Array {
  const encoder = new TextEncoder();
  return encoder.encode(values.map((value) => {
    const text = value.toString();
    return `${encoder.encode(text).byteLength}:${text}`;
  }).join(""));
}
