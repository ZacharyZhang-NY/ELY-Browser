import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  sessionDocument,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const DEVICE_APPROVAL_IDEMPOTENCY_KEY = "device-approval-0001";

describe("device approval routes", () => {
  it("approves a pending device from an approved current device", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        null,
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        deviceRow({ device_id: "device-02", approval_status: "pending", approved_at: null }),
        deviceRow({
          device_id: "device-02",
          approval_status: "approved",
          approved_at: 1_780_000_300,
        }),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceApprovalBody()),
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
      approved_by_device_id: "device-01",
      approved_at: 1_780_000_300,
      device: {
        device_id: "device-02",
        public_key: PUBLIC_KEY,
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: "approved",
        created_at: 1_780_000_000,
        approved_at: 1_780_000_300,
        last_active_at: 1_780_000_020,
        revoked_at: null,
        current: false,
      },
    });
    assert.equal(d1.batches[0], 2);
    assert.ok(d1.queries[0]?.includes("FROM device_approvals"));
    assert.ok(d1.queries[1]?.includes("approval_status = 'approved'"));
    assert.ok(d1.queries[3]?.includes("INSERT INTO device_approvals"));
    assert.ok(d1.queries[4]?.includes("UPDATE user_devices"));
    assert.deepEqual(d1.binds[0], ["user-01", DEVICE_APPROVAL_IDEMPOTENCY_KEY]);
    assert.deepEqual(d1.binds[1], ["user-01", "device-01"]);
    assert.deepEqual(d1.binds[2], ["user-01", "device-02"]);
    assert.deepEqual(d1.binds[3]?.slice(0, 4), [
      "user-01",
      DEVICE_APPROVAL_IDEMPOTENCY_KEY,
      "device-02",
      "device-01",
    ]);
    assert.equal(d1.binds[3]?.[7], DEVICE_APPROVAL_IDEMPOTENCY_KEY);
    assert.deepEqual(d1.binds[5], ["user-01", "device-02"]);
  });

  it("returns the existing approval for an idempotent replay", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        {
          device_id: "device-02",
          requester_device_id: "device-01",
          status: "approved",
          decided_at: 1_780_000_300,
        },
        deviceRow({
          device_id: "device-02",
          approval_status: "approved",
          approved_at: 1_780_000_300,
        }),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceApprovalBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 200);
    const body = (await response.json()) as { approved_at: number };
    assert.equal(body.approved_at, 1_780_000_300);
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 2);
    assert.deepEqual(d1.binds, [
      ["user-01", DEVICE_APPROVAL_IDEMPOTENCY_KEY],
      ["user-01", "device-02"],
    ]);
  });

  it("rejects approval from a current device that is not approved", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null, null] });
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceApprovalBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects self approval before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...deviceApprovalBody(), device_id: "device-01" }),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(d1.queries, []);
  });

  it("rejects invalid approval payloads before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...deviceApprovalBody(), idempotency_key: "short" }),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_device_approval" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects unauthenticated device approval before D1 writes", async () => {
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(deviceApprovalBody()),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });
});

function deviceApprovalBody(): Record<string, unknown> {
  return {
    version: 1,
    device_id: "device-02",
    idempotency_key: DEVICE_APPROVAL_IDEMPOTENCY_KEY,
  };
}

function deviceRow(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    device_id: "device-01",
    public_key: PUBLIC_KEY,
    device_name: "MacBook Pro",
    platform: "macOS",
    approval_status: "approved",
    created_at: 1_780_000_000,
    approved_at: 1_780_000_010,
    last_active_at: 1_780_000_020,
    revoked_at: null,
    ...overrides,
  };
}
