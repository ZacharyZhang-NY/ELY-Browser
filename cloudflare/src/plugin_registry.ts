import { prefixedKvKey } from "./kv_keys.js";

const PLUGIN_REGISTRY_NAMESPACE = "plugin_registry_cache";
const PLUGIN_ID_PATTERN = /^[a-z0-9][a-z0-9_-]*(?:\.[a-z0-9][a-z0-9_-]*)*$/;
const SIGNATURE_KEY_ID_PATTERN = /^[a-z0-9._-]{3,128}$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const SHA256_HEX_PATTERN = /^[a-f0-9]{64}$/;
const ED25519_SIGNATURE_HEX_PATTERN = /^[a-f0-9]{128}$/;
const PUBLIC_KEY_HEX_PATTERN = /^[a-f0-9]{64}$/;
const PLUGIN_PERMISSIONS = new Set([
  "tabs:read",
  "tabs:write",
  "spaces:read",
  "spaces:write",
  "bookmarks:read",
  "bookmarks:write",
  "history:read",
  "downloads:read",
  "downloads:write",
  "page:metadata",
  "page:screenshot",
  "page:script",
  "clipboard:read",
  "clipboard:write",
  "filesystem:read",
  "filesystem:write",
  "network:fetch",
  "settings:read",
  "settings:write",
  "sync:plugin",
  "ui:panel",
  "ui:command",
  "ui:context_menu",
]);
const PLUGIN_CONTRIBUTIONS = new Set([
  "command-bar-command",
  "tab-context-menu",
  "page-context-menu",
  "sidebar-panel",
  "settings-page",
  "status-bar-indicator",
  "download-action",
  "bookmark-action",
  "reading-mode-exporter",
]);

export interface PluginSignatureDocument {
  algorithm: "ed25519";
  key_id: string;
  public_key: string;
  value: string;
}

export interface PluginPackageLocation {
  url: string;
  sha256: string;
  size_bytes: number;
}

export interface PluginRegistryEntry {
  id: string;
  name: string;
  description: string;
  author: string;
  homepage: string;
  permissions: string[];
  contributes: string[];
  min_ely_build: string;
  checksum: string;
  signature: PluginSignatureDocument;
  package: PluginPackageLocation;
}

export interface PluginRegistryDocument {
  version: 1;
  generated_at: string;
  plugins: PluginRegistryEntry[];
}

export interface PluginCatalogEntry {
  id: string;
  name: string;
  description: string;
  author: string;
  homepage: string;
  permissions: string[];
  contributes: string[];
  min_ely_build: string;
}

export interface PluginCatalogDocument {
  version: 1;
  generated_at: string;
  plugins: PluginCatalogEntry[];
}

export interface PluginDetailsDocument {
  version: 1;
  generated_at: string;
  plugin: Omit<PluginRegistryEntry, "package">;
}

export interface PluginPackageDocument {
  version: 1;
  plugin_id: string;
  url: string;
  sha256: string;
  signature: string;
  size_bytes: number;
}

export class PluginRegistrySchemaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PluginRegistrySchemaError";
  }
}

export function pluginRegistryKvKey(environment: string): string {
  return prefixedKvKey(environment, PLUGIN_REGISTRY_NAMESPACE);
}

export function parsePluginRegistryDocument(value: string): PluginRegistryDocument {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new PluginRegistrySchemaError("plugin registry must be valid JSON");
  }

  if (!isRecord(parsed)) {
    throw new PluginRegistrySchemaError("plugin registry must be an object");
  }
  assertOnlyFields(parsed, ["version", "generated_at", "plugins"], "plugin registry");

  if (parsed.version !== 1) {
    throw new PluginRegistrySchemaError("plugin registry version must be 1");
  }

  return {
    version: 1,
    generated_at: isoTimestamp(stringField(parsed, "generated_at"), "generated_at"),
    plugins: parsePlugins(parsed.plugins),
  };
}

export function pluginCatalogDocument(
  registry: PluginRegistryDocument,
): PluginCatalogDocument {
  return {
    version: 1,
    generated_at: registry.generated_at,
    plugins: registry.plugins.map(pluginCatalogEntry),
  };
}

export function pluginDetailsDocument(
  registry: PluginRegistryDocument,
  pluginId: string,
): PluginDetailsDocument | null {
  const plugin = registry.plugins.find((entry) => entry.id === pluginId);
  if (plugin === undefined) {
    return null;
  }

  return {
    version: 1,
    generated_at: registry.generated_at,
    plugin: pluginDetailsEntry(plugin),
  };
}

export function pluginPackageDocument(
  registry: PluginRegistryDocument,
  pluginId: string,
): PluginPackageDocument | null {
  const plugin = registry.plugins.find((entry) => entry.id === pluginId);
  if (plugin === undefined) {
    return null;
  }

  return {
    version: 1,
    plugin_id: plugin.id,
    url: plugin.package.url,
    sha256: plugin.package.sha256,
    signature: plugin.signature.value,
    size_bytes: plugin.package.size_bytes,
  };
}

export function pluginRegistryId(value: string): string {
  if (!isPluginId(value)) {
    throw new PluginRegistrySchemaError(`invalid plugin id: ${value}`);
  }
  return value;
}

function parsePlugins(value: unknown): PluginRegistryEntry[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new PluginRegistrySchemaError("plugin registry must contain plugins");
  }

  const plugins: PluginRegistryEntry[] = [];
  const seenPluginIds = new Set<string>();
  for (const pluginValue of value) {
    const plugin = parsePlugin(pluginValue);
    if (seenPluginIds.has(plugin.id)) {
      throw new PluginRegistrySchemaError(`duplicate plugin id: ${plugin.id}`);
    }
    seenPluginIds.add(plugin.id);
    plugins.push(plugin);
  }
  return plugins;
}

function parsePlugin(value: unknown): PluginRegistryEntry {
  if (!isRecord(value)) {
    throw new PluginRegistrySchemaError("plugin entry must be an object");
  }
  assertOnlyFields(
    value,
    [
      "id",
      "name",
      "description",
      "author",
      "homepage",
      "permissions",
      "contributes",
      "min_ely_build",
      "checksum",
      "signature",
      "package",
    ],
    "plugin entry",
  );

  return {
    id: pluginRegistryId(stringField(value, "id")),
    name: stringField(value, "name"),
    description: stringField(value, "description"),
    author: stringField(value, "author"),
    homepage: httpUrl(stringField(value, "homepage"), "homepage"),
    permissions: uniqueKnownStrings(value.permissions, PLUGIN_PERMISSIONS, "permission"),
    contributes: uniqueKnownStrings(value.contributes, PLUGIN_CONTRIBUTIONS, "contribution"),
    min_ely_build: semver(stringField(value, "min_ely_build"), "min_ely_build"),
    checksum: sha256Hex(stringField(value, "checksum"), "checksum"),
    signature: parseSignature(value.signature),
    package: parsePackage(value.package),
  };
}

function parseSignature(value: unknown): PluginSignatureDocument {
  if (!isRecord(value)) {
    throw new PluginRegistrySchemaError("plugin signature must be an object");
  }
  assertOnlyFields(value, ["algorithm", "key_id", "public_key", "value"], "plugin signature");

  const algorithm = stringField(value, "algorithm");
  if (algorithm !== "ed25519") {
    throw new PluginRegistrySchemaError(`invalid plugin signature algorithm: ${algorithm}`);
  }

  return {
    algorithm: "ed25519",
    key_id: signatureKeyId(stringField(value, "key_id")),
    public_key: publicKeyHex(stringField(value, "public_key")),
    value: ed25519SignatureHex(stringField(value, "value")),
  };
}

function parsePackage(value: unknown): PluginPackageLocation {
  if (!isRecord(value)) {
    throw new PluginRegistrySchemaError("plugin package must be an object");
  }
  assertOnlyFields(value, ["url", "sha256", "size_bytes"], "plugin package");

  return {
    url: httpsUrl(stringField(value, "url"), "package.url"),
    sha256: sha256Hex(stringField(value, "sha256"), "package.sha256"),
    size_bytes: positiveIntegerField(value, "size_bytes"),
  };
}

function pluginCatalogEntry(plugin: PluginRegistryEntry): PluginCatalogEntry {
  return {
    id: plugin.id,
    name: plugin.name,
    description: plugin.description,
    author: plugin.author,
    homepage: plugin.homepage,
    permissions: plugin.permissions,
    contributes: plugin.contributes,
    min_ely_build: plugin.min_ely_build,
  };
}

function pluginDetailsEntry(plugin: PluginRegistryEntry): Omit<PluginRegistryEntry, "package"> {
  return {
    ...pluginCatalogEntry(plugin),
    checksum: plugin.checksum,
    signature: plugin.signature,
  };
}

function uniqueKnownStrings(value: unknown, allowed: Set<string>, label: string): string[] {
  if (!Array.isArray(value)) {
    throw new PluginRegistrySchemaError(`plugin ${label}s must be an array`);
  }

  const values: string[] = [];
  const seen = new Set<string>();
  for (const item of value) {
    if (typeof item !== "string" || item.trim() === "") {
      throw new PluginRegistrySchemaError(`plugin ${label} must be a non-empty string`);
    }

    const normalized = item.trim();
    if (!allowed.has(normalized)) {
      throw new PluginRegistrySchemaError(`invalid plugin ${label}: ${normalized}`);
    }
    if (seen.has(normalized)) {
      throw new PluginRegistrySchemaError(`duplicate plugin ${label}: ${normalized}`);
    }
    seen.add(normalized);
    values.push(normalized);
  }
  return values;
}

function signatureKeyId(value: string): string {
  if (!SIGNATURE_KEY_ID_PATTERN.test(value)) {
    throw new PluginRegistrySchemaError(`invalid plugin signature key id: ${value}`);
  }
  return value;
}

function semver(value: string, field: string): string {
  if (!SEMVER_PATTERN.test(value)) {
    throw new PluginRegistrySchemaError(`${field} must be a semantic version`);
  }
  return value;
}

function httpUrl(value: string, field: string): string {
  const url = parsedUrl(value, field);
  if (!matchesProtocol(url, ["http:", "https:"])) {
    throw new PluginRegistrySchemaError(`${field} must use http or https`);
  }
  return url.toString();
}

function httpsUrl(value: string, field: string): string {
  const url = parsedUrl(value, field);
  if (url.protocol !== "https:") {
    throw new PluginRegistrySchemaError(`${field} must use https`);
  }
  return url.toString();
}

function parsedUrl(value: string, field: string): URL {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new PluginRegistrySchemaError(`${field} must be a valid URL`);
  }
  if (url.host === "") {
    throw new PluginRegistrySchemaError(`${field} must include a host`);
  }
  return url;
}

function matchesProtocol(url: URL, protocols: string[]): boolean {
  return protocols.includes(url.protocol);
}

function sha256Hex(value: string, field: string): string {
  const normalized = value.toLowerCase();
  if (!SHA256_HEX_PATTERN.test(normalized)) {
    throw new PluginRegistrySchemaError(`${field} must be a SHA-256 hex digest`);
  }
  return normalized;
}

function publicKeyHex(value: string): string {
  const normalized = value.toLowerCase();
  if (!PUBLIC_KEY_HEX_PATTERN.test(normalized)) {
    throw new PluginRegistrySchemaError("plugin signature public key must be Ed25519 hex");
  }
  return normalized;
}

function ed25519SignatureHex(value: string): string {
  const normalized = value.toLowerCase();
  if (!ED25519_SIGNATURE_HEX_PATTERN.test(normalized)) {
    throw new PluginRegistrySchemaError("plugin signature value must be Ed25519 hex");
  }
  return normalized;
}

function isoTimestamp(value: string, field: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    throw new PluginRegistrySchemaError(`${field} must be an ISO timestamp`);
  }
  return new Date(timestamp).toISOString();
}

function positiveIntegerField(value: Record<string, unknown>, field: string): number {
  const fieldValue = value[field];
  if (typeof fieldValue !== "number" || !Number.isSafeInteger(fieldValue) || fieldValue <= 0) {
    throw new PluginRegistrySchemaError(`${field} must be a positive integer`);
  }
  return fieldValue;
}

function stringField(value: Record<string, unknown>, field: string): string {
  const fieldValue = value[field];
  if (typeof fieldValue !== "string" || fieldValue.trim() === "") {
    throw new PluginRegistrySchemaError(`${field} must be a non-empty string`);
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
      throw new PluginRegistrySchemaError(`${label} has unknown field: ${field}`);
    }
  }
}

function isPluginId(value: string): boolean {
  return value.length >= 3 && value.length <= 128 && PLUGIN_ID_PATTERN.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
