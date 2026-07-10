import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  recentDeviceActionProofBytes,
  recentDeviceActionRequestHash,
} from "../src/recent_device_action_proof.js";
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
    const requestBody = await accountDeleteBody();
    const d1 = testD1Database({
      firstRows: [
        null,
        { signing_public_key: PUBLIC_KEY },
        deletionCountsRow(),
      ],
      allRowSets: [
        [{ r2_key: PAYLOAD_KEY }, { r2_key: SNAPSHOT_KEY }],
        [{ token: ACCESS_TOKEN }],
        [{ r2_key: PAYLOAD_KEY }, { r2_key: SNAPSHOT_KEY }],
      ],
    });

    const response = await handleRequest(
      accountDeleteRequest(requestBody),
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
    assert.equal(d1.batches[0], 18);
    assert.ok(d1.queries[0]?.includes("FROM audit_events"));
    assert.ok(d1.queries[1]?.includes("signing_public_key"));
    assert.ok(d1.queries[2]?.includes("FROM user_devices"));
    assert.ok(d1.queries[3]?.includes("FROM sync_r2_gc_candidates"));
    assert.ok(d1.queries[4]?.includes("FROM better_auth_session"));
    assert.ok(d1.queries[5]?.includes("CASE WHEN EXISTS"));
    assert.ok(d1.queries[6]?.includes("UPDATE sync_r2_gc_candidates"));
    assert.ok(d1.queries[7]?.includes("DELETE FROM sync_change_log"));
    assert.ok(d1.queries[9]?.includes("DELETE FROM sync_snapshot_heads"));
    assert.ok(d1.queries[10]?.includes("DELETE FROM sync_snapshot_encryption"));
    assert.ok(d1.queries[11]?.includes("DELETE FROM sync_snapshots"));
    assert.ok(d1.queries[14]?.includes("DELETE FROM sync_vault_accounts"));
    assert.ok(d1.queries[16]?.includes("DELETE FROM better_auth_session_device_context"));
    assert.ok(d1.queries[17]?.includes("DELETE FROM user_devices"));
    assert.deepEqual(d1.binds[0], [accountDeletionEventId()]);
    assert.deepEqual(d1.binds[3], [USER_ID]);
    assert.ok(d1.queries[22]?.includes("SET user_id = NULL"));
    assert.deepEqual(d1.binds[5]?.slice(0, 6), [
      accountDeletionEventId(),
      null,
      DEVICE_ID,
      "account.delete",
      "account",
      USER_HASH,
    ]);
    assert.equal(d1.binds[5]?.[11], PUBLIC_KEY);
    assert.equal(d1.binds[5]?.[12], await requestHash(requestBody));
    assert.equal(d1.binds[5]?.[13], body.deleted_at);
  });

  it("returns an idempotent deletion document for existing audit events", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const requestBody = await accountDeleteBody();
    const d1 = testD1Database({
      firstRows: [
        {
          actor_device_id: DEVICE_ID,
          outcome: "success",
          subject_id: USER_HASH,
          metadata_hash: await requestHash(requestBody),
          created_at: 1_780_001_000,
        },
      ],
      allRows: [{ r2_key: PAYLOAD_KEY }],
    });

    const response = await handleRequest(
      accountDeleteRequest(requestBody),
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
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("deletes every legacy KV session key for the account", async () => {
    const secondToken = "second-session-token-0000000000000000";
    const currentHash = await authTokenHash(ACCESS_TOKEN);
    const secondHash = await authTokenHash(secondToken);
    const currentKey = authSessionCacheKvKey("local", currentHash);
    const secondKey = authSessionCacheKvKey("local", secondHash);
    const kvDeletes: string[] = [];
    const d1 = testD1Database({
      firstRows: [
        null,
        { signing_public_key: PUBLIC_KEY },
        deletionCountsRow(),
      ],
      allRowSets: [[], [{ token: ACCESS_TOKEN }, { token: secondToken }], []],
    });

    const response = await handleRequest(
      accountDeleteRequest(await accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        kvEntries: [
          [currentKey, sessionDocument(DEVICE_ID)],
          [secondKey, sessionDocument("device-02")],
        ],
      }),
    );

    assert.equal(response.status, 200);
    const body = await response.json() as { deleted: { kv_session_cache: number } };
    assert.equal(body.deleted.kv_session_cache, 2);
    assert.deepEqual(kvDeletes.sort(), [currentKey, secondKey].sort());
  });

  it("rejects replay mismatches before deleting account data", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const requestBody = await accountDeleteBody();
    const d1 = testD1Database({
      firstRows: [
        {
          actor_device_id: "device-02",
          outcome: "success",
          subject_id: USER_HASH,
          metadata_hash: await requestHash(requestBody),
          created_at: 1_780_001_000,
        },
      ],
    });

    const response = await handleRequest(
      accountDeleteRequest(requestBody),
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
    const d1 = testD1Database([]);

    const response = await handleRequest(
      accountDeleteRequest(await accountDeleteBody({ confirmation: "delete" })),
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
    assert.equal(d1.queries.length, 0);
    assert.deepEqual(d1.batches, []);
  });

  it("requires an approved device key for a new account deletion", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null, null] });

    const response = await handleRequest(
      accountDeleteRequest(await accountDeleteBody()),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "account_deletion_forbidden" });
    assert.equal(d1.queries.length, 2);
    assert.deepEqual(d1.batches, []);
  });

  it("keeps account deletion successful when scheduled GC must handle a malformed key", async () => {
    const kvDeletes: string[] = [];
    const r2Deletes: string[] = [];
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        null,
        { signing_public_key: PUBLIC_KEY },
        deletionCountsRow(),
      ],
      allRowSets: [
        [{ r2_key: "sync-snapshots/../bad.bin" }],
        [],
        [{ r2_key: "sync-snapshots/../bad.bin" }],
      ],
    });

    const response = await handleRequest(
      accountDeleteRequest(await accountDeleteBody()),
      testEnv({
        d1,
        kvDeletes,
        r2Deletes,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(r2Deletes, []);
    assert.deepEqual(kvDeletes, [authSessionCacheKvKey("local", tokenHash)]);
    assert.deepEqual(d1.batches, [18]);
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

async function accountDeleteBody(
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  const body: Record<string, unknown> = {
    version: 2,
    confirmation: "delete-elydora-account",
    idempotency_key: IDEMPOTENCY_KEY,
    proof_created_at: Math.floor(Date.now() / 1000),
    ...overrides,
  };
  body.action_proof = await signDeviceMessage(recentDeviceActionProofBytes({
    action: "account.delete",
    userId: USER_ID,
    sessionId: "session-01",
    deviceId: DEVICE_ID,
    confirmation: String(body.confirmation),
    idempotencyKey: String(body.idempotency_key),
    proofCreatedAt: Number(body.proof_created_at),
  }));
  return body;
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

function requestHash(body: Record<string, unknown>): Promise<string> {
  return recentDeviceActionRequestHash({
    action: "account.delete",
    userId: USER_ID,
    sessionId: "session-01",
    deviceId: DEVICE_ID,
    confirmation: String(body.confirmation),
    idempotencyKey: String(body.idempotency_key),
    proofCreatedAt: Number(body.proof_created_at),
    actionProof: String(body.action_proof),
  });
}

function bytes(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function sha256(payload: Uint8Array): string {
  return createHash("sha256").update(payload).digest("hex");
}
