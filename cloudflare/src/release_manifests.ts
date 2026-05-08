import { prefixedKvKey } from "./kv_keys.js";

const RELEASE_MANIFEST_NAMESPACE = "release_manifest_cache";
const RELEASE_CHANNEL_PATTERN = /^(stable|beta|nightly)$/;
const RELEASE_PLATFORM_PATTERN = /^(macos|windows|linux)$/;
const RELEASE_ARCHITECTURE_PATTERN = /^[a-z0-9._-]{2,32}$/;
const RELEASE_VERSION_PATTERN = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const SHA256_HEX_PATTERN = /^[a-f0-9]{64}$/;
const ED25519_SIGNATURE_HEX_PATTERN = /^[a-f0-9]{128}$/;

export interface ReleaseArtifact {
  platform: string;
  architecture: string;
  version: string;
  url: string;
  sha256: string;
  signature: string;
  size_bytes: number;
}

export interface ReleaseManifestDocument {
  version: 1;
  channel: string;
  generated_at: string;
  artifacts: ReleaseArtifact[];
}

export interface ReleaseSignatureQuery {
  platform: string;
  architecture: string;
  version?: string;
}

export interface ReleaseSignatureDocument {
  version: 1;
  channel: string;
  generated_at: string;
  platform: string;
  architecture: string;
  release_version: string;
  sha256: string;
  signature: string;
}

export class ReleaseManifestSchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ReleaseManifestSchemaError";
  }
}

export class ReleaseSignatureQueryError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ReleaseSignatureQueryError";
  }
}

export function releaseManifestKvKey(environment: string): string {
  return prefixedKvKey(environment, RELEASE_MANIFEST_NAMESPACE);
}

export function parseReleaseManifestDocument(value: string): ReleaseManifestDocument {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new ReleaseManifestSchemaError("release manifest must be valid JSON");
  }

  if (!isRecord(parsed)) {
    throw new ReleaseManifestSchemaError("release manifest must be an object");
  }
  assertOnlyFields(parsed, ["version", "channel", "generated_at", "artifacts"], "release manifest");

  if (parsed.version !== 1) {
    throw new ReleaseManifestSchemaError("release manifest version must be 1");
  }

  return {
    version: 1,
    channel: releaseChannel(stringField(parsed, "channel")),
    generated_at: isoTimestamp(stringField(parsed, "generated_at"), "generated_at"),
    artifacts: parseArtifacts(parsed.artifacts),
  };
}

export function parseReleaseSignatureQuery(
  searchParams: URLSearchParams,
): ReleaseSignatureQuery {
  const allowedFields = new Set(["platform", "architecture", "version"]);
  for (const field of searchParams.keys()) {
    if (!allowedFields.has(field)) {
      throw new ReleaseSignatureQueryError(`unknown release signature query field: ${field}`);
    }
  }

  const platform = requiredQueryField(searchParams, "platform");
  const architecture = requiredQueryField(searchParams, "architecture");
  const version = optionalQueryField(searchParams, "version");
  return {
    platform: releasePlatform(platform),
    architecture: releaseArchitecture(architecture),
    ...(version === undefined ? {} : { version: releaseVersion(version) }),
  };
}

export function releaseSignatureDocument(
  manifest: ReleaseManifestDocument,
  query: ReleaseSignatureQuery,
): ReleaseSignatureDocument | null {
  const artifact = manifest.artifacts.find((value) => {
    const targetMatches =
      value.platform === query.platform && value.architecture === query.architecture;
    const versionMatches = query.version === undefined || value.version === query.version;
    return targetMatches && versionMatches;
  });

  if (artifact === undefined) {
    return null;
  }

  return {
    version: 1,
    channel: manifest.channel,
    generated_at: manifest.generated_at,
    platform: artifact.platform,
    architecture: artifact.architecture,
    release_version: artifact.version,
    sha256: artifact.sha256,
    signature: artifact.signature,
  };
}

function parseArtifacts(value: unknown): ReleaseArtifact[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new ReleaseManifestSchemaError("release manifest must contain artifacts");
  }

  const artifacts: ReleaseArtifact[] = [];
  const seenTargets = new Set<string>();
  for (const artifactValue of value) {
    const artifact = parseArtifact(artifactValue);
    const target = `${artifact.platform}:${artifact.architecture}`;
    if (seenTargets.has(target)) {
      throw new ReleaseManifestSchemaError(`duplicate release artifact target: ${target}`);
    }
    seenTargets.add(target);
    artifacts.push(artifact);
  }
  return artifacts;
}

function parseArtifact(value: unknown): ReleaseArtifact {
  if (!isRecord(value)) {
    throw new ReleaseManifestSchemaError("release artifact must be an object");
  }
  assertOnlyFields(
    value,
    ["platform", "architecture", "version", "url", "sha256", "signature", "size_bytes"],
    "release artifact",
  );

  return {
    platform: releasePlatform(stringField(value, "platform")),
    architecture: releaseArchitecture(stringField(value, "architecture")),
    version: releaseVersion(stringField(value, "version")),
    url: httpsUrl(stringField(value, "url")),
    sha256: sha256Hex(stringField(value, "sha256")),
    signature: ed25519SignatureHex(stringField(value, "signature")),
    size_bytes: positiveIntegerField(value, "size_bytes"),
  };
}

function releaseChannel(value: string): string {
  if (!RELEASE_CHANNEL_PATTERN.test(value)) {
    throw new ReleaseManifestSchemaError(`invalid release channel: ${value}`);
  }
  return value;
}

function releasePlatform(value: string): string {
  if (!RELEASE_PLATFORM_PATTERN.test(value)) {
    throw new ReleaseManifestSchemaError(`invalid release platform: ${value}`);
  }
  return value;
}

function releaseArchitecture(value: string): string {
  if (!RELEASE_ARCHITECTURE_PATTERN.test(value)) {
    throw new ReleaseManifestSchemaError(`invalid release architecture: ${value}`);
  }
  return value;
}

function releaseVersion(value: string): string {
  if (!RELEASE_VERSION_PATTERN.test(value)) {
    throw new ReleaseManifestSchemaError(`invalid release version: ${value}`);
  }
  return value;
}

function httpsUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new ReleaseManifestSchemaError(`invalid release artifact URL: ${value}`);
  }
  if (url.protocol !== "https:") {
    throw new ReleaseManifestSchemaError(`release artifact URL must use https: ${value}`);
  }
  return url.toString();
}

function sha256Hex(value: string): string {
  const normalized = value.toLowerCase();
  if (!SHA256_HEX_PATTERN.test(normalized)) {
    throw new ReleaseManifestSchemaError(`invalid release artifact sha256: ${value}`);
  }
  return normalized;
}

function ed25519SignatureHex(value: string): string {
  const normalized = value.toLowerCase();
  if (!ED25519_SIGNATURE_HEX_PATTERN.test(normalized)) {
    throw new ReleaseManifestSchemaError("invalid release artifact signature");
  }
  return normalized;
}

function isoTimestamp(value: string, field: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    throw new ReleaseManifestSchemaError(`${field} must be an ISO timestamp`);
  }
  return new Date(timestamp).toISOString();
}

function positiveIntegerField(value: Record<string, unknown>, field: string): number {
  const fieldValue = value[field];
  if (typeof fieldValue !== "number" || !Number.isSafeInteger(fieldValue) || fieldValue <= 0) {
    throw new ReleaseManifestSchemaError(`${field} must be a positive integer`);
  }
  return fieldValue;
}

function stringField(value: Record<string, unknown>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue !== "string" || fieldValue.trim() === "") {
    throw new ReleaseManifestSchemaError(`${field} must be a non-empty string`);
  }
  return fieldValue.trim();
}

function assertOnlyFields(
  value: Record<string, unknown>,
  allowedFields: string[],
  label: string,
): void {
  const allowed = new Set(allowedFields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new ReleaseManifestSchemaError(`${label} has unknown field: ${field}`);
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requiredQueryField(searchParams: URLSearchParams, field: string): string {
  const values = searchParams.getAll(field);
  const [value] = values;
  if (values.length !== 1 || value === undefined || value.trim() === "") {
    throw new ReleaseSignatureQueryError(`${field} query parameter is required`);
  }
  return value.trim();
}

function optionalQueryField(searchParams: URLSearchParams, field: string): string | undefined {
  const values = searchParams.getAll(field);
  if (values.length === 0) {
    return undefined;
  }
  const [value] = values;
  if (values.length !== 1 || value === undefined || value.trim() === "") {
    throw new ReleaseSignatureQueryError(`${field} query parameter is invalid`);
  }
  return value.trim();
}
