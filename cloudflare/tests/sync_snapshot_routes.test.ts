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
const SNAPSHOT_ID = "snapshot-01";
const REGION = "us-east";

describe("sync snapshot routes", () => {
  it("uploads an encrypted snapshot from an approved current device", async () => {
    const payload = bytes("encrypted snapshot payload");
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        null,
        snapshotRow({ r2_key: key, payload_hash: payloadHash, size_bytes: payload.byteLength }),
      ],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(
        syncSnapshotBody({ payload_hash: payloadHash, data_base64: base64(payload) }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      snapshot: snapshotDocument({
        r2_key: key,
        payload_hash: payloadHash,
        size_bytes: payload.byteLength,
      }),
    });
    assert.equal(r2Puts.length, 1);
    assert.equal(r2Puts[0]?.key, key);
    assert.deepEqual(new Uint8Array(r2Puts[0]?.payload ?? new ArrayBuffer(0)), new Uint8Array(payload));
    assert.equal(r2Puts[0]?.options.customMetadata?.sha256, payloadHash);
    assert.equal(d1.batches[0], 1);
    assert.ok(d1.queries[1]?.includes("FROM sync_snapshots"));
    assert.ok(d1.queries[2]?.includes("INSERT INTO sync_snapshots"));
    assert.deepEqual(d1.binds[2]?.slice(0, 4), [USER_ID, SNAPSHOT_ID, key, payloadHash]);
  });

  it("downloads an encrypted snapshot with R2 checksum verification", async () => {
    const payload = bytes("encrypted snapshot payload");
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const r2Gets: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        snapshotRow({ r2_key: key, payload_hash: payloadHash, size_bytes: payload.byteLength }),
      ],
    });

    const response = await handleRequest(
      syncSnapshotGetRequest(),
      testEnv({
        d1,
        r2Gets,
        r2Objects: [[key, payload]],
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      snapshot: snapshotDocument({
        r2_key: key,
        payload_hash: payloadHash,
        size_bytes: payload.byteLength,
      }),
      data_base64: base64(payload),
    });
    assert.deepEqual(r2Gets, [key]);
  });

  it("returns not found for missing snapshot indexes", async () => {
    const r2Gets: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }, null] });

    const response = await handleRequest(
      syncSnapshotGetRequest(),
      testEnv({
        d1,
        r2Gets,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_not_found" });
    assert.deepEqual(r2Gets, []);
  });

  it("rejects snapshot checksum mismatches before D1 writes", async () => {
    const payload = bytes("encrypted snapshot payload");
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      syncSnapshotPostRequest(
        syncSnapshotBody({ payload_hash: "c".repeat(64), data_base64: base64(payload) }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_snapshot" });
    assert.equal(r2Puts.length, 0);
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects stale snapshot clocks before R2 writes", async () => {
    const payload = bytes("encrypted snapshot payload");
    const payloadHash = sha256(payload);
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        snapshotRow({ payload_hash: "d".repeat(64), logical_clock: 43 }),
      ],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(
        syncSnapshotBody({
          payload_hash: payloadHash,
          logical_clock: 42,
          data_base64: base64(payload),
        }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_conflict" });
    assert.equal(r2Puts.length, 0);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects same-clock snapshot write races after D1 persistence", async () => {
    const payload = bytes("encrypted snapshot payload");
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        null,
        snapshotRow({ r2_key: key, payload_hash: "e".repeat(64), size_bytes: payload.byteLength }),
      ],
    });

    const response = await handleRequest(
      syncSnapshotPostRequest(
        syncSnapshotBody({ payload_hash: payloadHash, data_base64: base64(payload) }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_conflict" });
    assert.equal(r2Puts.length, 1);
    assert.equal(d1.batches[0], 1);
  });

  it("rejects revoked devices before reading snapshot payloads", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null] });

    const response = await handleRequest(
      syncSnapshotPostRequest(syncSnapshotBody()),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_not_approved" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("fails closed when a stored snapshot payload fails checksum verification", async () => {
    const payload = bytes("encrypted snapshot payload");
    const payloadHash = sha256(payload);
    const key = snapshotKey(payloadHash);
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        snapshotRow({ r2_key: key, payload_hash: payloadHash, size_bytes: payload.byteLength }),
      ],
    });

    const response = await handleRequest(
      syncSnapshotGetRequest(),
      testEnv({
        d1,
        r2Objects: [[key, bytes("corrupt snapshot payload")]],
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "sync_snapshot_failed" });
  });
});

function syncSnapshotPostRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/snapshot", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function syncSnapshotGetRequest(): Request {
  return new Request(`https://elydora.test/api/sync/snapshot?snapshot_id=${SNAPSHOT_ID}`, {
    headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
  });
}

function syncSnapshotBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const payload = bytes("encrypted snapshot payload");
  const payloadHash = sha256(payload);
  return {
    version: 1,
    snapshot_id: SNAPSHOT_ID,
    region: REGION,
    payload_hash: payloadHash,
    schema_rev: 1,
    logical_clock: 42,
    data_base64: base64(payload),
    ...overrides,
  };
}

function snapshotRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    snapshot_id: SNAPSHOT_ID,
    r2_key: snapshotKey("a".repeat(64)),
    payload_hash: "a".repeat(64),
    schema_rev: 1,
    logical_clock: 42,
    device_id: DEVICE_ID,
    size_bytes: 26,
    created_at: 1_780_000_900,
    ...overrides,
  };
}

function snapshotDocument(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return snapshotRow(overrides);
}

function snapshotKey(_payloadHash: string): string {
  return `sync-snapshots/${REGION}/${sha256(bytes(USER_ID))}/${SNAPSHOT_ID}.bin`;
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

function base64(payload: ArrayBuffer): string {
  return Buffer.from(payload).toString("base64");
}

function sha256(payload: ArrayBuffer): string {
  return createHash("sha256").update(new Uint8Array(payload)).digest("hex");
}
