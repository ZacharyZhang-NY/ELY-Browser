import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { ACCESS_TOKEN, sessionDocument, testD1Database, testEnv } from "./devices_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const IDEMPOTENCY_KEY = "account-delete-000001";
const PAYLOAD_HASH = "b".repeat(64);
const USER_HASH = sha256(bytes(USER_ID));
const IDEMPOTENCY_HASH = sha256(bytes(IDEMPOTENCY_KEY));
const PAYLOAD_KEY = `sync-payloads/us-east/${USER_HASH}/tabs/tab-01/${PAYLOAD_HASH}.bin`;
const SNAPSHOT_KEY = `sync-snapshots/us-east/${USER_HASH}/snapshot-01.bin`;

describe("account deletion routes", () => {
  it("deletes account data and revokes the current auth session cache", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const sessionCacheKey = authSessionCacheKvKey("local", tokenHash);
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, null, deletionCountsRow()],
      allRows: [{ r2_key: PAYLOAD_KEY }, { r2_key: SNAPSHOT_KEY }],
    });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[sessionCacheKey, sessionDocument(DEVICE_ID)]],
      }),
    );

    const body = (await response.json()) as {
      version: number;
      account_hash: string;
      device_id: string;
      idempotency_key: string;
      deleted_at: number;
      deleted: Record<string, number>;
    };
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(body.version, 1);
    assert.equal(body.account_hash, USER_HASH);
    assert.equal(body.device_id, DEVICE_ID);
    assert.equal(body.idempotency_key, IDEMPOTENCY_KEY);
    assert.ok(Number.isSafeInteger(body.deleted_at));
    assert.deepEqual(body.deleted, {
      devices: 2,
      approvals: 3,
      sync_objects: 4,
      sync_changes: 9,
      sync_snapshots: 2,
      sync_tombstones: 1,
      audit_events: 7,
      session_device_contexts: 1,
      sessions: 2,
      accounts: 1,
      users: 1,
      r2_objects: 2,
      kv_session_cache: 1,
    });
    assert.deepEqual(r2Deletes, [PAYLOAD_KEY, SNAPSHOT_KEY]);
    assert.deepEqual(kvDeletes, [sessionCacheKey]);
    assert.equal(d1.batches[0], 12);
    assert.ok(d1.queries[1]?.includes("FROM audit_events"));
    assert.ok(d1.queries[2]?.includes("FROM user_devices"));
    assert.ok(d1.queries[3]?.includes("UNION"));
    assert.ok(d1.queries[4]?.includes("DELETE FROM sync_change_log"));
    assert.ok(d1.queries[9]?.includes("DELETE FROM better_auth_session_device_context"));
    assert.ok(d1.queries[10]?.includes("DELETE FROM user_devices"));
    assert.ok(d1.queries[15]?.includes("INSERT INTO audit_events"));
    assert.deepEqual(d1.binds[1], [accountDeletionEventId()]);
    assert.deepEqual(d1.binds[3], [USER_ID, USER_ID]);
    assert.deepEqual(d1.binds[15], [
      accountDeletionEventId(),
      DEVICE_ID,
      USER_HASH,
      IDEMPOTENCY_HASH,
      body.deleted_at,
    ]);
  });

  it("returns an idempotent deletion document for existing audit events", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        {
          actor_device_id: DEVICE_ID,
          outcome: "success",
          subject_id: USER_HASH,
          created_at: 1_780_001_000,
        },
      ],
      allRows: [{ r2_key: PAYLOAD_KEY }],
    });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      account_hash: USER_HASH,
      device_id: DEVICE_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      deleted_at: 1_780_001_000,
      deleted: {
        devices: 0,
        approvals: 0,
        sync_objects: 0,
        sync_changes: 0,
        sync_snapshots: 0,
        sync_tombstones: 0,
        audit_events: 0,
        session_device_contexts: 0,
        sessions: 0,
        accounts: 0,
        users: 0,
        r2_objects: 0,
        kv_session_cache: 0,
      },
    });
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(kvDeletes, []);
    assert.equal(d1.queries.length, 2);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects replay mismatches before deleting account data", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        {
          actor_device_id: "device-02",
          outcome: "success",
          subject_id: USER_HASH,
          created_at: 1_780_001_000,
        },
      ],
    });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_account_deletion" });
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(kvDeletes, []);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects missing confirmation before account deletion reads", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody({ confirmation: "delete" })),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_account_deletion" });
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(kvDeletes, []);
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects revoked devices before reading account deletion bodies", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null] });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody()),
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

  it("fails closed when stored R2 keys are malformed", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, null, deletionCountsRow()],
      allRows: [{ r2_key: "sync-snapshots/../bad.bin" }],
    });

    const response = await handleRequest(
      accountDeleteRequest(accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "account_deletion_failed" });
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(kvDeletes, []);
    assert.deepEqual(d1.batches, []);
  });
});

function accountDeleteRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/account/delete", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function accountDeleteBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 1,
    confirmation: "delete-elydora-account",
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function deletionCountsRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    devices: 2,
    approvals: 3,
    sync_objects: 4,
    sync_changes: 9,
    sync_snapshots: 2,
    sync_tombstones: 1,
    audit_events: 7,
    session_device_contexts: 1,
    sessions: 2,
    accounts: 1,
    users: 1,
    ...overrides,
  };
}

function accountDeletionEventId(): string {
  return `account-delete:${USER_HASH}:${IDEMPOTENCY_HASH}`;
}

function bytes(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function sha256(payload: Uint8Array): string {
  return createHash("sha256").update(payload).digest("hex");
}
