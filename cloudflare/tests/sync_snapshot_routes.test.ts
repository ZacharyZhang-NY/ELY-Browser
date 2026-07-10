import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  type RecordedR2Put,
  sessionDocument,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const SNAPSHOT_ID = "device-01";
const REGION = "us-east";
const KEY_ID = "1".repeat(64);
const CONTENT_HASH = "2".repeat(64);

describe("sync snapshot routes", () => {
  it("commits a genesis encrypted snapshot as the global head", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const row = snapshotRow({
      r2_key: snapshotKey(payloadHash),
      payload_hash: payloadHash,
      size_bytes: payload.byteLength,
    });
    const r2Puts: RecordedR2Put[] = [];
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, null, vaultKeyRow()],
      batchRowSets: [[[], [], [], [], [row]]],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        payload_hash: payloadHash,
        data_base64: base64(payload),
      })),
      await authorizedEnv(d1, { r2Puts }),
    );

    assert.equal(response.status, 201);
    assert.deepEqual(await response.json(), uploadDocument(row));
    assert.equal(r2Puts.length, 1);
    assert.equal(d1.batches[0], 5);
    assert.ok(d1.queries.some((query) => query.includes("INSERT INTO sync_snapshot_heads")));
  });

  it("returns the original success document for an exact replay", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const row = snapshotRow({
      r2_key: snapshotKey(payloadHash),
      payload_hash: payloadHash,
      size_bytes: payload.byteLength,
    });
    const r2Puts: RecordedR2Put[] = [];
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, row, vaultKeyRow()],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        payload_hash: payloadHash,
        data_base64: base64(payload),
      })),
      await authorizedEnv(d1, { r2Puts }),
    );

    assert.equal(response.status, 201);
    assert.deepEqual(await response.json(), uploadDocument(row));
    assert.deepEqual(d1.batches, []);
    assert.equal(r2Puts.length, 0);
    assert.ok(d1.queries.some((query) => query.includes("SET cleanup_snapshot_id = ?")));
  });

  it("returns the original success after the vault rotates", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const row = snapshotRow({
      r2_key: snapshotKey(payloadHash),
      payload_hash: payloadHash,
      size_bytes: payload.byteLength,
    });
    const r2Puts: RecordedR2Put[] = [];
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        row,
        { key_id: "f".repeat(64), generation: 2 },
      ],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        payload_hash: payloadHash,
        data_base64: base64(payload),
      })),
      await authorizedEnv(d1, { r2Puts }),
    );

    assert.equal(response.status, 201);
    assert.deepEqual(await response.json(), uploadDocument(row));
    assert.equal(r2Puts.length, 0);
    assert.equal(d1.queries.some((query) => query.includes("SET cleanup_snapshot_id = ?")), false);
  });

  it("returns the committed document when an identical concurrent writer wins", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const baseRow = snapshotRow({ payload_hash: "a".repeat(64) });
    const committed = snapshotRow({
      payload_hash: payloadHash,
      r2_key: snapshotKey(payloadHash),
      content_hash: CONTENT_HASH,
      logical_clock: 43,
      head_revision: 2,
      base_head_revision: 1,
      base_snapshot_id: SNAPSHOT_ID,
      base_payload_hash: "a".repeat(64),
      size_bytes: payload.byteLength,
    });
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, baseRow, vaultKeyRow(), committed],
      batchError: new Error("sync_snapshot_head_cas_failed"),
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        payload_hash: payloadHash,
        logical_clock: 43,
        head_revision: 2,
        base_head: headRef(baseRow),
        data_base64: base64(payload),
      })),
      await authorizedEnv(d1),
    );

    assert.equal(response.status, 201);
    assert.deepEqual(await response.json(), uploadDocument(committed));
    assert.deepEqual(d1.sessionConstraints, ["first-primary", "first-primary"]);
  });

  it("rejects a stale base before writing R2", async () => {
    const current = snapshotRow({ payload_hash: "a".repeat(64) });
    const staleBase = headRef({ payload_hash: "b".repeat(64) });
    const r2Puts: RecordedR2Put[] = [];
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, current, vaultKeyRow()],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        head_revision: 2,
        base_head: staleBase,
        logical_clock: 43,
      })),
      await authorizedEnv(d1, { r2Puts }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), conflictDocument(current));
    assert.equal(r2Puts.length, 0);
    assert.deepEqual(d1.batches, []);
  });

  it("returns the winning head when D1 rejects a concurrent writer", async () => {
    const baseRow = snapshotRow({ payload_hash: "a".repeat(64) });
    const winner = snapshotRow({
      payload_hash: "b".repeat(64),
      r2_key: snapshotKey("b".repeat(64)),
      content_hash: "3".repeat(64),
      logical_clock: 43,
      head_revision: 2,
      base_head_revision: 1,
      base_snapshot_id: SNAPSHOT_ID,
      base_payload_hash: "a".repeat(64),
    });
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, baseRow, vaultKeyRow(), winner],
      batchError: new Error("sync_snapshot_head_cas_failed"),
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({
        head_revision: 2,
        base_head: headRef(baseRow),
        logical_clock: 44,
      })),
      await authorizedEnv(d1),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), conflictDocument(winner));
  });

  it("downloads a snapshot only through its exact head token", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const row = snapshotRow({
      r2_key: key,
      payload_hash: payloadHash,
      size_bytes: payload.byteLength,
    });
    const r2Gets: string[] = [];
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }, row] });

    const response = await handleRequest(
      syncSnapshotGetRequest(headRef(row)),
      await authorizedEnv(d1, { r2Gets, r2Objects: [[key, payload]] }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      ...uploadDocument(row),
      data_base64: base64(payload),
    });
    assert.deepEqual(r2Gets, [key]);
  });

  it("returns the current head for a historical different-device token", async () => {
    const current = snapshotRow({
      snapshot_id: "device-02",
      payload_hash: "b".repeat(64),
      r2_key: snapshotKey("b".repeat(64)),
      head_revision: 2,
      base_head_revision: 1,
      base_snapshot_id: SNAPSHOT_ID,
      base_payload_hash: "a".repeat(64),
    });
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, null, current],
    });

    const response = await handleRequest(
      syncSnapshotGetRequest(headRef({ payload_hash: "a".repeat(64) })),
      await authorizedEnv(d1),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), conflictDocument(current));
  });

  it("returns a new head when cleanup removes payload after token validation", async () => {
    const old = snapshotRow();
    const current = snapshotRow({
      snapshot_id: "device-02",
      payload_hash: "b".repeat(64),
      r2_key: snapshotKey("b".repeat(64)),
      head_revision: 2,
      base_head_revision: 1,
      base_snapshot_id: SNAPSHOT_ID,
      base_payload_hash: "a".repeat(64),
      logical_clock: 43,
    });
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, old, current],
    });

    const response = await handleRequest(
      syncSnapshotGetRequest(headRef(old)),
      await authorizedEnv(d1),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), conflictDocument(current));
    assert.deepEqual(d1.sessionConstraints, ["first-primary", "first-primary"]);
  });

  it("preserves legacy encryption metadata on exact downloads", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const row = snapshotRow({
      r2_key: key,
      payload_hash: payloadHash,
      encryption_version: 1,
      size_bytes: payload.byteLength,
    });
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }, row] });

    const response = await handleRequest(
      syncSnapshotGetRequest(headRef(row)),
      await authorizedEnv(d1, { r2Objects: [[key, payload]] }),
    );

    assert.equal(response.status, 200);
    const document = await response.json() as { version: number; snapshot: { encryption_version: number } };
    assert.equal(document.version, 3);
    assert.equal(document.snapshot.encryption_version, 1);
  });

  it("rejects upload wire version 2 before R2 and D1 writes", async () => {
    const r2Puts: RecordedR2Put[] = [];
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody({ version: 2 })),
      await authorizedEnv(d1, { r2Puts }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_snapshot" });
    assert.equal(r2Puts.length, 0);
    assert.deepEqual(d1.batches, []);
  });

  it("fails closed when the committed head SELECT is empty", async () => {
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, null, vaultKeyRow()],
      batchRowSets: [[[], [], [], [], []]],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody()),
      await authorizedEnv(d1),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_failed" });
  });

  it("fails closed when stored ciphertext fails checksum verification", async () => {
    const payload = opaqueEnvelopeBytes();
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const row = snapshotRow({
      r2_key: key,
      payload_hash: payloadHash,
      size_bytes: payload.byteLength,
    });
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }, row, row] });

    const response = await handleRequest(
      syncSnapshotGetRequest(headRef(row)),
      await authorizedEnv(d1, { r2Objects: [[key, bytes("corrupt")]] }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_failed" });
  });

});

function syncSnapshotPostRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/snapshot", {
    method: "POST",
    headers: { authorization: `Bearer ${ACCESS_TOKEN}`, "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

function syncSnapshotGetRequest(head: Record<string, unknown>): Request {
  const query = new URLSearchParams({
    snapshot_id: String(head.snapshot_id),
    head_revision: String(head.revision),
    payload_hash: String(head.payload_hash),
  });
  return new Request(`https://elydora.test/api/sync/snapshot?${query}`, {
    headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
  });
}

function syncSnapshotBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const payload = opaqueEnvelopeBytes();
  return {
    version: 3,
    snapshot_id: SNAPSHOT_ID,
    region: REGION,
    payload_hash: sha256(payload),
    encryption_version: 2,
    vault_generation: 1,
    key_id: KEY_ID,
    content_hash: CONTENT_HASH,
    schema_rev: 1,
    logical_clock: 42,
    head_revision: 1,
    base_head: null,
    data_base64: base64(payload),
    ...overrides,
  };
}

function snapshotRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    snapshot_id: SNAPSHOT_ID,
    r2_key: snapshotKey("a".repeat(64)),
    payload_hash: "a".repeat(64),
    encryption_version: 2,
    vault_generation: 1,
    key_id: KEY_ID,
    content_hash: CONTENT_HASH,
    schema_rev: 1,
    logical_clock: 42,
    head_revision: 1,
    base_head_revision: null,
    base_snapshot_id: null,
    base_payload_hash: null,
    device_id: DEVICE_ID,
    size_bytes: 11,
    created_at: 1_780_000_900,
    ...overrides,
  };
}

function snapshotDocument(row: Record<string, unknown>): Record<string, unknown> {
  const {
    base_head_revision: revision,
    base_snapshot_id: snapshotId,
    base_payload_hash: payloadHash,
    ...document
  } = row;
  return {
    ...document,
    base_head: revision === null
      ? null
      : { revision, snapshot_id: snapshotId, payload_hash: payloadHash },
  };
}

function uploadDocument(row: Record<string, unknown>): Record<string, unknown> {
  return {
    version: 3,
    user_id: USER_ID,
    device_id: DEVICE_ID,
    snapshot: snapshotDocument(row),
  };
}

function conflictDocument(row: Record<string, unknown>): Record<string, unknown> {
  return {
    version: 1,
    error: "sync_snapshot_head_conflict",
    current_head: snapshotDocument(row),
  };
}

function headRef(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    revision: overrides.head_revision ?? 1,
    snapshot_id: overrides.snapshot_id ?? SNAPSHOT_ID,
    payload_hash: overrides.payload_hash ?? "a".repeat(64),
  };
}

function vaultKeyRow(): Record<string, unknown> {
  return { key_id: KEY_ID, generation: 1 };
}

async function authorizedEnv(
  d1: ReturnType<typeof testD1Database>,
  options: Omit<Parameters<typeof testEnv>[0], "d1" | "kvEntries"> = {},
): Promise<ReturnType<typeof testEnv>> {
  const tokenHash = await authTokenHash(ACCESS_TOKEN);
  return testEnv({
    ...options,
    d1,
    kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
  });
}

function snapshotKey(payloadHash: string): string {
  return `sync-snapshots/${REGION}/${sha256(bytes(USER_ID))}/${SNAPSHOT_ID}/${payloadHash}.bin`;
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

function opaqueEnvelopeBytes(): ArrayBuffer {
  return new Uint8Array([0x45, 0x4c, 0x59, 0x53, 0x59, 0x4e, 0x43, 0x00, 0xff, 0x80, 0x01]).buffer;
}

function base64(payload: ArrayBuffer): string {
  return Buffer.from(payload).toString("base64");
}

function sha256(payload: ArrayBuffer): string {
  return createHash("sha256").update(new Uint8Array(payload)).digest("hex");
}
