import type { AuthContext } from "./auth.js";
import type { ElyD1Result, Env } from "./bindings.js";
import { verifyEd25519Signature } from "./device_crypto.js";
import {
  type DeviceRevocationDocument,
  type DeviceRow,
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  currentDeviceId,
  deviceDocument,
  deviceIdValue,
  publicKeyValue,
} from "./device_schema.js";
import {
  type ApprovedDeviceRevocationRequest,
  compareDeviceIds,
  deviceRevocationRequest,
  deviceRevocationRequestHash,
  deviceRevocationProofBytes,
  pendingDeviceRevocationProofBytes,
  pendingDeviceRevocationRequestHash,
} from "./device_revocation_schema.js";
import {
  type RotationResultRow,
  rotationR2ObjectCount,
  rotationResult,
  rotationStatements,
} from "./device_revocation_store.js";
import { revokePendingDeviceDocument } from "./pending_device_revocation.js";

const APPROVED_V2_REQUESTER_QUERY = `
  SELECT device.device_id, keys.signing_public_key
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
`;
const DEVICE_BY_ID_QUERY = `
  SELECT
    device.device_id, device.public_key, device.device_name, device.platform,
    device.approval_status, device.created_at, device.approved_at,
    device.last_active_at, device.revoked_at, keys.wrapping_public_key
  FROM user_devices AS device
  LEFT JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
`;
const CURRENT_VAULT_KEY_QUERY = `
  SELECT current_key_id AS key_id, current_generation AS generation
  FROM sync_vault_accounts
  WHERE user_id = ?
`;
const REMAINING_APPROVED_V2_QUERY = `
  SELECT device.device_id
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id <> ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
  ORDER BY device.device_id ASC
`;
interface VaultKeyRow { key_id: unknown; generation: unknown }
interface DeviceIdRow { device_id: unknown }
interface ApprovedV2RequesterRow extends DeviceIdRow { signing_public_key: unknown }

export async function revokeDeviceDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceRevocationDocument> {
  const revocation = await deviceRevocationRequest(request);
  const approverDeviceId = currentDeviceId(context);
  if (approverDeviceId === revocation.deviceId) {
    throw new DevicePermissionError("device_self_revocation_forbidden");
  }
  const signingPublicKey = await approvedV2RequesterKey(env, context.userId, approverDeviceId);
  if (revocation.mode === "pending_revoke") {
    const { pendingRevocationProof, ...unsignedRevocation } = revocation;
    if (!(await verifyEd25519Signature(
      signingPublicKey,
      pendingRevocationProof,
      pendingDeviceRevocationProofBytes(context.userId, approverDeviceId, unsignedRevocation),
    ))) {
      throw new DevicePermissionError("device_revocation_proof_invalid");
    }
    const requestHash = await pendingDeviceRevocationRequestHash(
      context.userId,
      approverDeviceId,
      revocation,
    );
    return revokePendingDeviceDocument(
      env,
      context,
      approverDeviceId,
      revocation,
      requestHash,
      nowSeconds,
    );
  }
  const { rotationProof, ...unsignedRevocation } = revocation;
  if (!(await verifyEd25519Signature(
    signingPublicKey,
    rotationProof,
    deviceRevocationProofBytes(context.userId, approverDeviceId, unsignedRevocation),
  ))) {
    throw new DevicePermissionError("device_revocation_proof_invalid");
  }

  const requestHash = await deviceRevocationRequestHash(
    context.userId,
    approverDeviceId,
    revocation,
  );
  const existing = await rotationResult(env, context.userId, revocation.idempotencyKey);
  if (existing !== null) {
    return completedRevocationDocument(
      env,
      context.userId,
      approverDeviceId,
      revocation,
      requestHash,
      existing,
    );
  }

  await assertCurrentVaultKey(env, context.userId, revocation);
  await assertRevocableTarget(env, context.userId, approverDeviceId, revocation.deviceId);
  const recipients = await remainingApprovedV2Recipients(env, context.userId, revocation.deviceId);
  assertExactRecipients(revocation, recipients);
  const r2ObjectCount = await rotationR2ObjectCount(env, context.userId);
  const statements = await rotationStatements(
    env,
    context.userId,
    approverDeviceId,
    revocation,
    requestHash,
    r2ObjectCount,
    nowSeconds,
  );
  let results: ElyD1Result[];
  try {
    results = await env.ELY_DB.batch<ElyD1Result>(statements);
  } catch (error) {
    if (isRotationConflict(error)) {
      throw new DeviceConflictError("device_revocation_race");
    }
    throw error;
  }
  const finalizeChanges = changedRowCount(results.at(-1), "device_revocation_finalize");
  if (finalizeChanges > 1) {
    throw new DevicePersistenceError("device_revocation_write_count_invalid");
  }

  const completed = await rotationResult(env, context.userId, revocation.idempotencyKey);
  if (completed === null) {
    throw new DeviceConflictError("device_revocation_race");
  }
  try {
    return await completedRevocationDocument(
      env,
      context.userId,
      approverDeviceId,
      revocation,
      requestHash,
      completed,
    );
  } catch (error) {
    if (finalizeChanges === 0 && error instanceof DeviceConflictError) {
      throw new DeviceConflictError("device_revocation_race");
    }
    throw error;
  }
}

async function completedRevocationDocument(
  env: Env,
  userId: string,
  approverDeviceId: string,
  revocation: ApprovedDeviceRevocationRequest,
  requestHash: string,
  result: RotationResultRow,
): Promise<DeviceRevocationDocument> {
  assertRotationResult(result, revocation, approverDeviceId, requestHash);
  const revokedDevice = await deviceRowById(env, userId, revocation.deviceId);
  if (revokedDevice === null) {
    throw new DevicePersistenceError("device_revocation_missing");
  }
  const device = deviceDocument(revokedDevice, approverDeviceId);
  const completedAt = storedInteger(result.completed_at, "completed_at");
  if (device.approval_status !== "revoked" || device.revoked_at !== completedAt) {
    throw new DevicePersistenceError("device_revocation_state_invalid");
  }
  return {
    version: 2,
    mode: "approved_rotate",
    user_id: userId,
    revoked_by_device_id: approverDeviceId,
    revoked_at: completedAt,
    key_id: revocation.newKeyId,
    generation: revocation.newGeneration,
    device,
  };
}

function assertRotationResult(
  row: RotationResultRow,
  revocation: ApprovedDeviceRevocationRequest,
  approverDeviceId: string,
  requestHash: string,
): void {
  if (
    row.target_device_id !== revocation.deviceId ||
    row.approver_device_id !== approverDeviceId ||
    row.previous_key_id !== revocation.previousKeyId ||
    row.previous_generation !== revocation.previousGeneration ||
    row.new_key_id !== revocation.newKeyId ||
    row.new_generation !== revocation.newGeneration ||
    row.request_hash !== requestHash ||
    row.envelope_count !== revocation.envelopes.length
  ) {
    throw new DeviceConflictError("device_revocation_replay_mismatch");
  }
  const completedAt = storedInteger(row.completed_at, "completed_at");
  if (
    row.current_key_id !== revocation.newKeyId ||
    row.current_generation !== revocation.newGeneration ||
    row.target_status !== "revoked" ||
    row.revoked_at !== completedAt ||
    row.active_session_count !== 0 ||
    row.item_count !== revocation.envelopes.length ||
    storedInteger(row.r2_object_count, "r2_object_count") !==
      storedInteger(row.r2_item_count, "r2_item_count") ||
    row.persisted_count !== revocation.envelopes.length ||
    row.audit_count !== 1
  ) {
    throw new DevicePersistenceError("device_revocation_result_invalid");
  }
}

async function approvedV2RequesterKey(
  env: Env,
  userId: string,
  approverDeviceId: string,
): Promise<string> {
  const row = await env.ELY_DB.prepare(APPROVED_V2_REQUESTER_QUERY)
    .bind(userId, approverDeviceId)
    .first<ApprovedV2RequesterRow>();
  if (row === null) {
    throw new DevicePermissionError("requester_device_unapproved");
  }
  return publicKeyValue(row.signing_public_key, "signing_public_key");
}

async function assertCurrentVaultKey(
  env: Env,
  userId: string,
  revocation: ApprovedDeviceRevocationRequest,
): Promise<void> {
  const row = await env.ELY_DB.prepare(CURRENT_VAULT_KEY_QUERY).bind(userId).first<VaultKeyRow>();
  if (
    row === null ||
    row.key_id !== revocation.previousKeyId ||
    row.generation !== revocation.previousGeneration
  ) {
    throw new DeviceConflictError("device_revocation_vault_conflict");
  }
}

async function assertRevocableTarget(
  env: Env,
  userId: string,
  approverDeviceId: string,
  targetDeviceId: string,
): Promise<void> {
  const row = await deviceRowById(env, userId, targetDeviceId);
  if (row === null) {
    throw new DevicePermissionError("device_not_found");
  }
  const device = deviceDocument(row, approverDeviceId);
  if (device.revoked_at !== null || device.approval_status !== "approved") {
    throw new DevicePermissionError("device_not_revocable");
  }
}

async function remainingApprovedV2Recipients(
  env: Env,
  userId: string,
  targetDeviceId: string,
): Promise<string[]> {
  const result = await env.ELY_DB.prepare(REMAINING_APPROVED_V2_QUERY)
    .bind(userId, targetDeviceId)
    .all<DeviceIdRow>();
  const recipients = result.results.map((row) => deviceIdValue(row.device_id, "device_id"));
  if (new Set(recipients).size !== recipients.length) {
    throw new DevicePersistenceError("device_revocation_recipient_rows_invalid");
  }
  recipients.sort(compareDeviceIds);
  return recipients;
}

function assertExactRecipients(revocation: ApprovedDeviceRevocationRequest, expected: string[]): void {
  const actual = revocation.envelopes.map((item) => item.recipientDeviceId);
  if (actual.length !== expected.length || actual.some((deviceId, index) => deviceId !== expected[index])) {
    throw new DeviceConflictError("device_revocation_envelope_set_mismatch");
  }
}

function deviceRowById(env: Env, userId: string, deviceId: string): Promise<DeviceRow | null> {
  return env.ELY_DB.prepare(DEVICE_BY_ID_QUERY).bind(userId, deviceId).first<DeviceRow>();
}

function storedInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new DevicePersistenceError(`${label}_invalid`);
  }
  return value;
}

function changedRowCount(result: ElyD1Result | undefined, label: string): number {
  const changes = result?.meta?.changes;
  if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes < 0) {
    throw new DevicePersistenceError(`${label}_write_result_invalid`);
  }
  return changes;
}

function isRotationConflict(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return error.message.includes("sync_vault_rotation_guard_failed") ||
    error.message.includes("UNIQUE constraint failed: sync_vault_envelopes") ||
    error.message.includes("FOREIGN KEY constraint failed");
}
