import type { ElyD1PreparedStatement, Env } from "./bindings.js";
import {
  type ApprovedDeviceRevocationRequest,
  rotationEnvelopeIdempotencyKey,
} from "./device_revocation_schema.js";
import { DevicePersistenceError } from "./device_schema.js";

const ROTATION_R2_COUNT_QUERY = `
  SELECT COUNT(*) AS object_count FROM (
    SELECT payload_r2_key AS r2_key
    FROM sync_objects
    WHERE user_id = ? AND payload_r2_key IS NOT NULL
    UNION
    SELECT r2_key FROM sync_snapshots WHERE user_id = ?
  )
`;
const ROTATION_RESULT_QUERY = `
  SELECT
    rotation.target_device_id, rotation.approver_device_id,
    rotation.previous_key_id, rotation.previous_generation,
    rotation.new_key_id, rotation.new_generation, rotation.request_hash,
    rotation.envelope_count, rotation.r2_object_count, rotation.completed_at,
    account.current_key_id, account.current_generation,
    target.approval_status AS target_status, target.revoked_at,
    (SELECT COUNT(*)
      FROM better_auth_session AS session
      INNER JOIN better_auth_session_device_context AS context
        ON context.session_id = session.id
      WHERE context.user_id = rotation.user_id
        AND context.device_id = rotation.target_device_id) AS active_session_count,
    (SELECT COUNT(*) FROM sync_vault_rotation_envelopes AS item
      WHERE item.user_id = rotation.user_id
        AND item.rotation_idempotency_key = rotation.idempotency_key) AS item_count,
    (SELECT COUNT(*) FROM sync_vault_rotation_r2_objects AS item
      WHERE item.user_id = rotation.user_id
        AND item.rotation_idempotency_key = rotation.idempotency_key) AS r2_item_count,
    (SELECT COUNT(*)
      FROM sync_vault_rotation_envelopes AS item
      INNER JOIN sync_vault_envelopes AS envelope
        ON envelope.user_id = item.user_id
        AND envelope.recipient_device_id = item.recipient_device_id
        AND envelope.key_id = rotation.new_key_id
        AND envelope.generation = rotation.new_generation
        AND envelope.approver_device_id = rotation.approver_device_id
        AND envelope.envelope_version = item.envelope_version
        AND envelope.suite = item.suite
        AND envelope.encapped_key = item.encapped_key
        AND envelope.ciphertext = item.ciphertext
        AND envelope.idempotency_key = item.envelope_idempotency_key
      WHERE item.user_id = rotation.user_id
        AND item.rotation_idempotency_key = rotation.idempotency_key) AS persisted_count,
    (SELECT COUNT(*) FROM audit_events AS audit
      WHERE audit.event_id = rotation.audit_event_id
        AND audit.user_id = rotation.user_id
        AND audit.actor_device_id = rotation.approver_device_id
        AND audit.event_type = 'device.revoke'
        AND audit.subject_id = rotation.target_device_id
        AND audit.outcome = 'success'
        AND audit.metadata_hash = rotation.request_hash
        AND audit.created_at = rotation.completed_at) AS audit_count
  FROM sync_vault_rotations AS rotation
  LEFT JOIN sync_vault_accounts AS account ON account.user_id = rotation.user_id
  LEFT JOIN user_devices AS target
    ON target.user_id = rotation.user_id AND target.device_id = rotation.target_device_id
  WHERE rotation.user_id = ? AND rotation.idempotency_key = ?
`;
const ROTATION_INSERT_QUERY = `
  INSERT INTO sync_vault_rotations (
    user_id, idempotency_key, audit_event_id, target_device_id, approver_device_id,
    previous_key_id, previous_generation, new_key_id, new_generation,
    request_hash, envelope_count, r2_object_count, created_at, completed_at
  ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
  ON CONFLICT(user_id, idempotency_key) DO NOTHING
`;
const ROTATION_ENVELOPE_INSERT_QUERY = `
  INSERT INTO sync_vault_rotation_envelopes (
    user_id, rotation_idempotency_key, recipient_device_id,
    envelope_idempotency_key, envelope_version, suite, encapped_key, ciphertext
  )
  SELECT ?, ?, ?, ?, ?, ?, ?, ?
  FROM sync_vault_rotations AS rotation
  WHERE rotation.user_id = ? AND rotation.idempotency_key = ?
    AND rotation.completed_at IS NULL
    AND rotation.target_device_id = ? AND rotation.approver_device_id = ?
    AND rotation.previous_key_id = ? AND rotation.previous_generation = ?
    AND rotation.new_key_id = ? AND rotation.new_generation = ?
    AND rotation.request_hash = ?
  ON CONFLICT DO NOTHING
`;
const ROTATION_FINALIZE_QUERY = `
  UPDATE sync_vault_rotations
  SET completed_at = ?
  WHERE user_id = ? AND idempotency_key = ? AND completed_at IS NULL
    AND target_device_id = ? AND approver_device_id = ?
    AND previous_key_id = ? AND previous_generation = ?
    AND new_key_id = ? AND new_generation = ?
    AND request_hash = ? AND envelope_count = ? AND r2_object_count = ?
`;

interface R2CountRow { object_count: unknown }
export interface RotationResultRow {
  target_device_id: unknown;
  approver_device_id: unknown;
  previous_key_id: unknown;
  previous_generation: unknown;
  new_key_id: unknown;
  new_generation: unknown;
  request_hash: unknown;
  envelope_count: unknown;
  r2_object_count: unknown;
  completed_at: unknown;
  current_key_id: unknown;
  current_generation: unknown;
  target_status: unknown;
  revoked_at: unknown;
  active_session_count: unknown;
  item_count: unknown;
  r2_item_count: unknown;
  persisted_count: unknown;
  audit_count: unknown;
}

export async function rotationR2ObjectCount(env: Env, userId: string): Promise<number> {
  const rows = await env.ELY_DB.prepare(ROTATION_R2_COUNT_QUERY)
    .bind(userId, userId)
    .all<R2CountRow>();
  const count = rows.results[0]?.object_count;
  if (rows.results.length !== 1 || typeof count !== "number" || !Number.isSafeInteger(count) || count < 0) {
    throw new DevicePersistenceError("device_revocation_r2_count_invalid");
  }
  return count;
}

export function rotationResult(
  env: Env,
  userId: string,
  idempotencyKey: string,
): Promise<RotationResultRow | null> {
  return env.ELY_DB.prepare(ROTATION_RESULT_QUERY)
    .bind(userId, idempotencyKey)
    .first<RotationResultRow>();
}

export async function rotationStatements(
  env: Env,
  userId: string,
  approverDeviceId: string,
  revocation: ApprovedDeviceRevocationRequest,
  requestHash: string,
  r2ObjectCount: number,
  nowSeconds: number,
): Promise<ElyD1PreparedStatement[]> {
  const statements = [env.ELY_DB.prepare(ROTATION_INSERT_QUERY).bind(
    userId,
    revocation.idempotencyKey,
    `device-revoke:${requestHash}`,
    revocation.deviceId,
    approverDeviceId,
    revocation.previousKeyId,
    revocation.previousGeneration,
    revocation.newKeyId,
    revocation.newGeneration,
    requestHash,
    revocation.envelopes.length,
    r2ObjectCount,
    nowSeconds,
  )];
  const envelopeIds = await Promise.all(revocation.envelopes.map((item) =>
    rotationEnvelopeIdempotencyKey(userId, revocation.idempotencyKey, item.recipientDeviceId)
  ));
  revocation.envelopes.forEach((item, index) => statements.push(
    guardedEnvelopeStatement(
      env, userId, approverDeviceId, revocation, requestHash, envelopeIds[index]!, item,
    ),
  ));
  statements.push(env.ELY_DB.prepare(ROTATION_FINALIZE_QUERY).bind(
    nowSeconds,
    userId,
    revocation.idempotencyKey,
    revocation.deviceId,
    approverDeviceId,
    revocation.previousKeyId,
    revocation.previousGeneration,
    revocation.newKeyId,
    revocation.newGeneration,
    requestHash,
    revocation.envelopes.length,
    r2ObjectCount,
  ));
  return statements;
}

function guardedEnvelopeStatement(
  env: Env,
  userId: string,
  approverDeviceId: string,
  revocation: ApprovedDeviceRevocationRequest,
  requestHash: string,
  envelopeId: string,
  item: ApprovedDeviceRevocationRequest["envelopes"][number],
): ElyD1PreparedStatement {
  return env.ELY_DB.prepare(ROTATION_ENVELOPE_INSERT_QUERY).bind(
    userId,
    revocation.idempotencyKey,
    item.recipientDeviceId,
    envelopeId,
    item.envelope.version,
    item.envelope.suite,
    item.envelope.encapped_key,
    item.envelope.ciphertext,
    userId,
    revocation.idempotencyKey,
    revocation.deviceId,
    approverDeviceId,
    revocation.previousKeyId,
    revocation.previousGeneration,
    revocation.newKeyId,
    revocation.newGeneration,
    requestHash,
  );
}
