import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";
import { verifyEd25519Signature } from "./device_crypto.js";
import {
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
  assertOnlyFields,
  deviceIdValue,
  deviceRequestBody,
  publicKeyValue,
  signatureValue,
  timestamp,
} from "./device_schema.js";

const CHALLENGE_TTL_SECONDS = 300;
const CHALLENGE_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

const APPROVED_DEVICE_KEY_QUERY = `
  SELECT keys.signing_public_key
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2
`;
const CHALLENGE_UPSERT_QUERY = `
  INSERT INTO device_rebind_challenges (
    challenge_id,
    user_id,
    session_id,
    device_id,
    challenge,
    created_at,
    expires_at,
    consumed_at,
    consumption_nonce
  )
  SELECT ?, ?, ?, ?, ?, ?, ?, NULL, NULL
  WHERE EXISTS (
    SELECT 1 FROM better_auth_session WHERE id = ? AND userId = ?
  )
    AND NOT EXISTS (
      SELECT 1 FROM better_auth_session_device_context WHERE session_id = ?
    )
  ON CONFLICT(session_id) DO UPDATE SET
    challenge_id = excluded.challenge_id,
    user_id = excluded.user_id,
    device_id = excluded.device_id,
    challenge = excluded.challenge,
    created_at = excluded.created_at,
    expires_at = excluded.expires_at,
    consumed_at = NULL,
    consumption_nonce = NULL
`;
const CHALLENGE_QUERY = `
  SELECT
    rebind.challenge,
    rebind.expires_at,
    keys.signing_public_key
  FROM device_rebind_challenges AS rebind
  INNER JOIN user_devices AS device
    ON device.user_id = rebind.user_id AND device.device_id = rebind.device_id
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE rebind.challenge_id = ? AND rebind.user_id = ?
    AND rebind.session_id = ? AND rebind.device_id = ?
    AND rebind.consumed_at IS NULL
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2
`;
const CHALLENGE_CONSUME_QUERY = `
  UPDATE device_rebind_challenges
  SET consumed_at = ?, consumption_nonce = ?
  WHERE challenge_id = ? AND user_id = ? AND session_id = ? AND device_id = ?
    AND consumed_at IS NULL AND expires_at > ?
    AND NOT EXISTS (
      SELECT 1 FROM better_auth_session_device_context WHERE session_id = ?
    )
    AND EXISTS (
      SELECT 1
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id AND keys.device_id = device.device_id
      WHERE device.user_id = ? AND device.device_id = ?
        AND device.approval_status = 'approved' AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
    )
`;
const SESSION_BIND_QUERY = `
  INSERT INTO better_auth_session_device_context (
    session_id,
    user_id,
    device_id,
    updated_at
  )
  SELECT rebind.session_id, rebind.user_id, rebind.device_id, ?
  FROM device_rebind_challenges AS rebind
  WHERE rebind.challenge_id = ? AND rebind.consumption_nonce = ?
    AND rebind.consumed_at = ?
    AND EXISTS (
      SELECT 1 FROM better_auth_session
      WHERE id = rebind.session_id AND userId = rebind.user_id
    )
  ON CONFLICT(session_id) DO NOTHING
`;

interface ApprovedDeviceKeyRow {
  signing_public_key: unknown;
}

interface RebindChallengeRow extends ApprovedDeviceKeyRow {
  challenge: unknown;
  expires_at: unknown;
}

export interface DeviceRebindChallengeDocument {
  version: 1;
  challenge_id: string;
  device_id: string;
  challenge: string;
  expires_at: number;
}

export interface DeviceRebindDocument {
  version: 1;
  user_id: string;
  session_id: string;
  device_id: string;
  bound_at: number;
}

export async function issueDeviceRebindChallenge(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceRebindChallengeDocument> {
  const deviceId = await rebindChallengeDeviceId(request);
  assertUnboundSession(context);
  const keyRow = await env.ELY_DB.prepare(APPROVED_DEVICE_KEY_QUERY)
    .bind(context.userId, deviceId)
    .first<ApprovedDeviceKeyRow>();
  if (keyRow === null) {
    throw new DevicePermissionError("device_rebind_unavailable");
  }
  publicKeyValue(keyRow.signing_public_key, "signing_public_key");

  const challengeId = crypto.randomUUID();
  const expiresAt = nowSeconds + CHALLENGE_TTL_SECONDS;
  const challenge = canonicalChallenge(
    challengeId,
    context.userId,
    context.sessionId,
    deviceId,
    expiresAt,
    randomHex(32),
  );
  const result = await env.ELY_DB.prepare(CHALLENGE_UPSERT_QUERY)
    .bind(
      challengeId,
      context.userId,
      context.sessionId,
      deviceId,
      challenge,
      nowSeconds,
      expiresAt,
      context.sessionId,
      context.userId,
      context.sessionId,
    )
    .run();
  if (changedRowCount(result) !== 1) {
    throw new DevicePersistenceError("device_rebind_challenge_write_failed");
  }
  return { version: 1, challenge_id: challengeId, device_id: deviceId, challenge, expires_at: expiresAt };
}

export async function rebindDeviceSession(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<DeviceRebindDocument> {
  const rebind = await rebindRequest(request);
  assertUnboundSession(context);
  const row = await env.ELY_DB.prepare(CHALLENGE_QUERY)
    .bind(rebind.challengeId, context.userId, context.sessionId, rebind.deviceId)
    .first<RebindChallengeRow>();
  if (row === null) {
    throw new DevicePermissionError("device_rebind_forbidden");
  }
  const expiresAt = timestamp(row.expires_at, "expires_at");
  if (expiresAt <= nowSeconds) {
    throw new DevicePermissionError("device_rebind_challenge_expired");
  }
  const challenge = challengeValue(row.challenge);
  assertCanonicalChallenge(challenge, rebind.challengeId, context, rebind.deviceId, expiresAt);
  const signingPublicKey = publicKeyValue(row.signing_public_key, "signing_public_key");
  if (
    !(await verifyEd25519Signature(
      signingPublicKey,
      rebind.signature,
      new TextEncoder().encode(challenge),
    ))
  ) {
    throw new DevicePermissionError("device_rebind_signature_invalid");
  }

  const consumptionNonce = randomHex(32);
  const [consumeResult, bindResult] = await env.ELY_DB.batch([
    env.ELY_DB.prepare(CHALLENGE_CONSUME_QUERY).bind(
      nowSeconds,
      consumptionNonce,
      rebind.challengeId,
      context.userId,
      context.sessionId,
      rebind.deviceId,
      nowSeconds,
      context.sessionId,
      context.userId,
      rebind.deviceId,
    ),
    env.ELY_DB.prepare(SESSION_BIND_QUERY).bind(
      nowSeconds,
      rebind.challengeId,
      consumptionNonce,
      nowSeconds,
    ),
  ]);
  if (changedRowCount(consumeResult) !== 1 || changedRowCount(bindResult) !== 1) {
    throw new DeviceConflictError("device_rebind_challenge_consumed");
  }
  return {
    version: 1,
    user_id: context.userId,
    session_id: context.sessionId,
    device_id: rebind.deviceId,
    bound_at: nowSeconds,
  };
}

async function rebindChallengeDeviceId(request: Request): Promise<string> {
  const value = await deviceRequestBody(request, "device_rebind_challenge");
  assertOnlyFields(value, ["version", "device_id"]);
  if (value.version !== 1) {
    throw new DeviceSchemaError("device_rebind_challenge_version_invalid");
  }
  return deviceIdValue(value.device_id, "device_id");
}

async function rebindRequest(
  request: Request,
): Promise<{ challengeId: string; deviceId: string; signature: string }> {
  const value = await deviceRequestBody(request, "device_rebind");
  assertOnlyFields(value, ["version", "challenge_id", "device_id", "signature"]);
  if (value.version !== 1) {
    throw new DeviceSchemaError("device_rebind_version_invalid");
  }
  if (typeof value.challenge_id !== "string" || !CHALLENGE_ID_PATTERN.test(value.challenge_id)) {
    throw new DeviceSchemaError("challenge_id_invalid");
  }
  return {
    challengeId: value.challenge_id,
    deviceId: deviceIdValue(value.device_id, "device_id"),
    signature: signatureValue(value.signature, "signature"),
  };
}

function assertUnboundSession(context: AuthContext): void {
  if (context.deviceId !== undefined) {
    throw new DevicePermissionError("device_context_already_bound");
  }
}

function canonicalChallenge(
  challengeId: string,
  userId: string,
  sessionId: string,
  deviceId: string,
  expiresAt: number,
  nonce: string,
): string {
  return [
    "elydora-device-rebind-v1",
    `challenge_id=${challengeId}`,
    `user_id=${userId}`,
    `session_id=${sessionId}`,
    `device_id=${deviceId}`,
    `expires_at=${expiresAt}`,
    `nonce=${nonce}`,
  ].join("\n");
}

function assertCanonicalChallenge(
  challenge: string,
  challengeId: string,
  context: AuthContext,
  deviceId: string,
  expiresAt: number,
): void {
  const prefix = canonicalChallenge(
    challengeId,
    context.userId,
    context.sessionId,
    deviceId,
    expiresAt,
    "",
  );
  const nonce = challenge.slice(prefix.length);
  if (!challenge.startsWith(prefix) || !/^[a-f0-9]{64}$/.test(nonce)) {
    throw new DevicePersistenceError("device_rebind_challenge_invalid");
  }
}

function challengeValue(value: unknown): string {
  if (typeof value !== "string" || value.length < 1 || value.length > 1024) {
    throw new DevicePersistenceError("device_rebind_challenge_invalid");
  }
  return value;
}

function changedRowCount(result: unknown): number {
  if (typeof result !== "object" || result === null || !("meta" in result)) {
    throw new DevicePersistenceError("device_rebind_write_result_invalid");
  }
  const meta = result.meta;
  if (typeof meta !== "object" || meta === null || !("changes" in meta)) {
    throw new DevicePersistenceError("device_rebind_write_result_invalid");
  }
  const changes = meta.changes;
  if (typeof changes !== "number" || !Number.isSafeInteger(changes) || changes < 0) {
    throw new DevicePersistenceError("device_rebind_write_result_invalid");
  }
  return changes;
}

function randomHex(byteLength: number): string {
  const bytes = new Uint8Array(byteLength);
  crypto.getRandomValues(bytes);
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
