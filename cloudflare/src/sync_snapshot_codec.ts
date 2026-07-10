const SNAPSHOT_ID_PATTERN = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const DEVICE_ID_PATTERN = /^[a-zA-Z0-9._:-]{3,128}$/;
const SHA256_HEX = /^[a-f0-9]{64}$/;
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const REGION = /^[a-z0-9][a-z0-9-]{1,31}$/;

export type SnapshotRequestBody = Record<string, unknown>;

export class SyncSnapshotRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SyncSnapshotRequestError";
  }
}

export async function requestBody(request: Request): Promise<SnapshotRequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new SyncSnapshotRequestError("json_invalid");
  }
  return record(value, "body");
}

export function assertOnlyFields(value: SnapshotRequestBody, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new SyncSnapshotRequestError(`unexpected_field:${field}`);
    }
  }
}

export function assertOnlyQueryParams(url: URL, fields: string[]): void {
  const allowed = new Set(fields);
  for (const field of url.searchParams.keys()) {
    if (!allowed.has(field)) {
      throw new SyncSnapshotRequestError(`unexpected_query:${field}`);
    }
  }
}

export function snapshotIdValue(value: unknown): string {
  if (typeof value !== "string" || !SNAPSHOT_ID_PATTERN.test(value)) {
    throw new SyncSnapshotRequestError("snapshot_id_invalid");
  }
  return value;
}

export function deviceIdValue(value: unknown): string {
  if (typeof value !== "string" || !DEVICE_ID_PATTERN.test(value)) {
    throw new SyncSnapshotRequestError("device_id_invalid");
  }
  return value;
}

export function regionValue(value: unknown): string {
  if (typeof value !== "string" || !REGION.test(value)) {
    throw new SyncSnapshotRequestError("region_invalid");
  }
  return value;
}

export function sha256HexValue(value: unknown, label: string): string {
  if (typeof value !== "string" || !SHA256_HEX.test(value)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

export function integer(value: unknown, label: string, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

export function exactInteger<T extends number>(value: unknown, label: string, expected: T): T {
  if (value !== expected) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return expected;
}

export function text(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value;
}

export function payloadBytes(value: unknown, label: string, maxBytes: number): ArrayBuffer {
  const encoded = text(value, label);
  const maxEncodedLength = 4 * Math.ceil(maxBytes / 3);
  if (encoded.length > maxEncodedLength) {
    throw new SyncSnapshotRequestError(`${label}_size_invalid`);
  }
  if (!BASE64.test(encoded)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  const bytes = bytesFromBase64(encoded);
  if (bytes.byteLength === 0 || bytes.byteLength > maxBytes) {
    throw new SyncSnapshotRequestError(`${label}_size_invalid`);
  }
  return bytes;
}

export function base64FromBytes(payload: ArrayBuffer): string {
  const bytes = new Uint8Array(payload);
  const parts: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    parts.push(String.fromCharCode(...bytes.subarray(offset, offset + 0x8000)));
  }
  return btoa(parts.join(""));
}

export function arrayBufferFromBytes(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

export async function assertPayloadHash(
  payload: ArrayBuffer,
  expectedHash: string,
): Promise<void> {
  const actualHash = await sha256Hex(payload);
  if (actualHash !== expectedHash) {
    throw new SyncSnapshotRequestError("payload_hash_mismatch");
  }
}

export async function sha256Hex(payload: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", payload);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function record(value: unknown, label: string): SnapshotRequestBody {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new SyncSnapshotRequestError(`${label}_invalid`);
  }
  return value as SnapshotRequestBody;
}

function bytesFromBase64(value: string): ArrayBuffer {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes.buffer;
}
