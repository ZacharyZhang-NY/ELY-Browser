import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyAnalyticsDataPoint, Env } from "../src/bindings.js";
import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { withAuthenticatedApiControls } from "../src/api_controls.js";
import { handleRequest } from "../src/index.js";
import { jsonResponse } from "../src/responses.js";
import { publicSigningKeysKvKey } from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);
const ACCESS_TOKEN = "A".repeat(48);

describe("api controls", () => {
  it("rate limits public API routes before reading KV", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const kvReads: string[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys"),
      testEnv({ auditEvents, kvReads, rateLimitSuccess: false }),
    );

    assert.equal(response.status, 429);
    assert.equal(response.headers.get("retry-after"), "60");
    assert.deepEqual(await response.json(), { error: "rate_limited" });
    assert.deepEqual(kvReads, []);
    assert.equal(auditEvents.length, 1);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "plugins.signing_keys",
      "GET",
      "/api/plugins/signing-keys",
      "rate_limited",
    ]);
    assert.equal(auditEvents[0]?.doubles?.[0], 429);
  });

  it("records successful public API requests", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const rateLimitKeys: string[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys", {
        headers: { "cf-ray": "ray-1", "user-agent": "ely-test" },
      }),
      testEnv({ auditEvents, rateLimitKeys }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(rateLimitKeys, ["local:plugins.signing_keys"]);
    assert.equal(auditEvents.length, 1);
    assert.deepEqual(auditEvents[0]?.indexes, ["local"]);
    assert.deepEqual(auditEvents[0]?.blobs, [
      "plugins.signing_keys",
      "GET",
      "/api/plugins/signing-keys",
      "handled",
      "ray-1",
      "ely-test",
    ]);
    assert.equal(auditEvents[0]?.doubles?.[0], 200);
  });

  it("records method rejections without consuming rate limit tokens", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const rateLimitKeys: string[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys", { method: "POST" }),
      testEnv({ auditEvents, rateLimitKeys }),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(rateLimitKeys, []);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "plugins.signing_keys",
      "POST",
      "/api/plugins/signing-keys",
      "method_not_allowed",
    ]);
    assert.equal(auditEvents[0]?.doubles?.[0], 405);
  });

  it("rate limits authenticated API routes before reading session cache", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const kvReads: string[] = [];
    const response = await withAuthenticatedApiControls(
      new Request("https://elydora.test/api/devices", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({ auditEvents, kvReads, rateLimitSuccess: false }),
      "devices.list",
      ["GET"],
      () => Promise.resolve(jsonResponse({ ok: true }, 200)),
    );

    assert.equal(response.status, 429);
    assert.deepEqual(kvReads, []);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "devices.list",
      "GET",
      "/api/devices",
      "rate_limited",
    ]);
  });

  it("rejects missing authenticated API credentials after rate limit", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const rateLimitKeys: string[] = [];
    const response = await withAuthenticatedApiControls(
      new Request("https://elydora.test/api/devices"),
      testEnv({ auditEvents, rateLimitKeys }),
      "devices.list",
      ["GET"],
      () => Promise.resolve(jsonResponse({ ok: true }, 200)),
    );

    assert.equal(response.status, 401);
    assert.equal(response.headers.get("www-authenticate"), "Bearer");
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(rateLimitKeys, ["local:devices.list:anonymous"]);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "devices.list",
      "GET",
      "/api/devices",
      "authorization_missing",
    ]);
  });

  it("rejects malformed authenticated API credentials with audit coverage", async () => {
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const rateLimitKeys: string[] = [];
    const response = await withAuthenticatedApiControls(
      new Request("https://elydora.test/api/devices", {
        headers: { authorization: "Bearer short" },
      }),
      testEnv({ auditEvents, rateLimitKeys }),
      "devices.list",
      ["GET"],
      () => Promise.resolve(jsonResponse({ ok: true }, 200)),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_invalid" });
    assert.deepEqual(rateLimitKeys, ["local:devices.list:authorization_invalid"]);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "devices.list",
      "GET",
      "/api/devices",
      "authorization_invalid",
    ]);
  });

  it("passes authenticated session context and records subject audit fields", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const kvReads: string[] = [];
    const rateLimitKeys: string[] = [];
    let receivedTokenHash = "";
    const response = await withAuthenticatedApiControls(
      new Request("https://elydora.test/api/devices", {
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "cf-ray": "ray-auth",
          "user-agent": "ely-auth-test",
        },
      }),
      testEnv({
        auditEvents,
        kvReads,
        rateLimitKeys,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument()]],
      }),
      "devices.list",
      ["GET"],
      (context) => {
        receivedTokenHash = context.tokenHash;
        return Promise.resolve(
          jsonResponse({ user_id: context.userId, device_id: context.deviceId }, 200),
        );
      },
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { user_id: "user-01", device_id: "device-01" });
    assert.deepEqual(rateLimitKeys, [`local:devices.list:bearer:${tokenHash}`]);
    assert.deepEqual(kvReads, [authSessionCacheKvKey("local", tokenHash)]);
    assert.equal(receivedTokenHash, tokenHash);
    assert.deepEqual(auditEvents[0]?.blobs, [
      "devices.list",
      "GET",
      "/api/devices",
      "handled",
      "ray-auth",
      "ely-auth-test",
      "user-01",
      "device-01",
    ]);
  });

  it("rejects expired authenticated sessions", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const response = await withAuthenticatedApiControls(
      new Request("https://elydora.test/api/devices", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        kvEntries: [
          [
            authSessionCacheKvKey("local", tokenHash),
            sessionDocument("2026-01-01T00:00:00.000Z"),
          ],
        ],
      }),
      "devices.list",
      ["GET"],
      () => Promise.resolve(jsonResponse({ ok: true }, 200)),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "session_expired" });
  });
});

interface TestEnvOptions {
  auditEvents?: ElyAnalyticsDataPoint[];
  kvEntries?: [string, string][];
  kvReads?: string[];
  rateLimitKeys?: string[];
  rateLimitSuccess?: boolean;
}

function testEnv(options: TestEnvOptions = {}): Env {
  const values = new Map<string, string>([
    [
      publicSigningKeysKvKey("local"),
      JSON.stringify({
        version: 1,
        keys: [{ key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY }],
      }),
    ],
  ]);
  for (const [key, value] of options.kvEntries ?? []) {
    values.set(key, value);
  }

  return {
    ELY_ENVIRONMENT: "local",
    ELY_DB: testD1Database(),
    ELY_KV: {
      get(key: string): Promise<string | null> {
        options.kvReads?.push(key);
        return Promise.resolve(values.get(key) ?? null);
      },
    },
    ELY_STORAGE: testR2Bucket(),
    ELY_RATE_LIMITER: {
      limit(input: { key: string }): Promise<{ success: boolean }> {
        options.rateLimitKeys?.push(input.key);
        return Promise.resolve({ success: options.rateLimitSuccess ?? true });
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

function sessionDocument(expiresAt = "2099-01-01T00:00:00.000Z"): string {
  return JSON.stringify({
    version: 1,
    user_id: "user-01",
    session_id: "session-01",
    device_id: "device-01",
    expires_at: expiresAt,
  });
}

function testR2Bucket(): Env["ELY_STORAGE"] {
  return {
    get() {
      return Promise.resolve(null);
    },
    put() {
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(new ArrayBuffer(0));
        },
      });
    },
    delete() {
      return Promise.resolve();
    },
  };
}

function testD1Database(): Env["ELY_DB"] {
  return {
    prepare() {
      return testD1PreparedStatement();
    },
    batch() {
      return Promise.resolve([]);
    },
    exec() {
      return Promise.resolve({});
    },
  };
}

function testD1PreparedStatement(): ReturnType<Env["ELY_DB"]["prepare"]> {
  return {
    bind() {
      return this;
    },
    first() {
      return Promise.resolve(null);
    },
    all() {
      return Promise.resolve({ results: [] });
    },
    run() {
      return Promise.resolve({});
    },
  };
}
