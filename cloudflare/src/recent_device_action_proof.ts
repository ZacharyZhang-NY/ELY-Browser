import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession } from "./bindings.js";
import { verifyEd25519Signature } from "./device_crypto.js";

const ACTION_PROOF_DOMAIN = "elydora-sensitive-action-v2";
const PROOF_MAX_AGE_SECONDS = 5 * 60;
const PROOF_CLOCK_SKEW_SECONDS = 30;
const PUBLIC_KEY_PATTERN = /^[a-f0-9]{64}$/;
const SIGNATURE_PATTERN = /^[a-f0-9]{128}$/;

const APPROVED_DEVICE_SIGNING_KEY_QUERY = `
  SELECT keys.signing_public_key
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
`;

export type SensitiveAction = "account.delete" | "sync.reset";

export interface RecentDeviceActionProof {
  proofCreatedAt: number;
  actionProof: string;
}

export interface RecentDeviceActionProofFields extends RecentDeviceActionProof {
  action: SensitiveAction;
  userId: string;
  sessionId: string;
  deviceId: string;
  confirmation: string;
  idempotencyKey: string;
}

interface SigningKeyRow { signing_public_key: unknown }

export class RecentDeviceActionRequestError extends Error {}
export class RecentDeviceActionPermissionError extends Error {}
export class RecentDeviceActionPersistenceError extends Error {}

export function recentDeviceActionProof(
  proofCreatedAt: unknown,
  actionProof: unknown,
): RecentDeviceActionProof {
  if (
    typeof proofCreatedAt !== "number" ||
    !Number.isSafeInteger(proofCreatedAt) ||
    proofCreatedAt < 1
  ) {
    throw new RecentDeviceActionRequestError("proof_created_at_invalid");
  }
  if (typeof actionProof !== "string" || !SIGNATURE_PATTERN.test(actionProof)) {
    throw new RecentDeviceActionRequestError("action_proof_invalid");
  }
  return { proofCreatedAt, actionProof };
}

export async function assertRecentDeviceActionProof(
  database: ElyD1DatabaseSession,
  context: AuthContext,
  action: SensitiveAction,
  confirmation: string,
  idempotencyKey: string,
  proof: RecentDeviceActionProof,
): Promise<string> {
  if (context.deviceId === undefined) {
    throw new RecentDeviceActionPermissionError("device_context_required");
  }
  const row = await database.prepare(APPROVED_DEVICE_SIGNING_KEY_QUERY)
    .bind(context.userId, context.deviceId)
    .first<SigningKeyRow>();
  if (row === null) {
    throw new RecentDeviceActionPermissionError("device_action_forbidden");
  }
  if (
    typeof row.signing_public_key !== "string" ||
    !PUBLIC_KEY_PATTERN.test(row.signing_public_key)
  ) {
    throw new RecentDeviceActionPersistenceError("device_signing_key_invalid");
  }
  const fields: RecentDeviceActionProofFields = {
    action,
    userId: context.userId,
    sessionId: context.sessionId,
    deviceId: context.deviceId,
    confirmation,
    idempotencyKey,
    ...proof,
  };
  if (!(await verifyEd25519Signature(
    row.signing_public_key,
    proof.actionProof,
    recentDeviceActionProofBytes(fields),
  ))) {
    throw new RecentDeviceActionPermissionError("device_action_proof_invalid");
  }
  return row.signing_public_key;
}

export function assertFreshDeviceActionProof(
  proof: RecentDeviceActionProof,
  nowSeconds: number,
  freshnessRequired: boolean,
): void {
  if (freshnessRequired && (
    proof.proofCreatedAt < nowSeconds - PROOF_MAX_AGE_SECONDS ||
    proof.proofCreatedAt > nowSeconds + PROOF_CLOCK_SKEW_SECONDS
  )) {
    throw new RecentDeviceActionPermissionError("device_action_proof_expired");
  }
}

export function recentDeviceActionProofBytes(
  fields: Omit<RecentDeviceActionProofFields, "actionProof">,
): Uint8Array {
  return canonicalBytes(deviceActionProofValues(fields));
}

export async function recentDeviceActionRequestHash(
  fields: RecentDeviceActionProofFields,
): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    canonicalBytes([...deviceActionProofValues(fields), fields.actionProof]),
  );
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function deviceActionProofValues(
  fields: Omit<RecentDeviceActionProofFields, "actionProof">,
): (number | string)[] {
  return [
    ACTION_PROOF_DOMAIN,
    fields.action,
    fields.userId,
    fields.sessionId,
    fields.deviceId,
    fields.confirmation,
    fields.idempotencyKey,
    fields.proofCreatedAt,
  ];
}

function canonicalBytes(values: (number | string)[]): Uint8Array {
  const encoder = new TextEncoder();
  return encoder.encode(values.map((value) => {
    const text = value.toString();
    return `${encoder.encode(text).byteLength}:${text}`;
  }).join(""));
}
