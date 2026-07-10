import type { Env } from "./bindings.js";
import { prefixedKvKey } from "./kv_keys.js";

const AUTH_SESSION_CACHE_NAMESPACE = "auth_session_cache";
const BEARER_TOKEN_PATTERN = /^[A-Za-z0-9._~+/=-]{32,4096}$/;
const SUBJECT_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const BETTER_AUTH_SESSION_QUERY = `
  SELECT
    session.id,
    session.userId,
    session.expiresAt,
    device_context.device_id AS deviceId
  FROM better_auth_session AS session
  LEFT JOIN better_auth_session_device_context AS device_context
    ON device_context.session_id = session.id
  WHERE session.token = ?
`;

export interface AuthContext {
  userId: string;
  sessionId: string;
  tokenHash: string;
  expiresAt: string;
  deviceId?: string;
}

interface BetterAuthSessionRow extends Record<string, unknown> {
  id: unknown;
  userId: unknown;
  expiresAt: unknown;
  deviceId?: unknown;
}

export type AuthErrorCode =
  | "authorization_missing"
  | "authorization_invalid"
  | "session_not_found"
  | "session_expired";

export class AuthError extends Error {
  constructor(readonly code: AuthErrorCode) {
    super(code);
    this.name = "AuthError";
  }
}

export class AuthSessionSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AuthSessionSchemaError";
  }
}

export function authSessionCacheKvKey(environment: string, tokenHash: string): string {
  if (!/^[a-f0-9]{64}$/.test(tokenHash)) {
    throw new AuthSessionSchemaError("token_hash_invalid");
  }
  return `${prefixedKvKey(environment, AUTH_SESSION_CACHE_NAMESPACE)}:${tokenHash}`;
}

export async function authenticatedRateLimitKey(
  environment: string,
  route: string,
  request: Request,
): Promise<string> {
  let token: string | null;
  try {
    token = bearerToken(request);
  } catch (error) {
    if (error instanceof AuthError) {
      return `${environment}:${route}:authorization_invalid`;
    }
    throw error;
  }
  if (token === null) {
    return `${environment}:${route}:anonymous`;
  }
  return `${environment}:${route}:bearer:${await sha256Hex(token)}`;
}

export async function readAuthContext(
  request: Request,
  env: Env,
  now: Date = new Date(),
): Promise<AuthContext> {
  const token = bearerToken(request);
  if (token === null) {
    throw new AuthError("authorization_missing");
  }

  const tokenHash = await sha256Hex(token);
  return readBetterAuthSessionContext(env, token, tokenHash, now);
}

async function readBetterAuthSessionContext(
  env: Env,
  token: string,
  tokenHash: string,
  now: Date,
): Promise<AuthContext> {
  const row = await env.ELY_DB.prepare(BETTER_AUTH_SESSION_QUERY)
    .bind(token)
    .first<BetterAuthSessionRow>();
  if (row === null) {
    throw new AuthError("session_not_found");
  }

  const session: AuthContext = {
    userId: subjectId(stringField(row, "userId"), "userId"),
    sessionId: subjectId(stringField(row, "id"), "session_id"),
    tokenHash,
    expiresAt: timestampField(row, "expiresAt"),
  };
  const deviceId = optionalStringField(row, "deviceId");
  if (deviceId !== undefined) {
    session.deviceId = deviceIdValue(deviceId);
  }
  if (Date.parse(session.expiresAt) <= now.getTime()) {
    throw new AuthError("session_expired");
  }
  return session;
}

export async function authTokenHash(token: string): Promise<string> {
  return sha256Hex(token);
}

function bearerToken(request: Request): string | null {
  const header = request.headers.get("authorization");
  if (header === null) {
    return null;
  }

  const [scheme, token, extra] = header.trim().split(/\s+/);
  if (scheme !== "Bearer" || token === undefined || extra !== undefined) {
    throw new AuthError("authorization_invalid");
  }
  if (!BEARER_TOKEN_PATTERN.test(token)) {
    throw new AuthError("authorization_invalid");
  }
  return token;
}

function subjectId(value: string, label: string): string {
  if (!SUBJECT_ID_PATTERN.test(value)) {
    throw new AuthSessionSchemaError(`${label}_invalid`);
  }
  return value;
}

function deviceIdValue(value: string): string {
  if (!DEVICE_ID_PATTERN.test(value)) {
    throw new AuthSessionSchemaError("device_id_invalid");
  }
  return value;
}

function isoTimestamp(value: string, label: string): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) {
    throw new AuthSessionSchemaError(`${label}_invalid`);
  }
  return new Date(timestamp).toISOString();
}

function timestampField(value: Record<string, unknown>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue === "string" && fieldValue.trim() !== "") {
    return isoTimestamp(fieldValue, field);
  }
  if (typeof fieldValue === "number" && Number.isFinite(fieldValue)) {
    return new Date(fieldValue).toISOString();
  }
  if (fieldValue instanceof Date) {
    return fieldValue.toISOString();
  }
  throw new AuthSessionSchemaError(`${field}_required`);
}

function stringField(value: Record<string, unknown>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue !== "string" || fieldValue.trim() === "") {
    throw new AuthSessionSchemaError(`${field}_required`);
  }
  return fieldValue.trim();
}

function optionalStringField(value: Record<string, unknown>, field: string): string | undefined {
  const fieldValue = value[field];
  if (fieldValue === undefined || fieldValue === null) {
    return undefined;
  }
  if (typeof fieldValue !== "string" || fieldValue.trim() === "") {
    throw new AuthSessionSchemaError(`${field}_invalid`);
  }
  return fieldValue.trim();
}

async function sha256Hex(value: string): Promise<string> {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
