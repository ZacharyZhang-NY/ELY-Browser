import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";
import { pluginRegistryKvKey } from "../src/plugin_registry.js";
import { releaseManifestKvKey } from "../src/release_manifests.js";
import { publicSigningKeysKvKey } from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);
const RELEASE_SIGNATURE = "b".repeat(128);
const RELEASE_SHA256 = "c".repeat(64);
const PLUGIN_CHECKSUM = "d".repeat(64);
const PLUGIN_PACKAGE_SHA256 = "e".repeat(64);
const PLUGIN_SIGNATURE = "f".repeat(128);

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
      new Request("https://elydora.test/api/unknown"),
      testEnv(null),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "not_found" });
  });

  it("routes Better Auth session requests under api auth", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/auth/get-session"),
      testEnv(null),
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("content-type"), "application/json");
    assert.equal(await response.text(), "null");
  });

  it("returns public plugin catalog from KV", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins"),
      testEnv(null, null, pluginRegistryDocument()),
    );

    assert.equal(response.status, 200);
    assert.equal(
      response.headers.get("cache-control"),
      "public, max-age=300, stale-while-revalidate=60",
    );
    assert.deepEqual(await response.json(), {
      version: 1,
      generated_at: "2026-05-08T00:00:00.000Z",
      plugins: [
        {
          id: "elydora.reader",
          name: "Reader",
          description: "Reading workflow tools.",
          author: "Elydora",
          homepage: "https://elydora.com/plugins/reader",
          permissions: ["page:metadata", "ui:command"],
          contributes: ["command-bar-command"],
          min_ely_build: "0.1.0",
        },
      ],
    });
  });

  it("returns public plugin details from KV", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/elydora.reader"),
      testEnv(null, null, pluginRegistryDocument()),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      generated_at: "2026-05-08T00:00:00.000Z",
      plugin: {
        id: "elydora.reader",
        name: "Reader",
        description: "Reading workflow tools.",
        author: "Elydora",
        homepage: "https://elydora.com/plugins/reader",
        permissions: ["page:metadata", "ui:command"],
        contributes: ["command-bar-command"],
        min_ely_build: "0.1.0",
        checksum: PLUGIN_CHECKSUM,
        signature: {
          algorithm: "ed25519",
          key_id: "elydora-alpha-plugins",
          public_key: PUBLIC_KEY,
          value: PLUGIN_SIGNATURE,
        },
      },
    });
  });

  it("returns public plugin package download information", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/elydora.reader/package"),
      testEnv(null, null, pluginRegistryDocument()),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      plugin_id: "elydora.reader",
      url: "https://downloads.elydora.com/plugins/reader/0.1.0/reader.rplug",
      sha256: PLUGIN_PACKAGE_SHA256,
      signature: PLUGIN_SIGNATURE,
      size_bytes: 65536,
    });
  });

  it("rejects unsupported methods on public plugin catalog", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins", { method: "POST" }),
      testEnv(null, null, pluginRegistryDocument()),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "GET");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
  });

  it("returns service unavailable when plugin registry is missing", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins"),
      testEnv(null),
    );

    assert.equal(response.status, 503);
    assert.deepEqual(await response.json(), { error: "plugin_registry_unavailable" });
  });

  it("returns a generic server error for malformed plugin registry", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins"),
      testEnv(null, null, "{"),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "plugin_registry_invalid" });
  });

  it("returns not found for unknown public plugins", async () => {
    const response = await handleRequest(
      new Request("https://elydora.test/api/plugins/elydora.unknown"),
      testEnv(null, null, pluginRegistryDocument()),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "plugin_not_found" });
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

function testEnv(
  publicSigningKeys: string | null,
  releaseManifest?: string | null,
  pluginRegistry?: string | null,
): Env {
  const values = new Map<string, string>();
  if (publicSigningKeys !== null) {
    values.set(publicSigningKeysKvKey("local"), publicSigningKeys);
  }
  if (releaseManifest !== undefined && releaseManifest !== null) {
    values.set(releaseManifestKvKey("local"), releaseManifest);
  }
  if (pluginRegistry !== undefined && pluginRegistry !== null) {
    values.set(pluginRegistryKvKey("local"), pluginRegistry);
  }

  return {
    ELY_ENVIRONMENT: "local",
    ELY_AUTH_BASE_URL: "https://elydora.test",
    ELY_AUTH_SECRET: "test-auth-secret-for-worker-routes",
    ELY_DB: testD1Database(),
    ELY_KV: {
      get(key: string): Promise<string | null> {
        return Promise.resolve(values.get(key) ?? null);
      },
      delete(key: string): Promise<void> {
        values.delete(key);
        return Promise.resolve();
      },
    },
    ELY_STORAGE: testR2Bucket(),
    ELY_RATE_LIMITER: {
      limit(): Promise<{ success: boolean }> {
        return Promise.resolve({ success: true });
      },
    },
    ELY_API_AUDIT: {
      writeDataPoint(): void {},
    },
    ELY_DIAGNOSTICS: {
      writeDataPoint(): void {},
    },
  };
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

function pluginRegistryDocument(): string {
  return JSON.stringify({
    version: 1,
    generated_at: "2026-05-08T00:00:00.000Z",
    plugins: [
      {
        id: "elydora.reader",
        name: "Reader",
        description: "Reading workflow tools.",
        author: "Elydora",
        homepage: "https://elydora.com/plugins/reader",
        permissions: ["page:metadata", "ui:command"],
        contributes: ["command-bar-command"],
        min_ely_build: "0.1.0",
        checksum: PLUGIN_CHECKSUM,
        signature: {
          algorithm: "ed25519",
          key_id: "elydora-alpha-plugins",
          public_key: PUBLIC_KEY,
          value: PLUGIN_SIGNATURE,
        },
        package: {
          url: "https://downloads.elydora.com/plugins/reader/0.1.0/reader.rplug",
          sha256: PLUGIN_PACKAGE_SHA256,
          size_bytes: 65536,
        },
      },
    ],
  });
}
