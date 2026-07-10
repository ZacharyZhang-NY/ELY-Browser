import type { AuthContext } from "./auth.js";
import type { ElyD1DatabaseSession, ElyD1PreparedStatement, Env } from "./bindings.js";
import { syncVaultBootstrapProofValid } from "./sync_vault_bootstrap_proof.js";
import {
  CURRENT_DEVICE_ENVELOPE_QUERY,
  CURRENT_SYNC_VAULT_KEY_QUERY,
  HISTORICAL_DEVICE_ENVELOPE_QUERY,
  SYNC_VAULT_ACCOUNT_INSERT_QUERY,
  SYNC_VAULT_ENVELOPE_INSERT_QUERY,
} from "./sync_vault_sql.js";

const HPKE_SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";
const SHA256_HEX_PATTERN = /^[a-f0-9]{64}$/;
const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const IDEMPOTENCY_KEY_PATTERN = /^[a-zA-Z0-9._:-]{16,128}$/;
const SIGNATURE_PATTERN = /^[a-f0-9]{128}$/;
const BASE64URL_32_BYTES_PATTERN = /^[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]$/;
const BASE64URL_48_BYTES_PATTERN = /^[A-Za-z0-9_-]{64}$/;

export interface CurrentSyncVaultKey {
  keyId: string;
  generation: number;
}

export interface SyncVaultDocument {
  version: 1;
  user_id: string;
  key_id: string;
  generation: number;
  recipient_device_id: string;
  approver_device_id: string;
  envelope: WrappedAccountKeyDocument;
  created_at: number;
}

export interface WrappedAccountKeyDocument {
  version: 1;
  suite: typeof HPKE_SUITE;
  encapped_key: string;
  ciphertext: string;
}

interface SyncVaultBootstrapRequest {
  keyId: string;
  generation: number;
  envelope: WrappedAccountKeyDocument;
  idempotencyKey: string;
  bootstrapProof: string;
}

interface SyncVaultEnvelopeLookup {
  keyId: string;
  generation: number;
}

interface SyncVaultKeyRow {
  key_id: unknown;
  generation: unknown;
}

interface SyncVaultEnvelopeRow extends SyncVaultKeyRow {
  recipient_device_id: unknown;
  approver_device_id: unknown;
  envelope_version: unknown;
  suite: unknown;
  encapped_key: unknown;
  ciphertext: unknown;
  idempotency_key: unknown;
  created_at: unknown;
}
interface StoredSyncVaultEnvelope {
  document: SyncVaultDocument;
  idempotencyKey: string;
}
type RequestBody = Record<string, unknown>;
export class SyncVaultRequestError extends Error {}
export class SyncVaultPermissionError extends Error {}
export class SyncVaultNotFoundError extends Error {}
export class SyncVaultConflictError extends Error {}
export class SyncVaultPersistenceError extends Error {}
export async function syncVaultBootstrapDocument(
  request: Request,
  env: Env,
  context: AuthContext,
  nowSeconds = Math.floor(Date.now() / 1000),
): Promise<SyncVaultDocument> {
  const deviceId = currentDeviceId(context);
  const bootstrap = await syncVaultBootstrapRequest(request);
  const { bootstrapProof, ...unsignedBootstrap } = bootstrap;
  if (!(await syncVaultBootstrapProofValid(
    env,
    context.userId,
    deviceId,
    unsignedBootstrap,
    bootstrapProof,
  ))) {
    throw new SyncVaultPermissionError("sync_vault_bootstrap_proof_invalid");
  }
  await env.ELY_DB.batch(syncVaultBootstrapStatements(env, context.userId, deviceId, bootstrap, nowSeconds));

  const stored = await currentDeviceEnvelope(env, context.userId, deviceId);
  if (stored === null) {
    const currentKey = await currentSyncVaultKey(env, context.userId);
    if (currentKey === null) {
      throw new SyncVaultPersistenceError("sync_vault_account_missing");
    }
    if (currentKey.keyId !== bootstrap.keyId || currentKey.generation !== bootstrap.generation) {
      throw new SyncVaultConflictError("sync_vault_key_conflict");
    }
    throw new SyncVaultPersistenceError("sync_vault_envelope_missing");
  }
  assertBootstrapMatches(stored, bootstrap, deviceId);
  return stored.document;
}

export async function syncVaultCurrentDeviceDocument(
  url: URL,
  env: Env,
  context: AuthContext,
): Promise<SyncVaultDocument> {
  const deviceId = currentDeviceId(context);
  const stored = await currentDeviceEnvelope(
    env,
    context.userId,
    deviceId,
    syncVaultEnvelopeLookup(url),
  );
  if (stored === null) {
    throw new SyncVaultNotFoundError("sync_vault_envelope_not_found");
  }
  return stored.document;
}

export async function currentSyncVaultKey(
  env: Env,
  userId: string,
  database: ElyD1DatabaseSession = env.ELY_DB,
): Promise<CurrentSyncVaultKey | null> {
  const row = await database.prepare(CURRENT_SYNC_VAULT_KEY_QUERY)
    .bind(userId)
    .first<SyncVaultKeyRow>();
  if (row === null) {
    return null;
  }
  return {
    keyId: storedKeyId(row.key_id),
    generation: storedInteger(row.generation, "generation", 1),
  };
}

export async function assertCurrentSyncVaultKey(
  env: Env,
  userId: string,
  keyId: string,
  generation: number,
  database: ElyD1DatabaseSession = env.ELY_DB,
): Promise<void> {
  const current = await currentSyncVaultKey(env, userId, database);
  if (current === null) {
    throw new SyncVaultNotFoundError("sync_vault_not_initialized");
  }
  if (current.keyId !== keyId || current.generation !== generation) {
    throw new SyncVaultConflictError("sync_vault_key_not_current");
  }
}
export function parseWrappedAccountKey(value: unknown): WrappedAccountKeyDocument { return requestEnvelope(value); }

export function syncVaultRecipientEnvelopeStatement(
  env: Env,
  userId: string,
  recipientDeviceId: string,
  approverDeviceId: string,
  keyId: string,
  generation: number,
  envelope: WrappedAccountKeyDocument,
  idempotencyKey: string,
  nowSeconds: number,
): ElyD1PreparedStatement {
  return syncVaultEnvelopeStatement(
    env,
    userId,
    recipientDeviceId,
    approverDeviceId,
    keyId,
    generation,
    envelope,
    idempotencyKey,
    nowSeconds,
    "pending",
  );
}

function syncVaultBootstrapStatements(
  env: Env,
  userId: string,
  deviceId: string,
  bootstrap: SyncVaultBootstrapRequest,
  nowSeconds: number,
): ElyD1PreparedStatement[] {
  return [
    env.ELY_DB.prepare(SYNC_VAULT_ACCOUNT_INSERT_QUERY).bind(
      userId,
      bootstrap.keyId,
      bootstrap.generation,
      nowSeconds,
      nowSeconds,
      userId,
      deviceId,
    ),
    syncVaultEnvelopeStatement(
      env,
      userId,
      deviceId,
      deviceId,
      bootstrap.keyId,
      bootstrap.generation,
      bootstrap.envelope,
      bootstrap.idempotencyKey,
      nowSeconds,
      "approved",
    ),
  ];
}

function syncVaultEnvelopeStatement(
  env: Env,
  userId: string,
  recipientDeviceId: string,
  approverDeviceId: string,
  keyId: string,
  generation: number,
  envelope: WrappedAccountKeyDocument,
  idempotencyKey: string,
  nowSeconds: number,
  recipientStatus: "pending" | "approved",
): ElyD1PreparedStatement {
  const wrapped = requestEnvelope(envelope);
  requestDeviceId(recipientDeviceId, "recipient_device_id");
  requestDeviceId(approverDeviceId, "approver_device_id");
  requestKeyId(keyId);
  requestInteger(generation, "generation", 1);
  requestIdempotencyKey(idempotencyKey);
  requestInteger(nowSeconds, "created_at", 0);
  return env.ELY_DB.prepare(SYNC_VAULT_ENVELOPE_INSERT_QUERY).bind(
    userId,
    recipientDeviceId,
    approverDeviceId,
    keyId,
    generation,
    wrapped.version,
    wrapped.suite,
    wrapped.encapped_key,
    wrapped.ciphertext,
    idempotencyKey,
    nowSeconds,
    recipientDeviceId,
    recipientStatus,
    approverDeviceId,
    userId,
    keyId,
    generation,
  );
}

async function currentDeviceEnvelope(
  env: Env,
  userId: string,
  deviceId: string,
  lookup?: SyncVaultEnvelopeLookup,
): Promise<StoredSyncVaultEnvelope | null> {
  const statement = lookup === undefined
    ? env.ELY_DB.prepare(CURRENT_DEVICE_ENVELOPE_QUERY).bind(userId, deviceId)
    : env.ELY_DB.prepare(HISTORICAL_DEVICE_ENVELOPE_QUERY)
      .bind(userId, deviceId, lookup.keyId, lookup.generation);
  const row = await statement.first<SyncVaultEnvelopeRow>();
  if (row === null) {
    return null;
  }
  const recipientDeviceId = storedDeviceId(row.recipient_device_id, "recipient_device_id");
  if (recipientDeviceId !== deviceId) {
    throw new SyncVaultPersistenceError("recipient_device_id_mismatch");
  }
  return {
    document: {
      version: 1,
      user_id: userId,
      key_id: storedKeyId(row.key_id),
      generation: storedInteger(row.generation, "generation", 1),
      recipient_device_id: recipientDeviceId,
      approver_device_id: storedDeviceId(row.approver_device_id, "approver_device_id"),
      envelope: storedEnvelope(row),
      created_at: storedInteger(row.created_at, "created_at", 0),
    },
    idempotencyKey: storedIdempotencyKey(row.idempotency_key),
  };
}

function syncVaultEnvelopeLookup(url: URL): SyncVaultEnvelopeLookup | undefined {
  if ([...url.searchParams].length === 0) {
    return undefined;
  }
  for (const field of url.searchParams.keys()) {
    if (field !== "key_id" && field !== "generation") {
      throw new SyncVaultRequestError(`unexpected_query:${field}`);
    }
  }
  const keyIds = url.searchParams.getAll("key_id");
  const generations = url.searchParams.getAll("generation");
  if (keyIds.length !== 1 || generations.length !== 1) {
    throw new SyncVaultRequestError("vault_query_pair_required");
  }
  if (!/^[1-9][0-9]*$/.test(generations[0] ?? "")) {
    throw new SyncVaultRequestError("generation_invalid");
  }
  return {
    keyId: requestKeyId(keyIds[0]),
    generation: requestInteger(Number(generations[0]), "generation", 1),
  };
}

async function syncVaultBootstrapRequest(request: Request): Promise<SyncVaultBootstrapRequest> {
  const body = await requestBody(request);
  assertOnlyFields(body, [
    "version",
    "key_id",
    "generation",
    "envelope",
    "idempotency_key",
    "bootstrap_proof",
  ]);
  if (body.version !== 2) {
    throw new SyncVaultRequestError("version_invalid");
  }
  if (body.generation !== 1) {
    throw new SyncVaultRequestError("generation_invalid");
  }
  return {
    keyId: requestKeyId(body.key_id),
    generation: 1,
    envelope: requestEnvelope(body.envelope),
    idempotencyKey: requestIdempotencyKey(body.idempotency_key),
    bootstrapProof: requestSignature(body.bootstrap_proof),
  };
}

function requestEnvelope(value: unknown): WrappedAccountKeyDocument {
  const envelope = record(value, "envelope");
  assertOnlyFields(envelope, ["version", "suite", "encapped_key", "ciphertext"]);
  if (envelope.version !== 1 || envelope.suite !== HPKE_SUITE) {
    throw new SyncVaultRequestError("envelope_metadata_invalid");
  }
  if (typeof envelope.encapped_key !== "string" || !BASE64URL_32_BYTES_PATTERN.test(envelope.encapped_key)) {
    throw new SyncVaultRequestError("encapped_key_invalid");
  }
  if (typeof envelope.ciphertext !== "string" || !BASE64URL_48_BYTES_PATTERN.test(envelope.ciphertext)) {
    throw new SyncVaultRequestError("ciphertext_invalid");
  }
  return {
    version: 1,
    suite: HPKE_SUITE,
    encapped_key: envelope.encapped_key,
    ciphertext: envelope.ciphertext,
  };
}

function storedEnvelope(row: SyncVaultEnvelopeRow): WrappedAccountKeyDocument {
  if (row.envelope_version !== 1 || row.suite !== HPKE_SUITE) {
    throw new SyncVaultPersistenceError("envelope_metadata_invalid");
  }
  if (typeof row.encapped_key !== "string" || !BASE64URL_32_BYTES_PATTERN.test(row.encapped_key)) {
    throw new SyncVaultPersistenceError("encapped_key_invalid");
  }
  if (typeof row.ciphertext !== "string" || !BASE64URL_48_BYTES_PATTERN.test(row.ciphertext)) {
    throw new SyncVaultPersistenceError("ciphertext_invalid");
  }
  return {
    version: 1,
    suite: HPKE_SUITE,
    encapped_key: row.encapped_key,
    ciphertext: row.ciphertext,
  };
}

function assertBootstrapMatches(
  stored: StoredSyncVaultEnvelope,
  bootstrap: SyncVaultBootstrapRequest,
  deviceId: string,
): void {
  const { document } = stored;
  if (
    document.key_id !== bootstrap.keyId ||
    document.generation !== bootstrap.generation ||
    document.recipient_device_id !== deviceId ||
    document.approver_device_id !== deviceId ||
    document.envelope.version !== bootstrap.envelope.version ||
    document.envelope.suite !== bootstrap.envelope.suite ||
    document.envelope.encapped_key !== bootstrap.envelope.encapped_key ||
    document.envelope.ciphertext !== bootstrap.envelope.ciphertext ||
    stored.idempotencyKey !== bootstrap.idempotencyKey
  ) {
    throw new SyncVaultConflictError("sync_vault_bootstrap_replay_mismatch");
  }
}

async function requestBody(request: Request): Promise<RequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new SyncVaultRequestError("json_invalid");
  }
  return record(value, "body");
}

function record(value: unknown, label: string): RequestBody {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncVaultRequestError(`${label}_invalid`);
  }
  return value as RequestBody;
}

function assertOnlyFields(value: RequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new SyncVaultRequestError(`unexpected_field:${field}`);
    }
  }
}

function currentDeviceId(context: AuthContext): string {
  if (context.deviceId === undefined) {
    throw new SyncVaultRequestError("device_context_required");
  }
  return context.deviceId;
}

function requestKeyId(value: unknown): string {
  if (typeof value !== "string" || !SHA256_HEX_PATTERN.test(value)) {
    throw new SyncVaultRequestError("key_id_invalid");
  }
  return value;
}

function storedKeyId(value: unknown): string {
  if (typeof value !== "string" || !SHA256_HEX_PATTERN.test(value)) {
    throw new SyncVaultPersistenceError("key_id_invalid");
  }
  return value;
}

function requestIdempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !IDEMPOTENCY_KEY_PATTERN.test(value)) {
    throw new SyncVaultRequestError("idempotency_key_invalid");
  }
  return value;
}

function requestSignature(value: unknown): string {
  if (typeof value !== "string" || !SIGNATURE_PATTERN.test(value)) {
    throw new SyncVaultRequestError("bootstrap_proof_invalid");
  }
  return value;
}

function requestDeviceId(value: unknown, label: string): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new SyncVaultRequestError(`${label}_invalid`);
  }
  return value;
}

function requestInteger(value: unknown, label: string, min: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min) {
    throw new SyncVaultRequestError(`${label}_invalid`);
  }
  return value;
}

function storedIdempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !IDEMPOTENCY_KEY_PATTERN.test(value)) {
    throw new SyncVaultPersistenceError("idempotency_key_invalid");
  }
  return value;
}

function storedDeviceId(value: unknown, label: string): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new SyncVaultPersistenceError(`${label}_invalid`);
  }
  return value;
}

function storedInteger(value: unknown, label: string, min: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min) {
    throw new SyncVaultPersistenceError(`${label}_invalid`);
  }
  return value;
}
