import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { ACCESS_TOKEN, sessionDocument, testD1Database, testEnv } from "./devices_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";

describe("sync status routes", () => {
  it("returns cloud sync cursor, object, snapshot, and device status", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { latest_change_id: 51, total_changes: 7 },
        { total_snapshots: 2 },
        latestSnapshotRow(),
        { approved_devices: 3 },
      ],
      allRows: [
        objectStatusRow({ object_type: "bookmarks", active_count: 4, deleted_count: 1 }),
        objectStatusRow({ object_type: "tabs", active_count: 9, latest_logical_clock: 44 }),
      ],
    });

    const response = await handleRequest(
      syncStatusRequest(),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      cursor: { latest_change_id: 51, total_changes: 7 },
      objects: [
        objectStatusDocument({ object_type: "bookmarks", active_count: 4, deleted_count: 1 }),
        objectStatusDocument({ object_type: "tabs", active_count: 9, latest_logical_clock: 44 }),
      ],
      snapshots: {
        total_snapshots: 2,
        latest: latestSnapshotRow(),
      },
      devices: {
        approved_count: 3,
        current_device_id: DEVICE_ID,
        current_device_approved: true,
      },
    });
    assert.ok(d1.queries[0]?.includes("approval_status = 'approved'"));
    assert.ok(d1.queries[1]?.includes("FROM sync_change_log"));
    assert.ok(d1.queries[2]?.includes("FROM sync_objects"));
    assert.ok(d1.queries[3]?.includes("FROM sync_snapshots"));
    assert.ok(d1.queries[4]?.includes("FROM sync_snapshots"));
    assert.ok(d1.queries[5]?.includes("FROM user_devices"));
    assert.deepEqual(d1.binds, [
      [USER_ID, DEVICE_ID],
      [USER_ID],
      [USER_ID],
      [USER_ID],
      [USER_ID],
      [USER_ID],
    ]);
  });

  it("returns empty status when the account has no sync facts", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { latest_change_id: 0, total_changes: 0 },
        { total_snapshots: 0 },
        null,
        { approved_devices: 1 },
      ],
    });

    const response = await handleRequest(
      syncStatusRequest(),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    const body = (await response.json()) as {
      cursor: { latest_change_id: number; total_changes: number };
      objects: [];
      snapshots: { total_snapshots: number; latest: null };
    };
    assert.deepEqual(body.cursor, { latest_change_id: 0, total_changes: 0 });
    assert.deepEqual(body.objects, []);
    assert.deepEqual(body.snapshots, { total_snapshots: 0, latest: null });
  });

  it("rejects revoked devices before reading sync status", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [null, { latest_change_id: 51, total_changes: 7 }],
      allRows: [objectStatusRow()],
    });

    const response = await handleRequest(
      syncStatusRequest(),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_not_approved" });
    assert.equal(d1.queries.length, 1);
  });

  it("rejects unsupported methods before session reads", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/status", {
        method: "POST",
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({ d1: testD1Database([]) }),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
  });

  it("returns a server error for malformed status rows", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        { device_id: DEVICE_ID },
        { latest_change_id: 51, total_changes: 7 },
        { total_snapshots: 1 },
        latestSnapshotRow(),
        { approved_devices: 1 },
      ],
      allRows: [objectStatusRow({ object_type: "passwords" })],
    });

    const response = await handleRequest(
      syncStatusRequest(),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "sync_status_invalid" });
  });
});

function syncStatusRequest(): Request {
  return new Request("https://elydora.test/api/sync/status", {
    headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
  });
}

function objectStatusRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    object_type: "tabs",
    active_count: 3,
    deleted_count: 0,
    latest_logical_clock: 42,
    latest_updated_at: 1_780_000_700,
    ...overrides,
  };
}

function objectStatusDocument(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return objectStatusRow(overrides);
}

function latestSnapshotRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    snapshot_id: "snapshot-01",
    payload_hash: "a".repeat(64),
    logical_clock: 42,
    device_id: DEVICE_ID,
    size_bytes: 26,
    created_at: 1_780_000_900,
    ...overrides,
  };
}
