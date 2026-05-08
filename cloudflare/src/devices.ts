import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import {
  type DeviceApprovalDocument,
  type DeviceApprovalRequest,
  type DeviceApprovalRow,
  type DeviceListDocument,
  type DeviceRegistrationDocument,
  type DeviceRevocationDocument,
  type DeviceRevocationRequest,
  type DeviceRevocationRow,
  type DeviceRow,
  DevicePermissionError,
  DevicePersistenceError,
  approvedDeviceDocument,
  currentDeviceId,
  deviceApprovalRequest,
  deviceDocument,
  deviceIdValue,
  deviceRegistrationRequest,
  deviceRevocationRequest,
  revokedDeviceDocument,
  timestamp,
} from "./device_schema.js";

export {
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
} from "./device_schema.js";

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
const DEVICE_REVOCATION_BY_IDEMPOTENCY_KEY_QUERY = `
  SELECT
    actor_device_id,
    subject_id,
    outcome,
    created_at
  FROM audit_events
  WHERE user_id = ? AND event_id = ? AND event_type = 'device.revoke'
`;
const DEVICE_REVOCATION_INSERT_QUERY = `
  INSERT INTO audit_events (
    event_id,
    user_id,
    actor_device_id,
    event_type,
    subject_type,
    subject_id,
    outcome,
    metadata_hash,
    created_at
  ) VALUES (?, ?, ?, 'device.revoke', 'device', ?, 'success', NULL, ?)
  ON CONFLICT(event_id) DO NOTHING
`;
const DEVICE_REVOKE_QUERY = `
  UPDATE user_devices
  SET
    approval_status = 'revoked',
    revoked_at = COALESCE(revoked_at, ?)
  WHERE user_id = ? AND device_id = ? AND revoked_at IS NULL
`;

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

  await assertApprovedRequester(env, context.userId, requesterDeviceId);
  const existingApproval = await env.ELY_DB.prepare(DEVICE_APPROVAL_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, approval.idempotencyKey)
    .first<DeviceApprovalRow>();
  if (existingApproval !== null) {
    return existingApprovalDocument(env, context, approval, requesterDeviceId, existingApproval);
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

export async function revokeDeviceDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceRevocationDocument> {
  const revocation = await deviceRevocationRequest(request);
  const requesterDeviceId = currentDeviceId(context);
  if (requesterDeviceId === revocation.deviceId) {
    throw new DevicePermissionError("device_self_revocation_forbidden");
  }

  const revocationEventId = deviceRevocationEventId(context.userId, revocation.idempotencyKey);
  await assertApprovedRequester(env, context.userId, requesterDeviceId);
  const existingRevocation = await env.ELY_DB.prepare(DEVICE_REVOCATION_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, revocationEventId)
    .first<DeviceRevocationRow>();
  if (existingRevocation !== null) {
    return existingRevocationDocument(env, context, revocation, requesterDeviceId, existingRevocation);
  }

  const targetDevice = await deviceRowById(env, context.userId, revocation.deviceId);
  if (targetDevice === null) {
    throw new DevicePermissionError("device_not_found");
  }
  const targetDocument = deviceDocument(targetDevice, requesterDeviceId);
  if (targetDocument.revoked_at !== null) {
    throw new DevicePermissionError("device_already_revoked");
  }

  await env.ELY_DB.batch([
    env.ELY_DB.prepare(DEVICE_REVOCATION_INSERT_QUERY).bind(
      revocationEventId,
      context.userId,
      requesterDeviceId,
      revocation.deviceId,
      nowSeconds,
    ),
    env.ELY_DB.prepare(DEVICE_REVOKE_QUERY).bind(nowSeconds, context.userId, revocation.deviceId),
  ]);

  const revokedDevice = await deviceRowById(env, context.userId, revocation.deviceId);
  if (revokedDevice === null) {
    throw new DevicePersistenceError("device_revocation_missing");
  }
  return revokedDeviceDocument(context.userId, requesterDeviceId, revokedDevice);
}

function deviceRevocationEventId(userId: string, idempotencyKey: string): string {
  return `device-revoke:${userId}:${idempotencyKey}`;
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

async function existingRevocationDocument(
  env: Env,
  context: AuthContext,
  revocation: DeviceRevocationRequest,
  requesterDeviceId: string,
  row: DeviceRevocationRow,
): Promise<DeviceRevocationDocument> {
  const revokedDeviceId = deviceIdValue(row.subject_id, "subject_id");
  const revokedByDeviceId = deviceIdValue(row.actor_device_id, "actor_device_id");
  if (
    revokedDeviceId !== revocation.deviceId ||
    revokedByDeviceId !== requesterDeviceId ||
    row.outcome !== "success"
  ) {
    throw new DevicePermissionError("device_revocation_replay_mismatch");
  }

  const revokedDevice = await deviceRowById(env, context.userId, revokedDeviceId);
  if (revokedDevice === null) {
    throw new DevicePersistenceError("device_revocation_missing");
  }

  return {
    ...revokedDeviceDocument(context.userId, requesterDeviceId, revokedDevice),
    revoked_at: timestamp(row.created_at, "created_at"),
  };
}

async function assertApprovedRequester(
  env: Env,
  userId: string,
  requesterDeviceId: string,
): Promise<void> {
  const requester = await env.ELY_DB.prepare(APPROVED_DEVICE_QUERY)
    .bind(userId, requesterDeviceId)
    .first<DeviceRow>();
  if (requester === null) {
    throw new DevicePermissionError("requester_device_unapproved");
  }
}

async function deviceRowById(env: Env, userId: string, deviceId: string): Promise<DeviceRow | null> {
  return env.ELY_DB.prepare(DEVICE_BY_ID_QUERY).bind(userId, deviceId).first<DeviceRow>();
}
