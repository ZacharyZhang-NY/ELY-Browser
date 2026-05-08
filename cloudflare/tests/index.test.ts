import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";
import { releaseManifestKvKey } from "../src/release_manifests.js";
import { publicSigningKeysKvKey } from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);
const RELEASE_SIGNATURE = "b".repeat(128);
const RELEASE_SHA256 = "c".repeat(64);

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

  it("returns release manifest from KV", async () => {
    const env = testEnv(null, releaseManifestDocument());

    const response = await handleRequest(
      new Request("https://elydora.test/api/releases/manifest"),
      env,
    );

    assert.equal(response.status, 200);
    assert.equal(
      response.headers.get("cache-control"),
      "public, max-age=120, stale-while-revalidate=60",
    );
    assert.deepEqual(await response.json(), {
      version: 1,
      channel: "stable",
      generated_at: "2026-05-08T00:00:00.000Z",
      artifacts: [
        {
          platform: "macos",
          architecture: "aarch64",
          version: "0.1.0",
          url: "https://downloads.elydora.com/ely-browser/0.1.0/macos-aarch64.zip",
          sha256: RELEASE_SHA256,
          signature: RELEASE_SIGNATURE,
          size_bytes: 1048576,
        },
      ],
    });
  });

  it("rejects unsupported methods on release manifest", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/releases/manifest", { method: "POST" }),
      testEnv(null, releaseManifestDocument()),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
  });

  it("returns service unavailable when release manifest is missing", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/releases/manifest"),
      testEnv(null),
    );

    assert.equal(response.status, 503);
    assert.deepEqual(await response.json(), { error: "release_manifest_unavailable" });
  });

  it("returns a generic server error for malformed release manifest", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/releases/manifest"),
      testEnv(null, "{"),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "release_manifest_invalid" });
  });

  it("returns release signature from the release manifest cache", async () => {
    const response = await handleRequest(
      new Request(
        "https://elydora.test/api/releases/signature?platform=macos&architecture=aarch64",
      ),
      testEnv(null, releaseManifestDocument()),
    );

    assert.equal(response.status, 200);
    assert.equal(
      response.headers.get("cache-control"),
      "public, max-age=120, stale-while-revalidate=60",
    );
    assert.deepEqual(await response.json(), {
      version: 1,
      channel: "stable",
      generated_at: "2026-05-08T00:00:00.000Z",
      platform: "macos",
      architecture: "aarch64",
      release_version: "0.1.0",
      sha256: RELEASE_SHA256,
      signature: RELEASE_SIGNATURE,
    });
  });

  it("rejects invalid release signature query parameters", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/releases/signature?platform=macos"),
      testEnv(null, releaseManifestDocument()),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_release_signature_query" });
  });

  it("returns not found for unmatched release signature targets", async () => {
    const response = await handleRequest(
      new Request(
        "https://elydora.test/api/releases/signature?platform=macos&architecture=x86_64",
      ),
      testEnv(null, releaseManifestDocument()),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "release_signature_not_found" });
  });

  it("rejects unsupported methods on release signature", async () => {
    const response = await handleRequest(
      new Request(
        "https://elydora.test/api/releases/signature?platform=macos&architecture=aarch64",
        { method: "POST" },
      ),
      testEnv(null, releaseManifestDocument()),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
  });
});

function testEnv(publicSigningKeys: string | null, releaseManifest?: string | null): Env {
  const values = new Map<string, string>();
  if (publicSigningKeys !== null) {
    values.set(publicSigningKeysKvKey("local"), publicSigningKeys);
  }
  if (releaseManifest !== undefined && releaseManifest !== null) {
    values.set(releaseManifestKvKey("local"), releaseManifest);
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

function releaseManifestDocument(): string {
  return JSON.stringify({
    version: 1,
    channel: "stable",
    generated_at: "2026-05-08T00:00:00.000Z",
    artifacts: [
      {
        platform: "macos",
        architecture: "aarch64",
        version: "0.1.0",
        url: "https://downloads.elydora.com/ely-browser/0.1.0/macos-aarch64.zip",
        sha256: RELEASE_SHA256,
        signature: RELEASE_SIGNATURE,
        size_bytes: 1048576,
      },
    ],
  });
}
