import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { recentDeviceActionProofBytes } from "../src/recent_device_action_proof.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  sessionDocument,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const IDEMPOTENCY_KEY = "sync-reset-000001";
const PAYLOAD_HASH = "a".repeat(64);
const USER_HASH = sha256(bytes(USER_ID));
const PAYLOAD_KEY = `sync-payloads/us-east/${USER_HASH}/tabs/tab-01/${PAYLOAD_HASH}.bin`;
const SNAPSHOT_KEY = `sync-snapshots/us-east/${USER_HASH}/snapshot-01.bin`;

describe("sync reset routes", () => {
  it("deletes cloud sync data and records an audit event", async () => {
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { signing_public_key: PUBLIC_KEY },
        null,
        resetCountsRow(),
      ],
      allRows: [{ r2_key: PAYLOAD_KEY }, { r2_key: SNAPSHOT_KEY }],
    });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody()),
      testEnv({
        d1,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    const body = (await response.json()) as {
      version: number;
      user_id: string;
      device_id: string;
      idempotency_key: string;
      reset_at: number;
      deleted: Record<string, number>;
    };
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(body.version, 1);
    assert.equal(body.user_id, USER_ID);
    assert.equal(body.device_id, DEVICE_ID);
    assert.equal(body.idempotency_key, IDEMPOTENCY_KEY);
    assert.ok(Number.isSafeInteger(body.reset_at));
    assert.deepEqual(body.deleted, {
      objects: 4,
      changes: 9,
      snapshots: 2,
      tombstones: 1,
      r2_objects: 2,
    });
    assert.deepEqual(r2Deletes, [PAYLOAD_KEY, SNAPSHOT_KEY]);
    assert.equal(d1.batches[0], 9);
    assert.ok(d1.queries[1]?.includes("signing_public_key"));
    assert.ok(d1.queries[2]?.includes("FROM audit_events"));
    assert.ok(d1.queries[3]?.includes("FROM sync_objects"));
    assert.ok(d1.queries[4]?.includes("FROM sync_r2_gc_candidates"));
    assert.deepEqual(d1.binds[2], [USER_ID, syncResetEventId()]);
    assert.deepEqual(d1.binds[3], [USER_ID, USER_ID, USER_ID, USER_ID]);
    assert.deepEqual(d1.binds[4], [USER_ID]);
    assert.ok(d1.queries[5]?.includes("CASE WHEN EXISTS"));
    assert.ok(d1.queries[6]?.includes("UPDATE sync_r2_gc_candidates"));
    assert.ok(d1.queries[7]?.includes("UPDATE sync_vault_rotations"));
    assert.ok(d1.queries[8]?.includes("DELETE FROM sync_change_log"));
    assert.ok(d1.queries[10]?.includes("DELETE FROM sync_snapshot_heads"));
    assert.ok(d1.queries[11]?.includes("DELETE FROM sync_snapshot_encryption"));
    assert.ok(d1.queries[12]?.includes("DELETE FROM sync_snapshots"));
    assert.equal(d1.queries.some((query) => query.includes("DELETE FROM sync_vault")), false);
    assert.deepEqual(d1.binds[5]?.slice(0, 6), [
      syncResetEventId(),
      USER_ID,
      DEVICE_ID,
      "sync.reset",
      "sync",
      USER_ID,
    ]);
    assert.equal(d1.binds[5]?.[11], PUBLIC_KEY);
  });

  it("returns an idempotent reset document for existing audit events", async () => {
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { signing_public_key: PUBLIC_KEY },
        { actor_device_id: DEVICE_ID, outcome: "success", created_at: 1_780_001_000 },
      ],
      allRows: [{ r2_key: PAYLOAD_KEY }],
    });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody()),
      testEnv({
        d1,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      reset_at: 1_780_001_000,
      deleted: { objects: 0, changes: 0, snapshots: 0, tombstones: 0, r2_objects: 0 },
    });
    assert.deepEqual(r2Deletes, [PAYLOAD_KEY]);
    assert.equal(d1.queries.length, 6);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects replay mismatches before deleting sync data", async () => {
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { signing_public_key: PUBLIC_KEY },
        { actor_device_id: "device-02", outcome: "success", created_at: 1_780_001_000 },
      ],
    });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody()),
      testEnv({
        d1,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_reset" });
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects missing confirmation before reset reads", async () => {
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody({ confirmation: "delete" })),
      testEnv({
        d1,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_reset" });
    assert.deepEqual(r2Deletes, []);
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects revoked devices before reading reset bodies", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null] });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody()),
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

  it("keeps reset successful when scheduled GC must handle a malformed legacy key", async () => {
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { signing_public_key: PUBLIC_KEY },
        null,
        resetCountsRow(),
      ],
      allRows: [{ r2_key: "sync-snapshots/../bad.bin" }],
    });

    const response = await handleRequest(
      syncResetRequest(await syncResetBody()),
      testEnv({
        d1,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(d1.batches, [9]);
  });
});

function syncResetRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/reset", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

async function syncResetBody(
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  const body: Record<string, unknown> = {
    version: 2,
    confirmation: "delete-cloud-sync-data",
    idempotency_key: IDEMPOTENCY_KEY,
    proof_created_at: Math.floor(Date.now() / 1000),
    ...overrides,
  };
  body.action_proof = await signDeviceMessage(recentDeviceActionProofBytes({
    action: "sync.reset",
    userId: USER_ID,
    sessionId: "session-01",
    deviceId: DEVICE_ID,
    confirmation: String(body.confirmation),
    idempotencyKey: String(body.idempotency_key),
    proofCreatedAt: Number(body.proof_created_at),
  }));
  return body;
}

function resetCountsRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return { objects: 4, changes: 9, snapshots: 2, tombstones: 1, ...overrides };
}

function syncResetEventId(): string {
  return `sync-reset:${USER_ID}:${IDEMPOTENCY_KEY}`;
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

function sha256(payload: ArrayBuffer): string {
  return createHash("sha256").update(new Uint8Array(payload)).digest("hex");
}
