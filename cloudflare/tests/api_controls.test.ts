import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyAnalyticsDataPoint, Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";
import { publicSigningKeysKvKey } from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);

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
});

interface TestEnvOptions {
  auditEvents?: ElyAnalyticsDataPoint[];
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

  return {
    ELY_ENVIRONMENT: "local",
    ELY_KV: {
      get(key: string): Promise<string | null> {
        options.kvReads?.push(key);
        return Promise.resolve(values.get(key) ?? null);
      },
    },
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
