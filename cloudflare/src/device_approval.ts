import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import {
  assertDeviceApprovalProof,
  assertFreshDeviceApprovalProof,
} from "./device_approval_proof.js";
import {
  type DeviceApprovalDocument,
  type DeviceApprovalRequest,
  type DeviceApprovalRow,
  type DeviceRow,
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
  approvedDeviceDocument,
  currentDeviceId,
  deviceApprovalRequest,
  deviceDocument,
  deviceIdValue,
  idempotencyKeyValue,
  keyIdValue,
  positiveInteger,
  publicKeyValue,
  timestamp,
  wrappedAccountKey,
} from "./device_schema.js";
import { syncVaultRecipientEnvelopeStatement } from "./sync_vault.js";

const STORED_APPROVAL_STATUSES = new Set(["pending", "approved", "rejected", "expired"]);
const APPROVED_REQUESTER_QUERY = `
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
    device.device_id,
    device.public_key,
    device.device_name,
    device.platform,
    device.approval_status,
    device.created_at,
    device.approved_at,
    device.last_active_at,
    device.revoked_at,
    keys.wrapping_public_key
  FROM user_devices AS device
  LEFT JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
`;
const DEVICE_APPROVAL_BY_IDEMPOTENCY_KEY_QUERY = `
  SELECT device_id, requester_device_id, status, decided_at
  FROM device_approvals
  WHERE user_id = ? AND idempotency_key = ?
`;
const DEVICE_APPROVAL_INSERT_QUERY = `
  INSERT INTO device_approvals (
    user_id, approval_id, device_id, requester_device_id, status,
    requested_at, decided_at, expires_at, idempotency_key
  )
  SELECT ?, ?, ?, ?, 'approved', ?, ?, ?, ?
  WHERE EXISTS (
    SELECT 1
    FROM sync_vault_accounts AS accounts
    INNER JOIN sync_vault_envelopes AS envelope
      ON envelope.user_id = accounts.user_id
      AND envelope.key_id = accounts.current_key_id
      AND envelope.generation = accounts.current_generation
    WHERE accounts.user_id = ? AND envelope.recipient_device_id = ?
      AND envelope.approver_device_id = ? AND envelope.key_id = ?
      AND envelope.generation = ? AND envelope.envelope_version = ?
      AND envelope.suite = ? AND envelope.encapped_key = ?
      AND envelope.ciphertext = ? AND envelope.idempotency_key = ?
  )
  ON CONFLICT(user_id, idempotency_key) DO NOTHING
`;
const DEVICE_APPROVE_QUERY = `
  UPDATE user_devices
  SET approval_status = 'approved', approved_at = COALESCE(approved_at, ?), last_active_at = ?
  WHERE user_id = ? AND device_id = ? AND approval_status = 'pending' AND revoked_at IS NULL
    AND EXISTS (
      SELECT 1
      FROM sync_vault_accounts AS accounts
      INNER JOIN sync_vault_envelopes AS envelope
        ON envelope.user_id = accounts.user_id
        AND envelope.key_id = accounts.current_key_id
        AND envelope.generation = accounts.current_generation
      WHERE accounts.user_id = ? AND envelope.recipient_device_id = ?
        AND envelope.approver_device_id = ? AND envelope.key_id = ?
        AND envelope.generation = ? AND envelope.envelope_version = ?
        AND envelope.suite = ? AND envelope.encapped_key = ?
        AND envelope.ciphertext = ? AND envelope.idempotency_key = ?
    )
`;
const CURRENT_RECIPIENT_ENVELOPE_QUERY = `
  SELECT
    accounts.current_key_id AS key_id,
    accounts.current_generation AS generation,
    envelope.recipient_device_id,
    envelope.approver_device_id,
    envelope.envelope_version,
    envelope.suite,
    envelope.encapped_key,
    envelope.ciphertext,
    envelope.idempotency_key
  FROM sync_vault_accounts AS accounts
  INNER JOIN sync_vault_envelopes AS envelope
    ON envelope.user_id = accounts.user_id
    AND envelope.key_id = accounts.current_key_id
    AND envelope.generation = accounts.current_generation
  WHERE accounts.user_id = ? AND envelope.recipient_device_id = ?
`;

interface CurrentRecipientEnvelopeRow {
  key_id: unknown;
  generation: unknown;
  recipient_device_id: unknown;
  approver_device_id: unknown;
  envelope_version: unknown;
  suite: unknown;
  encapped_key: unknown;
  ciphertext: unknown;
  idempotency_key: unknown;
}

interface ApprovedRequesterRow {
  device_id: unknown;
  signing_public_key: unknown;
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
  const signingPublicKey = await approvedRequesterSigningKey(
    env,
    context.userId,
    requesterDeviceId,
  );
  await assertDeviceApprovalProof(
    signingPublicKey,
    context.userId,
    requesterDeviceId,
    approval,
  );
  const existingApproval = await env.ELY_DB.prepare(DEVICE_APPROVAL_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, approval.idempotencyKey)
    .first<DeviceApprovalRow>();
  assertFreshDeviceApprovalProof(approval, nowSeconds, existingApproval === null);
  if (existingApproval !== null) {
    return existingApprovalDocument(env, context, approval, requesterDeviceId, existingApproval);
  }

  const pendingDevice = await deviceRowById(env, context.userId, approval.deviceId);
  if (pendingDevice === null) {
    throw new DevicePermissionError("device_not_found");
  }
  const pendingDocument = storedDeviceDocument(pendingDevice, requesterDeviceId);
  if (
    pendingDocument.approval_status !== "pending" ||
    pendingDocument.revoked_at !== null ||
    pendingDocument.wrapping_public_key === undefined
  ) {
    throw new DevicePermissionError("device_not_pending");
  }

  const envelopeBindings = approvalEnvelopeBindings(
    context.userId,
    approval,
    requesterDeviceId,
  );
  await env.ELY_DB.batch([
    syncVaultRecipientEnvelopeStatement(
      env,
      context.userId,
      approval.deviceId,
      requesterDeviceId,
      approval.keyId,
      approval.generation,
      approval.envelope,
      approval.idempotencyKey,
      nowSeconds,
    ),
    env.ELY_DB.prepare(DEVICE_APPROVAL_INSERT_QUERY).bind(
      context.userId,
      approval.idempotencyKey,
      approval.deviceId,
      requesterDeviceId,
      nowSeconds,
      nowSeconds,
      nowSeconds,
      approval.idempotencyKey,
      ...envelopeBindings,
    ),
    env.ELY_DB.prepare(DEVICE_APPROVE_QUERY).bind(
      nowSeconds,
      nowSeconds,
      context.userId,
      approval.deviceId,
      ...envelopeBindings,
    ),
  ]);

  return approvedDeviceWithEnvelope(env, context, approval, requesterDeviceId, false);
}

async function existingApprovalDocument(
  env: Env,
  context: AuthContext,
  approval: DeviceApprovalRequest,
  requesterDeviceId: string,
  row: DeviceApprovalRow,
): Promise<DeviceApprovalDocument> {
  const { approvedDeviceId, approvedByDeviceId, status, decidedAt } = storedApproval(row);
  if (
    approvedDeviceId !== approval.deviceId ||
    approvedByDeviceId !== requesterDeviceId ||
    status !== "approved"
  ) {
    throw new DevicePermissionError("device_approval_replay_mismatch");
  }
  if (decidedAt === null) {
    throw new DevicePersistenceError("device_approval_state_invalid");
  }
  const document = await approvedDeviceWithEnvelope(
    env,
    context,
    approval,
    requesterDeviceId,
    true,
  );
  return { ...document, approved_at: decidedAt };
}

async function approvedDeviceWithEnvelope(
  env: Env,
  context: AuthContext,
  approval: DeviceApprovalRequest,
  requesterDeviceId: string,
  replay: boolean,
): Promise<DeviceApprovalDocument> {
  const approvedDevice = await deviceRowById(env, context.userId, approval.deviceId);
  if (approvedDevice === null) {
    throw new DevicePersistenceError("device_approval_missing");
  }
  const device = storedDeviceDocument(approvedDevice, requesterDeviceId);
  if (
    device.approval_status !== "approved" ||
    device.approved_at === null ||
    device.wrapping_public_key === undefined
  ) {
    if (device.approval_status === "approved" && device.wrapping_public_key === undefined) {
      throw new DevicePersistenceError("device_approval_state_invalid");
    }
    throw approvalMismatch(replay);
  }
  const envelope = await env.ELY_DB.prepare(CURRENT_RECIPIENT_ENVELOPE_QUERY)
    .bind(context.userId, approval.deviceId)
    .first<CurrentRecipientEnvelopeRow>();
  if (
    envelope === null ||
    !approvalEnvelopeMatches(storedApprovalEnvelope(envelope), approval, requesterDeviceId)
  ) {
    throw approvalMismatch(replay);
  }
  try {
    return approvedDeviceDocument(context.userId, requesterDeviceId, approvedDevice);
  } catch (error) {
    throw storedApprovalError(error);
  }
}

function approvalEnvelopeBindings(
  userId: string,
  approval: DeviceApprovalRequest,
  requesterDeviceId: string,
): unknown[] {
  return [
    userId,
    approval.deviceId,
    requesterDeviceId,
    approval.keyId,
    approval.generation,
    approval.envelope.version,
    approval.envelope.suite,
    approval.envelope.encapped_key,
    approval.envelope.ciphertext,
    approval.idempotencyKey,
  ];
}

function approvalEnvelopeMatches(
  row: CurrentRecipientEnvelopeRow,
  approval: DeviceApprovalRequest,
  requesterDeviceId: string,
): boolean {
  return (
    row.key_id === approval.keyId &&
    row.generation === approval.generation &&
    row.recipient_device_id === approval.deviceId &&
    row.approver_device_id === requesterDeviceId &&
    row.envelope_version === approval.envelope.version &&
    row.suite === approval.envelope.suite &&
    row.encapped_key === approval.envelope.encapped_key &&
    row.ciphertext === approval.envelope.ciphertext &&
    row.idempotency_key === approval.idempotencyKey
  );
}

function approvalMismatch(replay: boolean): Error {
  return replay
    ? new DevicePermissionError("device_approval_replay_mismatch")
    : new DeviceConflictError("device_approval_envelope_conflict");
}

function storedApproval(row: DeviceApprovalRow): {
  approvedDeviceId: string;
  approvedByDeviceId: string;
  status: string;
  decidedAt: number | null;
} {
  try {
    if (typeof row.status !== "string" || !STORED_APPROVAL_STATUSES.has(row.status)) {
      throw new DeviceSchemaError("approval_status_invalid");
    }
    return {
      approvedDeviceId: deviceIdValue(row.device_id, "device_id"),
      approvedByDeviceId: deviceIdValue(row.requester_device_id, "requester_device_id"),
      status: row.status,
      decidedAt: row.decided_at === null ? null : timestamp(row.decided_at, "decided_at"),
    };
  } catch (error) {
    throw storedApprovalError(error);
  }
}

function storedDeviceDocument(row: DeviceRow, requesterDeviceId: string) {
  try {
    return deviceDocument(row, requesterDeviceId);
  } catch (error) {
    throw storedApprovalError(error);
  }
}

function storedApprovalEnvelope(row: CurrentRecipientEnvelopeRow): CurrentRecipientEnvelopeRow {
  try {
    keyIdValue(row.key_id);
    positiveInteger(row.generation, "generation");
    deviceIdValue(row.recipient_device_id, "recipient_device_id");
    deviceIdValue(row.approver_device_id, "approver_device_id");
    wrappedAccountKey({
      version: row.envelope_version,
      suite: row.suite,
      encapped_key: row.encapped_key,
      ciphertext: row.ciphertext,
    });
    idempotencyKeyValue(row.idempotency_key);
    return row;
  } catch (error) {
    throw storedApprovalError(error);
  }
}

function storedApprovalError(error: unknown): Error {
  return error instanceof DeviceSchemaError
    ? new DevicePersistenceError("device_approval_state_invalid")
    : error as Error;
}

async function approvedRequesterSigningKey(
  env: Env,
  userId: string,
  requesterDeviceId: string,
): Promise<string> {
  const requester = await env.ELY_DB.prepare(APPROVED_REQUESTER_QUERY)
    .bind(userId, requesterDeviceId)
    .first<ApprovedRequesterRow>();
  if (requester === null) {
    throw new DevicePermissionError("requester_device_unapproved");
  }
  try {
    return publicKeyValue(requester.signing_public_key, "signing_public_key");
  } catch (error) {
    if (error instanceof DeviceSchemaError) {
      throw new DevicePersistenceError("requester_signing_key_invalid");
    }
    throw error;
  }
}

function deviceRowById(env: Env, userId: string, deviceId: string): Promise<DeviceRow | null> {
  return env.ELY_DB.prepare(DEVICE_BY_ID_QUERY).bind(userId, deviceId).first<DeviceRow>();
}
