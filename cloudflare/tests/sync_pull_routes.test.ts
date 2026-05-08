import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { ACCESS_TOKEN, sessionDocument, testD1Database, testEnv } from "./devices_test_support.js";

const PAYLOAD_HASH = "a".repeat(64);

describe("sync pull routes", () => {
  it("returns sync change log entries for an approved current device", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [{ device_id: "device-01" }],
      allRows: [
        syncChangeRow({ change_id: 11, object_id: "tab-01" }),
        syncChangeRow({ change_id: 12, object_id: "bookmark-01", object_type: "bookmarks" }),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=10&limit=2", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: "user-01",
      device_id: "device-01",
      cursor: 10,
      next_cursor: 12,
      has_more: false,
      changes: [
        syncChangeDocument({ change_id: 11, object_id: "tab-01" }),
        syncChangeDocument({ change_id: 12, object_id: "bookmark-01", object_type: "bookmarks" }),
      ],
    });
    assert.ok(d1.queries[0]?.includes("approval_status = 'approved'"));
    assert.ok(d1.queries[1]?.includes("FROM sync_change_log"));
    assert.deepEqual(d1.binds, [
      ["user-01", "device-01"],
      ["user-01", 10, 3],
    ]);
  });

  it("reports more changes when the pull window is saturated", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [{ device_id: "device-01" }],
      allRows: [
        syncChangeRow({ change_id: 11, object_id: "tab-01" }),
        syncChangeRow({ change_id: 12, object_id: "tab-02" }),
        syncChangeRow({ change_id: 13, object_id: "tab-03" }),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=10&limit=2", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    const body = (await response.json()) as { has_more: boolean; next_cursor: number; changes: [] };
    assert.equal(response.status, 200);
    assert.equal(body.has_more, true);
    assert.equal(body.next_cursor, 12);
    assert.equal(body.changes.length, 2);
  });

  it("rejects revoked devices before reading sync deltas", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null], allRows: [syncChangeRow()] });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=10", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_not_approved" });
    assert.equal(d1.queries.length, 1);
  });

  it("rejects invalid cursors after session and device validation", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: "device-01" }] });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=old", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_pull" });
    assert.equal(d1.queries.length, 1);
  });

  it("returns a server error for malformed sync change rows", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [{ device_id: "device-01" }],
      allRows: [syncChangeRow({ payload_hash: "bad" })],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=10", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "sync_pull_invalid" });
  });

  it("rejects unauthenticated sync pulls before D1 reads", async () => {
    const d1 = testD1Database({ allRows: [syncChangeRow()] });
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=0"),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });
});

function syncChangeRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    change_id: 11,
    object_id: "tab-01",
    object_type: "tabs",
    operation: "upsert",
    payload_hash: PAYLOAD_HASH,
    logical_clock: 42,
    device_id: "device-02",
    created_at: 1_780_000_500,
    ...overrides,
  };
}

function syncChangeDocument(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return syncChangeRow(overrides);
}
