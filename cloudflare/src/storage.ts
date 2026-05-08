import type { ElyR2Bucket } from "./bindings.js";

const SHA256_HEX = /^[a-f0-9]{64}$/;
const REGION = /^[a-z0-9][a-z0-9-]{1,31}$/;
const SEGMENT = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const SYNC_OBJECT_TYPES = new Set([
  "spaces",
  "tabs",
  "bookmarks",
  "notes",
  "reading-list",
  "profiles",
  "site-permissions",
  "history",
  "plugin-settings",
]);

export class StorageObjectError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "StorageObjectError";
  }
}

export interface StoredObject {
  key: string;
  sha256: string;
  sizeBytes: number;
}

export function syncPayloadKey(params: {
  region: string;
  userHash: string;
  objectType: string;
  objectId: string;
  payloadHash: string;
}): string {
  assertRegion(params.region);
  assertSha256Hex(params.userHash, "user_hash");
  assertSyncObjectType(params.objectType);
  assertSegment(params.objectId, "object_id");
  assertSha256Hex(params.payloadHash, "payload_hash");
  return [
    "sync-payloads",
    params.region,
    params.userHash,
    params.objectType,
    params.objectId,
    `${params.payloadHash}.bin`,
  ].join("/");
}

export function syncSnapshotKey(params: {
  region: string;
  userHash: string;
  snapshotId: string;
}): string {
  assertRegion(params.region);
  assertSha256Hex(params.userHash, "user_hash");
  assertSegment(params.snapshotId, "snapshot_id");
  return ["sync-snapshots", params.region, params.userHash, `${params.snapshotId}.bin`].join("/");
}

export function pluginPackageKey(params: { pluginId: string; packageHash: string }): string {
  assertSegment(params.pluginId, "plugin_id");
  assertSha256Hex(params.packageHash, "package_hash");
  return ["plugin-packages", params.pluginId, `${params.packageHash}.rplug`].join("/");
}

export function pluginAssetKey(params: { pluginId: string; assetHash: string }): string {
  assertSegment(params.pluginId, "plugin_id");
  assertSha256Hex(params.assetHash, "asset_hash");
  return ["plugin-assets", params.pluginId, params.assetHash].join("/");
}

export function userAvatarKey(params: { userHash: string; avatarHash: string }): string {
  assertSha256Hex(params.userHash, "user_hash");
  assertSha256Hex(params.avatarHash, "avatar_hash");
  return ["user-avatars", params.userHash, params.avatarHash].join("/");
}

export function crashAttachmentKey(params: {
  reportId: string;
  attachmentHash: string;
}): string {
  assertSegment(params.reportId, "report_id");
  assertSha256Hex(params.attachmentHash, "attachment_hash");
  return ["crash-attachments", params.reportId, params.attachmentHash].join("/");
}

export function exportObjectKey(params: { userHash: string; exportId: string }): string {
  assertSha256Hex(params.userHash, "user_hash");
  assertSegment(params.exportId, "export_id");
  return ["exports", params.userHash, `${params.exportId}.bin`].join("/");
}

export async function putVerifiedObject(
  bucket: ElyR2Bucket,
  key: string,
  payload: ArrayBuffer,
  expectedSha256: string,
  contentType: string,
): Promise<StoredObject> {
  assertKnownObjectKey(key);
  assertSha256Hex(expectedSha256, "sha256");
  assertKeyChecksum(key, expectedSha256);
  const actualSha256 = await sha256Hex(payload);
  if (actualSha256 !== expectedSha256) {
    throw new StorageObjectError("r2_checksum_mismatch");
  }

  await bucket.put(key, payload, {
    httpMetadata: { contentType },
    customMetadata: { sha256: expectedSha256 },
    sha256: hexToArrayBuffer(expectedSha256),
  });
  return { key, sha256: expectedSha256, sizeBytes: payload.byteLength };
}

export async function getVerifiedObject(
  bucket: ElyR2Bucket,
  key: string,
  expectedSha256: string,
): Promise<ArrayBuffer | null> {
  assertKnownObjectKey(key);
  assertSha256Hex(expectedSha256, "sha256");
  assertKeyChecksum(key, expectedSha256);
  const object = await bucket.get(key);
  if (object === null) {
    return null;
  }

  const payload = await object.arrayBuffer();
  const actualSha256 = await sha256Hex(payload);
  if (actualSha256 !== expectedSha256) {
    throw new StorageObjectError("r2_checksum_mismatch");
  }
  return payload;
}

function assertKnownObjectKey(key: string): void {
  const matches = [
    /^sync-payloads\/[a-z0-9][a-z0-9-]{1,31}\/[a-f0-9]{64}\/[a-z0-9][a-z0-9._-]{0,127}\/[a-z0-9][a-z0-9._-]{0,127}\/[a-f0-9]{64}\.bin$/,
    /^sync-snapshots\/[a-z0-9][a-z0-9-]{1,31}\/[a-f0-9]{64}\/[a-z0-9][a-z0-9._-]{0,127}\.bin$/,
    /^plugin-packages\/[a-z0-9][a-z0-9._-]{0,127}\/[a-f0-9]{64}\.rplug$/,
    /^plugin-assets\/[a-z0-9][a-z0-9._-]{0,127}\/[a-f0-9]{64}$/,
    /^user-avatars\/[a-f0-9]{64}\/[a-f0-9]{64}$/,
    /^crash-attachments\/[a-z0-9][a-z0-9._-]{0,127}\/[a-f0-9]{64}$/,
    /^exports\/[a-f0-9]{64}\/[a-z0-9][a-z0-9._-]{0,127}\.bin$/,
  ];
  if (!matches.some((pattern) => pattern.test(key))) {
    throw new StorageObjectError("r2_key_invalid");
  }
  if (key.startsWith("sync-payloads/")) {
    assertSyncObjectType(key.split("/")[3] ?? "");
  }
}

function assertKeyChecksum(key: string, expectedSha256: string): void {
  const keyChecksum = checksumFromKey(key);
  if (keyChecksum !== null && keyChecksum !== expectedSha256) {
    throw new StorageObjectError("r2_key_checksum_mismatch");
  }
}

function checksumFromKey(key: string): string | null {
  const segments = key.split("/");
  const prefix = segments[0];
  const lastSegment = segments[segments.length - 1];
  if (lastSegment === undefined) {
    return null;
  }

  if (prefix === "sync-payloads") {
    return lastSegment.slice(0, -".bin".length);
  }
  if (prefix === "plugin-packages") {
    return lastSegment.slice(0, -".rplug".length);
  }
  if (prefix === "plugin-assets" || prefix === "user-avatars" || prefix === "crash-attachments") {
    return lastSegment;
  }
  return null;
}

function assertRegion(value: string): void {
  if (!REGION.test(value)) {
    throw new StorageObjectError("region_invalid");
  }
}

function assertSegment(value: string, name: string): void {
  if (!SEGMENT.test(value)) {
    throw new StorageObjectError(`${name}_invalid`);
  }
}

function assertSyncObjectType(value: string): void {
  if (!SYNC_OBJECT_TYPES.has(value)) {
    throw new StorageObjectError("object_type_invalid");
  }
}

function assertSha256Hex(value: string, name: string): void {
  if (!SHA256_HEX.test(value)) {
    throw new StorageObjectError(`${name}_invalid`);
  }
}

async function sha256Hex(payload: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", payload);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function hexToArrayBuffer(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes.buffer;
}
