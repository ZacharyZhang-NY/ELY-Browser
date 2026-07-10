import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import { assertDeviceRegistrationProof } from "./device_registration_proof.js";
import {
  type DeviceListDocument,
  type DeviceRegistrationDocument,
  type DeviceRegistrationRequest,
  type DeviceRow,
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  currentDeviceId,
  deviceDocument,
  deviceRegistrationRequest,
} from "./device_schema.js";

export {
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
} from "./device_schema.js";

const UNBOUND_SESSION_MAX_AGE_SECONDS = 10 * 60;
const SESSION_CLOCK_SKEW_SECONDS = 30;

const DEVICE_COLUMNS = `
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
`;
const DEVICE_FROM = `
  FROM user_devices AS device
  LEFT JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
`;
const DEVICE_LIST_QUERY = `
  SELECT ${DEVICE_COLUMNS}
  ${DEVICE_FROM}
  WHERE device.user_id = ?
  ORDER BY
    device.revoked_at IS NOT NULL,
    COALESCE(device.last_active_at, device.approved_at, device.created_at) DESC,
    device.device_id ASC
`;
const DEVICE_REGISTER_QUERY = `
  WITH registration AS (
    SELECT CASE
      WHEN NOT EXISTS (SELECT 1 FROM user_devices WHERE user_id = ?) THEN 'approved'
      ELSE 'pending'
    END AS approval_status
  )
  INSERT INTO user_devices (
    user_id, device_id, public_key, device_name, platform,
    approval_status, created_at, approved_at, last_active_at, revoked_at, idempotency_key
  )
  SELECT ?, ?, ?, ?, ?, approval_status, ?,
    CASE WHEN approval_status = 'approved' THEN ? ELSE NULL END,
    ?, NULL, ?
  FROM registration
  WHERE EXISTS (
    SELECT 1 FROM better_auth_session
    WHERE id = ? AND userId = ?
  )
  ON CONFLICT DO NOTHING
`;
const DEVICE_KEYS_INSERT_QUERY = `
  INSERT INTO user_device_keys (
    user_id, device_id, signing_public_key,
    wrapping_public_key, key_protocol_version, created_at
  )
  SELECT ?, ?, ?, ?, 2, ?
  WHERE EXISTS (
    SELECT 1 FROM user_devices
    WHERE user_id = ? AND device_id = ? AND public_key = ?
      AND device_name = ? AND platform = ? AND idempotency_key = ?
      AND revoked_at IS NULL
  )
    AND EXISTS (
      SELECT 1 FROM better_auth_session
      WHERE id = ? AND userId = ?
    )
  ON CONFLICT DO NOTHING
`;
const DEVICE_BY_IDEMPOTENCY_KEY_QUERY = `
  SELECT ${DEVICE_COLUMNS}
  ${DEVICE_FROM}
  WHERE device.user_id = ? AND device.idempotency_key = ?
`;
const DEVICE_BY_ID_QUERY = `
  SELECT ${DEVICE_COLUMNS}
  ${DEVICE_FROM}
  WHERE device.user_id = ? AND device.device_id = ?
`;
const SESSION_DEVICE_CONTEXT_UPSERT_QUERY = `
  INSERT INTO better_auth_session_device_context (
    session_id, user_id, device_id, updated_at
  )
  SELECT ?, ?, ?, ?
  WHERE EXISTS (
    SELECT 1
    FROM better_auth_session
    WHERE id = ? AND userId = ?
  )
  ON CONFLICT(session_id) DO NOTHING
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
  await assertDeviceRegistrationProof(registration);
  assertFreshUnboundSession(context, nowSeconds);

  const [deviceWriteResult, keyWriteResult] = await env.ELY_DB.batch([
    env.ELY_DB.prepare(DEVICE_REGISTER_QUERY).bind(
      context.userId,
      context.userId,
      registration.deviceId,
      registration.publicKey,
      registration.deviceName,
      registration.platform,
      nowSeconds,
      nowSeconds,
      nowSeconds,
      registration.idempotencyKey,
      context.sessionId,
      context.userId,
    ),
    env.ELY_DB.prepare(DEVICE_KEYS_INSERT_QUERY).bind(
      context.userId,
      registration.deviceId,
      registration.publicKey,
      registration.wrappingPublicKey,
      nowSeconds,
      context.userId,
      registration.deviceId,
      registration.publicKey,
      registration.deviceName,
      registration.platform,
      registration.idempotencyKey,
      context.sessionId,
      context.userId,
    ),
  ]);
  const insertedRows = changedRowCount(deviceWriteResult, "device_registration");
  const insertedKeyRows = changedRowCount(keyWriteResult, "device_key_registration");
  if (insertedRows > 1 || insertedKeyRows > 1 || insertedRows !== insertedKeyRows) {
    throw new DevicePersistenceError("device_registration_write_count_invalid");
  }
  const row = await env.ELY_DB.prepare(DEVICE_BY_IDEMPOTENCY_KEY_QUERY)
    .bind(context.userId, registration.idempotencyKey)
    .first<DeviceRow>();
  if (row === null) {
    if (insertedRows === 0) {
      throw new DeviceConflictError("device_registration_conflict");
    }
    throw new DevicePersistenceError("device_registration_missing");
  }
  const device = deviceDocument(row, registration.deviceId);
  if (!registrationMatches(device, registration)) {
    throw new DeviceConflictError("device_registration_conflict");
  }
  if (insertedRows === 0) {
    if (
      context.deviceId === undefined ||
      context.deviceId !== device.device_id ||
      device.revoked_at !== null
    ) {
      throw new DeviceConflictError("device_registration_conflict");
    }
    return {
      version: 2,
      user_id: context.userId,
      device,
    };
  }
  if (device.revoked_at !== null) {
    throw new DevicePersistenceError("device_registration_state_invalid");
  }
  await bindSessionDeviceContext(env, context, device.device_id, nowSeconds);

  return {
    version: 2,
    user_id: context.userId,
    device,
  };
}

function assertFreshUnboundSession(context: AuthContext, nowSeconds: number): void {
  if (context.deviceId !== undefined) {
    return;
  }
  const createdAtSeconds = Math.floor(Date.parse(context.createdAt) / 1000);
  if (
    createdAtSeconds < nowSeconds - UNBOUND_SESSION_MAX_AGE_SECONDS ||
    createdAtSeconds > nowSeconds + SESSION_CLOCK_SKEW_SECONDS
  ) {
    throw new DevicePermissionError("fresh_session_required");
  }
}

function registrationMatches(
  device: DeviceRegistrationDocument["device"],
  registration: DeviceRegistrationRequest,
): boolean {
  return (
    device.device_id === registration.deviceId &&
    device.public_key === registration.publicKey &&
    device.wrapping_public_key === registration.wrappingPublicKey &&
    device.device_name === registration.deviceName &&
    device.platform === registration.platform
  );
}

function changedRowCount(result: unknown, label: string): number {
  if (typeof result !== "object" || result === null || !("meta" in result)) {
    throw new DevicePersistenceError(`${label}_write_result_invalid`);
  }
  const meta = result.meta;
  if (typeof meta !== "object" || meta === null || !("changes" in meta)) {
    throw new DevicePersistenceError(`${label}_write_result_invalid`);
  }
  const changes = meta.changes;
  if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes < 0) {
    throw new DevicePersistenceError(`${label}_write_result_invalid`);
  }
  return changes;
}

async function bindSessionDeviceContext(
  env: Env,
  context: AuthContext,
  deviceId: string,
  nowSeconds: number,
): Promise<void> {
  const result = await env.ELY_DB.prepare(SESSION_DEVICE_CONTEXT_UPSERT_QUERY)
    .bind(context.sessionId, context.userId, deviceId, nowSeconds, context.sessionId, context.userId)
    .run();
  if (changedRowCount(result, "device_session_binding") !== 1) {
    throw new DeviceConflictError("device_session_binding_conflict");
  }
}
