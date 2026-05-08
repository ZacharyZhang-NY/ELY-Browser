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
const OBJECT_ID = "tab-01";
const OBJECT_TYPE = "tabs";

describe("sync push routes", () => {
  it("pushes an inline encrypted sync object from an approved current device", async () => {
    const payload = bytes("encrypted tab payload");
    const payloadHash = sha256(payload);
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        null,
        syncObjectRow({ payload_hash: payloadHash }),
      ],
    });

    const response = await handleRequest(
      syncPushRequest(syncPushBody({ payload_hash: payloadHash, payload: inlinePayload(payload) })),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      object: syncObjectDocument({ payload_hash: payloadHash }),
    });
    assert.equal(d1.batches[0], 2);
    assert.ok(d1.queries[0]?.includes("approval_status = 'approved'"));
    assert.ok(d1.queries[1]?.includes("FROM sync_objects"));
    assert.ok(d1.queries[2]?.includes("INSERT INTO sync_objects"));
    assert.ok(d1.queries[3]?.includes("INSERT INTO sync_change_log"));
    assert.deepEqual(d1.binds[0], [USER_ID, DEVICE_ID]);
    assert.deepEqual(d1.binds[1], [USER_ID, OBJECT_ID]);
    assert.deepEqual(d1.binds[2]?.slice(0, 3), [USER_ID, OBJECT_ID, OBJECT_TYPE]);
    assert.deepEqual(new Uint8Array(d1.binds[2]?.[3] as ArrayBuffer), new Uint8Array(payload));
    assert.equal(d1.binds[2]?.[4], null);
    assert.equal(d1.binds[3]?.[3], "upsert");
  });

  it("pushes an R2 encrypted sync object after checksum verification", async () => {
    const payload = bytes("large encrypted tab payload");
    const payloadHash = sha256(payload);
    const userHash = sha256(bytes(USER_ID));
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        null,
        syncObjectRow({
          payload_hash: payloadHash,
          payload_r2_key: `sync-payloads/us-east/${userHash}/tabs/${OBJECT_ID}/${payloadHash}.bin`,
        }),
      ],
    });

    const response = await handleRequest(
      syncPushRequest(
        syncPushBody({ payload_hash: payloadHash, payload: r2Payload("us-east", payload) }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(r2Puts.length, 1);
    assert.equal(
      r2Puts[0]?.key,
      `sync-payloads/us-east/${userHash}/tabs/${OBJECT_ID}/${payloadHash}.bin`,
    );
    assert.equal(r2Puts[0]?.options.customMetadata?.sha256, payloadHash);
    const body = (await response.json()) as { object: { payload_storage: string } };
    assert.equal(body.object.payload_storage, "r2");
    assert.equal(d1.binds[2]?.[3], null);
    assert.equal(d1.binds[2]?.[4], r2Puts[0]?.key);
  });

  it("pushes a delete tombstone and writes the change log", async () => {
    const payloadHash = "b".repeat(64);
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        syncObjectRow({ payload_hash: "a".repeat(64), logical_clock: 41 }),
        syncObjectRow({ payload_hash: payloadHash, logical_clock: 42, deleted_at: 1_780_000_800 }),
      ],
    });

    const response = await handleRequest(
      syncPushRequest(
        syncPushBody({ operation: "delete", payload_hash: payloadHash, payload: undefined }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(d1.batches[0], 3);
    assert.ok(d1.queries[4]?.includes("INSERT INTO sync_tombstones"));
    assert.equal(d1.binds[2]?.[3], null);
    assert.equal(d1.binds[2]?.[4], null);
    assert.equal(d1.binds[3]?.[3], "delete");
    const body = (await response.json()) as { object: { payload_storage: string } };
    assert.equal(body.object.payload_storage, "tombstone");
  });

  it("rejects payload checksum mismatches before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });
    const response = await handleRequest(
      syncPushRequest(
        syncPushBody({
          payload_hash: "c".repeat(64),
          payload: inlinePayload(bytes("encrypted tab payload")),
        }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_push" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects R2 object ids that cannot form storage keys before D1 writes", async () => {
    const payload = bytes("large encrypted tab payload");
    const payloadHash = sha256(payload);
    const r2Puts: RecordedR2Put[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      syncPushRequest(
        syncPushBody({
          object_id: "Tab:01",
          payload_hash: payloadHash,
          payload: r2Payload("us-east", payload),
        }),
      ),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_push" });
    assert.equal(r2Puts.length, 0);
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects stale logical clocks before persistence writes", async () => {
    const payload = bytes("encrypted tab payload");
    const payloadHash = sha256(payload);
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        syncObjectRow({ payload_hash: "d".repeat(64), logical_clock: 43 }),
      ],
    });

    const response = await handleRequest(
      syncPushRequest(
        syncPushBody({
          payload_hash: payloadHash,
          logical_clock: 42,
          payload: inlinePayload(payload),
        }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "sync_conflict" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects same-clock object write races after D1 persistence", async () => {
    const payload = bytes("encrypted tab payload");
    const payloadHash = sha256(payload);
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        null,
        syncObjectRow({ payload_hash: "e".repeat(64), logical_clock: 42 }),
      ],
    });

    const response = await handleRequest(
      syncPushRequest(syncPushBody({ payload_hash: payloadHash, payload: inlinePayload(payload) })),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "sync_conflict" });
    assert.equal(d1.batches[0], 2);
  });

  it("rejects revoked devices before reading the sync push body", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null] });
    const response = await handleRequest(
      syncPushRequest(syncPushBody()),
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
});

function syncPushRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/push", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function syncPushBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const payload = bytes("encrypted tab payload");
  const payloadHash = sha256(payload);
  return {
    version: 1,
    object_id: OBJECT_ID,
    object_type: OBJECT_TYPE,
    operation: "upsert",
    payload_hash: payloadHash,
    schema_rev: 1,
    logical_clock: 42,
    payload: inlinePayload(payload),
    ...overrides,
  };
}

function inlinePayload(payload: ArrayBuffer): Record<string, unknown> {
  return { kind: "inline", data_base64: base64(payload) };
}

function r2Payload(region: string, payload: ArrayBuffer): Record<string, unknown> {
  return { kind: "r2", region, data_base64: base64(payload) };
}

function syncObjectRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    object_id: OBJECT_ID,
    object_type: OBJECT_TYPE,
    payload_r2_key: null,
    payload_hash: "a".repeat(64),
    schema_rev: 1,
    logical_clock: 42,
    device_id: DEVICE_ID,
    created_at: 1_780_000_700,
    updated_at: 1_780_000_700,
    deleted_at: null,
    ...overrides,
  };
}

function syncObjectDocument(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const row = syncObjectRow(overrides);
  return {
    object_id: row.object_id,
    object_type: row.object_type,
    operation: row.deleted_at === null ? "upsert" : "delete",
    payload_hash: row.payload_hash,
    schema_rev: row.schema_rev,
    logical_clock: row.logical_clock,
    device_id: row.device_id,
    created_at: row.created_at,
    updated_at: row.updated_at,
    deleted_at: row.deleted_at,
    payload_storage:
      row.deleted_at !== null ? "tombstone" : row.payload_r2_key === null ? "inline" : "r2",
    payload_r2_key: row.payload_r2_key,
  };
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
