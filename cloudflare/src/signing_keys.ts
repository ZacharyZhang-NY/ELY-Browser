import { prefixedKvKey } from "./kv_keys.js";

const KEY_ID_PATTERN = /^[a-z0-9._-]{3,128}$/;
const PUBLIC_KEY_PATTERN = /^[a-f0-9]{64}$/;
const PUBLIC_SIGNING_KEYS_NAMESPACE = "public_signing_keys";

export interface PublicSigningKey {
  key_id: string;
  public_key: string;
}

export interface PublicSigningKeysDocument {
  version: 1;
  keys: PublicSigningKey[];
}

export class SigningKeysSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SigningKeysSchemaError";
  }
}

export function publicSigningKeysKvKey(environment: string): string {
  return prefixedKvKey(environment, PUBLIC_SIGNING_KEYS_NAMESPACE);
}

export function parsePublicSigningKeysDocument(value: string): PublicSigningKeysDocument {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new SigningKeysSchemaError("public signing keys document must be valid JSON");
  }

  if (!isRecord(parsed)) {
    throw new SigningKeysSchemaError("public signing keys document must be an object");
  }

  if (parsed.version !== 1) {
    throw new SigningKeysSchemaError("public signing keys document version must be 1");
  }

  if (!Array.isArray(parsed.keys) || parsed.keys.length === 0) {
    throw new SigningKeysSchemaError("public signing keys document must contain keys");
  }

  return {
    version: 1,
    keys: parsePublicSigningKeys(parsed.keys),
  };
}

function parsePublicSigningKeys(values: unknown[]): PublicSigningKey[] {
  const keys: PublicSigningKey[] = [];
  const seenKeyIds = new Set<string>();

  for (const value of values) {
    const key = parsePublicSigningKey(value);
    if (seenKeyIds.has(key.key_id)) {
      throw new SigningKeysSchemaError(`duplicate public signing key id: ${key.key_id}`);
    }
    seenKeyIds.add(key.key_id);
    keys.push(key);
  }

  return keys;
}

function parsePublicSigningKey(value: unknown): PublicSigningKey {
  if (!isRecord(value)) {
    throw new SigningKeysSchemaError("public signing key must be an object");
  }

  const keyId = stringField(value, "key_id");
  const publicKey = stringField(value, "public_key").toLowerCase();

  if (!KEY_ID_PATTERN.test(keyId)) {
    throw new SigningKeysSchemaError(`invalid public signing key id: ${keyId}`);
  }
  if (!PUBLIC_KEY_PATTERN.test(publicKey)) {
    throw new SigningKeysSchemaError(`invalid public signing key value for ${keyId}`);
  }

  return { key_id: keyId, public_key: publicKey };
}

function stringField(value: Record<string, unknown>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue !== "string" || fieldValue.trim() === "") {
    throw new SigningKeysSchemaError(`${field} must be a non-empty string`);
  }
  return fieldValue.trim();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
