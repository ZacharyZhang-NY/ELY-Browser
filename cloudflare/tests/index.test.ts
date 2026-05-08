import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";
import { publicSigningKeysKvKey } from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);

describe("worker routes", () => {
  it("returns public plugin signing keys from KV", async () => {
    const env = testEnv(
      JSON.stringify({
        version: 1,
        keys: [{ key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY }],
      }),
    );

    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys"),
      env,
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("content-type"), "application/json; charset=utf-8");
    assert.equal(response.headers.get("x-content-type-options"), "nosniff");
    assert.equal(
      response.headers.get("cache-control"),
      "public, max-age=300, stale-while-revalidate=60",
    );
    assert.deepEqual(await response.json(), {
      version: 1,
      keys: [{ key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY }],
    });
  });

  it("rejects unsupported methods on public signing keys", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys", { method: "POST" }),
      testEnv("{}"),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
  });

  it("returns service unavailable when public signing keys are missing", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys"),
      testEnv(null),
    );

    assert.equal(response.status, 503);
    assert.deepEqual(await response.json(), { error: "public_signing_keys_unavailable" });
  });

  it("returns a generic server error for malformed public signing keys", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/signing-keys"),
      testEnv("{"),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "public_signing_keys_invalid" });
  });

  it("returns JSON not found for unknown routes", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/status"),
      testEnv(null),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "not_found" });
  });
});

function testEnv(publicSigningKeys: string | null): Env {
  const values = new Map<string, string>();
  if (publicSigningKeys !== null) {
    values.set(publicSigningKeysKvKey("local"), publicSigningKeys);
  }

  return {
    ELY_ENVIRONMENT: "local",
    ELY_KV: {
      get(key: string): Promise<string | null> {
        return Promise.resolve(values.get(key) ?? null);
      },
    },
  };
}
