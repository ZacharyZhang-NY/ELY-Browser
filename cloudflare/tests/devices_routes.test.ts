import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyAnalyticsDataPoint } from "../src/bindings.js";
import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  sessionDocument,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const IDEMPOTENCY_KEY = "device-register-0001";

describe("device routes", () => {
  it("returns authenticated user devices from D1", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const d1 = testD1Database([
      {
        device_id: "device-01",
        public_key: PUBLIC_KEY,
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: "approved",
        created_at: 1_780_000_000,
        approved_at: 1_780_000_010,
        last_active_at: 1_780_000_020,
        revoked_at: null,
      },
      {
        device_id: "device-02",
        public_key: PUBLIC_KEY,
        device_name: "iMac",
        platform: "macOS",
        approval_status: "revoked",
        created_at: 1_770_000_000,
        approved_at: 1_770_000_010,
        last_active_at: 1_770_000_020,
        revoked_at: 1_770_000_030,
      },
    ]);

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}`, "cf-ray": "ray-devices" },
      }),
      testEnv({
        auditEvents,
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument()]],
      }),
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: "user-01",
      devices: [
        {
          device_id: "device-01",
          public_key: PUBLIC_KEY,
          device_name: "MacBook Pro",
          platform: "macOS",
          approval_status: "approved",
          created_at: 1_780_000_000,
          approved_at: 1_780_000_010,
          last_active_at: 1_780_000_020,
          revoked_at: null,
          current: true,
        },
        {
          device_id: "device-02",
          public_key: PUBLIC_KEY,
          device_name: "iMac",
          platform: "macOS",
          approval_status: "revoked",
          created_at: 1_770_000_000,
          approved_at: 1_770_000_010,
          last_active_at: 1_770_000_020,
          revoked_at: 1_770_000_030,
          current: false,
        },
      ],
    });
    assert.deepEqual(d1.binds, [["user-01"]]);
    assert.ok(d1.queries[0]?.includes("FROM user_devices"));
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 8), [
      "devices.list",
      "GET",
      "/api/devices",
      "handled",
      "ray-devices",
      "",
      "user-01",
      "device-01",
    ]);
  });

  it("rejects unauthenticated device list requests before D1 reads", async () => {
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices"),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects unsupported device list methods before session reads", async () => {
    const d1 = testD1Database([]);
    const kvReads: string[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices", { method: "POST" }),
      testEnv({ d1, kvReads }),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
    assert.deepEqual(kvReads, []);
    assert.deepEqual(d1.queries, []);
  });

  it("returns a generic server error for malformed D1 device rows", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1: testD1Database([
          {
            device_id: "device-01",
            public_key: PUBLIC_KEY,
            device_name: "MacBook Pro",
            platform: "macOS",
            approval_status: "deleted",
            created_at: 1_780_000_000,
            approved_at: null,
            last_active_at: null,
            revoked_at: null,
          },
        ]),
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument()]],
      }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "devices_invalid" });
  });

  it("registers the current device as a pending idempotent D1 write", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const sessionCacheKey = authSessionCacheKvKey("local", tokenHash);
    const kvPuts: [string, string][] = [];
    const d1 = testD1Database([
      {
        device_id: "device-01",
        public_key: PUBLIC_KEY.toUpperCase(),
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: "pending",
        created_at: 1_780_000_100,
        approved_at: null,
        last_active_at: 1_780_000_100,
        revoked_at: null,
      },
    ]);

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceRegistrationBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[sessionCacheKey, sessionDocument()]],
        kvPuts,
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: "user-01",
      device: {
        device_id: "device-01",
        public_key: PUBLIC_KEY,
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: "pending",
        created_at: 1_780_000_100,
        approved_at: null,
        last_active_at: 1_780_000_100,
        revoked_at: null,
        current: true,
      },
    });
    assert.ok(d1.queries[0]?.includes("INSERT INTO user_devices"));
    assert.ok(d1.queries[0]?.includes("ON CONFLICT DO NOTHING"));
    assert.ok(d1.queries[1]?.includes("WHERE user_id = ? AND idempotency_key = ?"));
    assert.ok(d1.queries[2]?.includes("better_auth_session_device_context"));
    assert.deepEqual(d1.binds[0]?.slice(0, 5), [
      "user-01",
      "device-01",
      PUBLIC_KEY,
      "MacBook Pro",
      "macOS",
    ]);
    assert.equal(typeof d1.binds[0]?.[5], "number");
    assert.equal(typeof d1.binds[0]?.[6], "number");
    assert.equal(d1.binds[0]?.[7], IDEMPOTENCY_KEY);
    assert.deepEqual(d1.binds[1], ["user-01", IDEMPOTENCY_KEY]);
    assert.deepEqual(d1.binds[2]?.slice(0, 3), ["session-01", "user-01", "device-01"]);
    assert.deepEqual(kvPuts, []);
  });

  it("registers D1 device context for sessions without a current device", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const sessionCacheKey = authSessionCacheKvKey("local", tokenHash);
    const kvPuts: [string, string][] = [];
    const deviceRow = {
      device_id: "device-01",
      public_key: PUBLIC_KEY,
      device_name: "MacBook Pro",
      platform: "macOS",
      approval_status: "pending",
      created_at: 1_780_000_100,
      approved_at: null,
      last_active_at: 1_780_000_100,
      revoked_at: null,
    };
    const d1 = testD1Database({
      allRows: [deviceRow],
      firstRows: [deviceRow],
      sessionRow: {
        id: "session-01",
        userId: "user-01",
        expiresAt: "2099-01-01T00:00:00.000Z",
        deviceId: null,
      },
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceRegistrationBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[sessionCacheKey, sessionDocument(null)]],
        kvPuts,
      }),
    );

    assert.equal(response.status, 201);
    assert.deepEqual(d1.binds[2]?.slice(0, 3), ["session-01", "user-01", "device-01"]);
    assert.deepEqual(kvPuts, []);
  });

  it("rejects an unbound session replaying any existing device registration", async () => {
    for (const status of ["pending", "approved", "revoked"] as const) {
      const existingDevice = {
        device_id: "device-01",
        public_key: PUBLIC_KEY,
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: status,
        created_at: 1_780_000_000,
        approved_at: status === "pending" ? null : 1_780_000_010,
        last_active_at: 1_780_000_020,
        revoked_at: status === "revoked" ? 1_780_000_030 : null,
      };
      const d1 = testD1Database({
        firstRows: [existingDevice],
        runChanges: [0],
        sessionRow: {
          id: "session-02",
          userId: "user-01",
          expiresAt: "2099-01-01T00:00:00.000Z",
          deviceId: null,
        },
      });

      const response = await handleRequest(
        new Request("https://elydora.test/api/devices/register", {
          method: "POST",
          headers: {
            authorization: `Bearer ${ACCESS_TOKEN}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(deviceRegistrationBody()),
        }),
        testEnv({ d1 }),
      );

      assert.equal(response.status, 409, status);
      assert.deepEqual(await response.json(), { error: "device_registration_conflict" });
      assert.equal(d1.queries.some((query) => query.includes("session_device_context")), false);
    }
  });

  it("allows an exact idempotent retry from the already bound session", async () => {
    const pendingDevice = {
      device_id: "device-01",
      public_key: PUBLIC_KEY,
      device_name: "MacBook Pro",
      platform: "macOS",
      approval_status: "pending",
      created_at: 1_780_000_000,
      approved_at: null,
      last_active_at: 1_780_000_020,
      revoked_at: null,
    };
    const d1 = testD1Database({ firstRows: [pendingDevice], runChanges: [0] });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(deviceRegistrationBody()),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 201);
    assert.equal(
      ((await response.json()) as { device: { device_id: string } }).device.device_id,
      "device-01",
    );
    assert.equal(d1.queries.some((query) => query.includes("session_device_context")), false);
  });

  it("rejects a device id collision with a different idempotency key", async () => {
    const d1 = testD1Database({
      firstRows: [],
      runChanges: [0],
      sessionRow: {
        id: "session-02",
        userId: "user-01",
        expiresAt: "2099-01-01T00:00:00.000Z",
        deviceId: null,
      },
    });
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          ...deviceRegistrationBody(),
          idempotency_key: "device-register-0002",
        }),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "device_registration_conflict" });
    assert.equal(d1.queries.some((query) => query.includes("session_device_context")), false);
  });

  it("rejects invalid device registration payloads before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...deviceRegistrationBody(), idempotency_key: "short" }),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument()]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_device_registration" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects registration for a different authenticated device before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...deviceRegistrationBody(), device_id: "device-02" }),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument()]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_context_mismatch" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects unauthenticated device registration before D1 writes", async () => {
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(deviceRegistrationBody()),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });
});

function deviceRegistrationBody(): Record<string, unknown> {
  return {
    version: 1,
    device_id: "device-01",
    public_key: PUBLIC_KEY,
    device_name: "MacBook Pro",
    platform: "macOS",
    idempotency_key: IDEMPOTENCY_KEY,
  };
}
