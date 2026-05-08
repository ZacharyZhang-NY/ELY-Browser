const KV_NAMESPACE_PREFIX = "ely";
const ENVIRONMENT_NAME_PATTERN = /^[a-z0-9._-]{3,64}$/;
const KV_KEY_PART_PATTERN = /^[a-z0-9._-]{3,128}$/;

export class KvKeySchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "KvKeySchemaError";
  }
}

export function prefixedKvKey(environment: string, namespace: string): string {
  const normalizedEnvironment = normalizedEnvironmentName(environment);
  const normalizedNamespace = normalizedKvKeyPart("KV namespace", namespace);
  return `${KV_NAMESPACE_PREFIX}:${normalizedEnvironment}:${normalizedNamespace}`;
}

function normalizedEnvironmentName(value: string): string {
  const environment = value.trim();
  if (!ENVIRONMENT_NAME_PATTERN.test(environment)) {
    throw new KvKeySchemaError(`invalid environment name: ${value}`);
  }
  return environment;
}

function normalizedKvKeyPart(label: string, value: string): string {
  const keyPart = value.trim();
  if (!KV_KEY_PART_PATTERN.test(keyPart)) {
    throw new KvKeySchemaError(`invalid ${label}: ${value}`);
  }
  return keyPart;
}
