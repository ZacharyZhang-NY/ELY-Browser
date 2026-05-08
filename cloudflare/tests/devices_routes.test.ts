import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type {
  ElyAnalyticsDataPoint,
  ElyD1Database,
  ElyD1PreparedStatement,
  ElyR2PutOptions,
  Env,
} from "../src/bindings.js";
import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";

const ACCESS_TOKEN = "D".repeat(48);
const PUBLIC_KEY = "a".repeat(64);

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
});

interface TestEnvOptions {
  auditEvents?: ElyAnalyticsDataPoint[];
  d1?: RecordedD1Database;
  kvEntries?: [string, string][];
  kvReads?: string[];
}

interface RecordedD1Database extends ElyD1Database {
  binds: unknown[][];
  queries: string[];
}

function testEnv(options: TestEnvOptions): Env {
  const values = new Map(options.kvEntries ?? []);
  return {
    ELY_ENVIRONMENT: "local",
    ELY_DB: options.d1 ?? testD1Database([]),
    ELY_KV: {
      get(key: string): Promise<string | null> {
        options.kvReads?.push(key);
        return Promise.resolve(values.get(key) ?? null);
      },
    },
    ELY_STORAGE: testR2Bucket(),
    ELY_RATE_LIMITER: {
      limit(): Promise<{ success: boolean }> {
        return Promise.resolve({ success: true });
      },
    },
    ELY_API_AUDIT: {
      writeDataPoint(event?: ElyAnalyticsDataPoint): void {
        if (event !== undefined) {
          options.auditEvents?.push(event);
        }
      },
    },
  };
}

function testD1Database(rows: unknown[]): RecordedD1Database {
  const binds: unknown[][] = [];
  const queries: string[] = [];
  return {
    binds,
    queries,
    prepare(query: string) {
      queries.push(query);
      return testD1PreparedStatement(rows, binds);
    },
    batch() {
      return Promise.resolve([]);
    },
    exec() {
      return Promise.resolve({});
    },
  };
}

function testD1PreparedStatement(rows: unknown[], binds: unknown[][]): ElyD1PreparedStatement {
  return {
    bind(...values: unknown[]) {
      binds.push(values);
      return this;
    },
    first() {
      return Promise.resolve(null);
    },
    all<T>() {
      return Promise.resolve({ results: rows as T[] });
    },
    run() {
      return Promise.resolve({});
    },
  };
}

function testR2Bucket(): Env["ELY_STORAGE"] {
  return {
    get() {
      return Promise.resolve(null);
    },
    put(_key: string, value: ArrayBuffer, _options?: ElyR2PutOptions) {
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(value);
        },
      });
    },
  };
}

function sessionDocument(): string {
  return JSON.stringify({
    version: 1,
    user_id: "user-01",
    session_id: "session-01",
    device_id: "device-01",
    expires_at: "2099-01-01T00:00:00.000Z",
  });
}
