import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession, ElyD1PreparedStatement, ElyD1Result } from "./bindings.js";

const DESTRUCTIVE_ACTION_GATE_INSERT_QUERY = `
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
  ) VALUES (
    ?, ?, ?, ?, ?, ?,
    CASE WHEN EXISTS (
      SELECT 1
      FROM better_auth_session AS session
      INNER JOIN better_auth_session_device_context AS session_device
        ON session_device.session_id = session.id
        AND session_device.user_id = session.userId
      INNER JOIN user_devices AS device
        ON device.user_id = session_device.user_id
        AND device.device_id = session_device.device_id
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id
        AND keys.device_id = device.device_id
      WHERE session.id = ? AND session.userId = ?
        AND session_device.device_id = ?
        AND (
          (typeof(session.expiresAt) = 'text' AND julianday(session.expiresAt) > julianday(?))
          OR
          (typeof(session.expiresAt) IN ('integer', 'real') AND session.expiresAt > ?)
        )
        AND device.approval_status = 'approved' AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
        AND keys.signing_public_key = ?
    ) THEN 'success' ELSE NULL END,
    ?, ?
  )
`;

const DESTRUCTIVE_ACTION_LIVE_QUERY = `
  SELECT 1 AS authorized
  FROM better_auth_session AS session
  INNER JOIN better_auth_session_device_context AS session_device
    ON session_device.session_id = session.id
    AND session_device.user_id = session.userId
  INNER JOIN user_devices AS device
    ON device.user_id = session_device.user_id
    AND device.device_id = session_device.device_id
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id
    AND keys.device_id = device.device_id
  WHERE session.id = ? AND session.userId = ?
    AND session_device.device_id = ?
    AND (
      (typeof(session.expiresAt) = 'text' AND julianday(session.expiresAt) > julianday(?))
      OR
      (typeof(session.expiresAt) IN ('integer', 'real') AND session.expiresAt > ?)
    )
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
    AND keys.signing_public_key = ?
`;

export interface DestructiveActionGate {
  eventId: string;
  auditUserId: string | null;
  eventType: "account.delete" | "sync.reset";
  subjectType: "account" | "sync";
  subjectId: string;
  metadataHash: string | null;
}

export function destructiveActionGateStatement(
  database: ElyD1DatabaseSession,
  context: AuthContext,
  signingPublicKey: string,
  gate: DestructiveActionGate,
  nowSeconds: number,
): ElyD1PreparedStatement {
  if (context.deviceId === undefined) {
    throw new DestructiveActionGateError("device_context_required");
  }
  const now = new Date(nowSeconds * 1000);
  if (!Number.isSafeInteger(nowSeconds) || nowSeconds < 0 || !Number.isFinite(now.getTime())) {
    throw new DestructiveActionGateError("destructive_action_time_invalid");
  }
  return database.prepare(DESTRUCTIVE_ACTION_GATE_INSERT_QUERY).bind(
    gate.eventId,
    gate.auditUserId,
    context.deviceId,
    gate.eventType,
    gate.subjectType,
    gate.subjectId,
    context.sessionId,
    context.userId,
    context.deviceId,
    now.toISOString(),
    now.getTime(),
    signingPublicKey,
    gate.metadataHash,
    nowSeconds,
  );
}

export function assertDestructiveActionGateResult(result: unknown): void {
  if (changedRows(result) !== 1) {
    throw new DestructiveActionGateError("destructive_action_gate_failed");
  }
}

export async function destructiveActionGateIsLive(
  database: ElyD1DatabaseSession,
  context: AuthContext,
  signingPublicKey: string,
  nowSeconds: number,
): Promise<boolean> {
  if (context.deviceId === undefined) return false;
  const now = new Date(nowSeconds * 1000);
  if (!Number.isSafeInteger(nowSeconds) || nowSeconds < 0 || !Number.isFinite(now.getTime())) {
    throw new DestructiveActionGateError("destructive_action_time_invalid");
  }
  const row = await database.prepare(DESTRUCTIVE_ACTION_LIVE_QUERY).bind(
    context.sessionId,
    context.userId,
    context.deviceId,
    now.toISOString(),
    now.getTime(),
    signingPublicKey,
  ).first<{ authorized: unknown }>();
  return row?.authorized === 1;
}

export class DestructiveActionGateError extends Error {}

function changedRows(result: unknown): number {
  if (typeof result !== "object" || result === null || !("meta" in result)) return -1;
  const changes = (result as ElyD1Result).meta?.changes;
  return typeof changes === "number" && Number.isSafeInteger(changes) ? changes : -1;
}
