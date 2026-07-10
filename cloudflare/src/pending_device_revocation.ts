import type { AuthContext } from "./auth.js";
import type { ElyD1Result, Env } from "./bindings.js";
import type { PendingDeviceRevocationRequest } from "./device_revocation_schema.js";
import {
  type DeviceRevocationDocument,
  type DeviceRow,
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  deviceDocument,
} from "./device_schema.js";

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
const PENDING_RESULT_QUERY = `
  SELECT
    revocation.target_device_id, revocation.approver_device_id,
    revocation.request_hash, revocation.completed_at,
    target.approval_status AS target_status, target.revoked_at,
    (SELECT COUNT(*)
      FROM better_auth_session AS session
      INNER JOIN better_auth_session_device_context AS context
        ON context.session_id = session.id
      WHERE context.user_id = revocation.user_id
        AND context.device_id = revocation.target_device_id) AS active_session_count,
    (SELECT COUNT(*) FROM audit_events AS audit
      WHERE audit.event_id = revocation.audit_event_id
        AND audit.user_id = revocation.user_id
        AND audit.actor_device_id = revocation.approver_device_id
        AND audit.event_type = 'device.revoke'
        AND audit.subject_id = revocation.target_device_id
        AND audit.outcome = 'success'
        AND audit.metadata_hash = revocation.request_hash
        AND audit.created_at = revocation.completed_at) AS audit_count
  FROM pending_device_revocations AS revocation
  LEFT JOIN user_devices AS target
    ON target.user_id = revocation.user_id AND target.device_id = revocation.target_device_id
  WHERE revocation.user_id = ? AND revocation.idempotency_key = ?
`;
const PENDING_INSERT_QUERY = `
  INSERT INTO pending_device_revocations (
    user_id, idempotency_key, audit_event_id, target_device_id,
    approver_device_id, request_hash, created_at, completed_at
  ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
  ON CONFLICT(user_id, idempotency_key) DO NOTHING
`;
const PENDING_FINALIZE_QUERY = `
  UPDATE pending_device_revocations
  SET completed_at = ?
  WHERE user_id = ? AND idempotency_key = ? AND completed_at IS NULL
    AND target_device_id = ? AND approver_device_id = ? AND request_hash = ?
`;

interface PendingResultRow {
  target_device_id: unknown;
  approver_device_id: unknown;
  request_hash: unknown;
  completed_at: unknown;
  target_status: unknown;
  revoked_at: unknown;
  active_session_count: unknown;
  audit_count: unknown;
}

export async function revokePendingDeviceDocument(
  env: Env,
  context: AuthContext,
  approverDeviceId: string,
  revocation: PendingDeviceRevocationRequest,
  requestHash: string,
  nowSeconds: number,
): Promise<DeviceRevocationDocument> {
  const existing = await pendingResult(env, context.userId, revocation.idempotencyKey);
  if (existing !== null) {
    return completedPendingDocument(
      env,
      context.userId,
      approverDeviceId,
      revocation,
      requestHash,
      existing,
    );
  }
  const target = await deviceRowById(env, context.userId, revocation.deviceId);
  if (target === null) {
    throw new DevicePermissionError("device_not_found");
  }
  const targetDocument = deviceDocument(target, approverDeviceId);
  if (targetDocument.approval_status !== "pending" || targetDocument.revoked_at !== null) {
    throw new DeviceConflictError("pending_device_revocation_target_invalid");
  }
  let results: ElyD1Result[];
  try {
    results = await env.ELY_DB.batch<ElyD1Result>([
      env.ELY_DB.prepare(PENDING_INSERT_QUERY).bind(
        context.userId,
        revocation.idempotencyKey,
        `pending-device-revoke:${requestHash}`,
        revocation.deviceId,
        approverDeviceId,
        requestHash,
        nowSeconds,
      ),
      env.ELY_DB.prepare(PENDING_FINALIZE_QUERY).bind(
        nowSeconds,
        context.userId,
        revocation.idempotencyKey,
        revocation.deviceId,
        approverDeviceId,
        requestHash,
      ),
    ]);
  } catch (error) {
    if (pendingConflict(error)) {
      throw new DeviceConflictError("pending_device_revocation_race");
    }
    throw error;
  }
  const finalizeChanges = changedRowCount(results.at(-1));
  if (finalizeChanges > 1) {
    throw new DevicePersistenceError("pending_device_revocation_write_count_invalid");
  }
  const completed = await pendingResult(env, context.userId, revocation.idempotencyKey);
  if (completed === null) {
    throw new DeviceConflictError("pending_device_revocation_race");
  }
  try {
    return await completedPendingDocument(
      env,
      context.userId,
      approverDeviceId,
      revocation,
      requestHash,
      completed,
    );
  } catch (error) {
    if (finalizeChanges === 0 && error instanceof DeviceConflictError) {
      throw new DeviceConflictError("pending_device_revocation_race");
    }
    throw error;
  }
}

async function completedPendingDocument(
  env: Env,
  userId: string,
  approverDeviceId: string,
  revocation: PendingDeviceRevocationRequest,
  requestHash: string,
  result: PendingResultRow,
): Promise<DeviceRevocationDocument> {
  if (
    result.target_device_id !== revocation.deviceId ||
    result.approver_device_id !== approverDeviceId ||
    result.request_hash !== requestHash
  ) {
    throw new DeviceConflictError("pending_device_revocation_replay_mismatch");
  }
  const completedAt = storedInteger(result.completed_at, "completed_at");
  if (
    result.target_status !== "revoked" ||
    result.revoked_at !== completedAt ||
    result.active_session_count !== 0 ||
    result.audit_count !== 1
  ) {
    throw new DevicePersistenceError("pending_device_revocation_result_invalid");
  }
  const row = await deviceRowById(env, userId, revocation.deviceId);
  if (row === null) {
    throw new DevicePersistenceError("pending_device_revocation_missing");
  }
  const device = deviceDocument(row, approverDeviceId);
  if (device.approval_status !== "revoked" || device.revoked_at !== completedAt) {
    throw new DevicePersistenceError("pending_device_revocation_state_invalid");
  }
  return {
    version: 2,
    mode: "pending_revoke",
    user_id: userId,
    revoked_by_device_id: approverDeviceId,
    revoked_at: completedAt,
    device,
  };
}

function pendingResult(
  env: Env,
  userId: string,
  idempotencyKey: string,
): Promise<PendingResultRow | null> {
  return env.ELY_DB.prepare(PENDING_RESULT_QUERY)
    .bind(userId, idempotencyKey)
    .first<PendingResultRow>();
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

function changedRowCount(result: ElyD1Result | undefined): number {
  const changes = result?.meta?.changes;
  if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes < 0) {
    throw new DevicePersistenceError("pending_device_revocation_write_result_invalid");
  }
  return changes;
}

function pendingConflict(error: unknown): boolean {
  return error instanceof Error && (
    error.message.includes("pending_device_revocation_guard_failed") ||
    error.message.includes("FOREIGN KEY constraint failed")
  );
}
