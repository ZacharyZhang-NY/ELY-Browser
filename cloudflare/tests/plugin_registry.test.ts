import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  PluginRegistrySchemaError,
  parsePluginRegistryDocument,
  pluginCatalogDocument,
  pluginDetailsDocument,
  pluginPackageDocument,
  pluginRegistryKvKey,
} from "../src/plugin_registry.js";

const CHECKSUM = "a".repeat(64);
const PACKAGE_SHA256 = "b".repeat(64);
const PUBLIC_KEY = "c".repeat(64);
const SIGNATURE = "d".repeat(128);

describe("plugin registry", () => {
  it("builds an environment-prefixed KV key", () => {
    assert.equal(pluginRegistryKvKey("production"), "ely:production:plugin_registry_cache");
  });

  it("parses a valid plugin registry", () => {
    const document = parsePluginRegistryDocument(validRegistry());

    assert.deepEqual(document, {
      version: 1,
      generated_at: "2026-05-08T00:00:00.000Z",
      plugins: [parsedPlugin()],
    });
  });

  it("builds a public catalog document", () => {
    const registry = parsePluginRegistryDocument(validRegistry());

    assert.deepEqual(pluginCatalogDocument(registry), {
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

  it("extracts plugin details and package download documents", () => {
    const registry = parsePluginRegistryDocument(validRegistry());

    assert.deepEqual(pluginDetailsDocument(registry, "elydora.reader"), {
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
        checksum: CHECKSUM,
        signature: {
          algorithm: "ed25519",
          key_id: "elydora-alpha-plugins",
          public_key: PUBLIC_KEY,
          value: SIGNATURE,
        },
      },
    });
    assert.deepEqual(pluginPackageDocument(registry, "elydora.reader"), {
      version: 1,
      plugin_id: "elydora.reader",
      url: "https://downloads.elydora.com/plugins/reader/0.1.0/reader.rplug",
      sha256: PACKAGE_SHA256,
      signature: SIGNATURE,
      size_bytes: 65536,
    });
  });

  it("rejects duplicate plugin ids", () => {
    assert.throws(
      () =>
        parsePluginRegistryDocument(
          validRegistry({
            plugins: [validPlugin(), { ...validPlugin(), name: "Reader Copy" }],
          }),
        ),
      PluginRegistrySchemaError,
    );
  });

  it("rejects malformed permission and package URLs", () => {
    assert.throws(
      () =>
        parsePluginRegistryDocument(
          validRegistry({
            plugins: [{ ...validPlugin(), permissions: ["tabs:admin"] }],
          }),
        ),
      PluginRegistrySchemaError,
    );
    assert.throws(
      () =>
        parsePluginRegistryDocument(
          validRegistry({
            plugins: [
              {
                ...validPlugin(),
                package: { ...validPluginPackage(), url: "http://downloads.elydora.com/a.rplug" },
              },
            ],
          }),
        ),
      PluginRegistrySchemaError,
    );
  });
});

function validRegistry(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    version: 1,
    generated_at: "2026-05-08T00:00:00.000Z",
    plugins: [validPlugin()],
    ...overrides,
  });
}

function validPlugin(): Record<string, unknown> {
  return {
    id: "elydora.reader",
    name: "Reader",
    description: "Reading workflow tools.",
    author: "Elydora",
    homepage: "https://elydora.com/plugins/reader",
    permissions: ["page:metadata", "ui:command"],
    contributes: ["command-bar-command"],
    min_ely_build: "0.1.0",
    checksum: CHECKSUM,
    signature: {
      algorithm: "ed25519",
      key_id: "elydora-alpha-plugins",
      public_key: PUBLIC_KEY.toUpperCase(),
      value: SIGNATURE.toUpperCase(),
    },
    package: validPluginPackage(),
  };
}

function parsedPlugin(): Record<string, unknown> {
  return {
    ...validPlugin(),
    signature: {
      algorithm: "ed25519",
      key_id: "elydora-alpha-plugins",
      public_key: PUBLIC_KEY,
      value: SIGNATURE,
    },
    package: {
      ...validPluginPackage(),
      sha256: PACKAGE_SHA256,
    },
  };
}

function validPluginPackage(): Record<string, unknown> {
  return {
    url: "https://downloads.elydora.com/plugins/reader/0.1.0/reader.rplug",
    sha256: PACKAGE_SHA256.toUpperCase(),
    size_bytes: 65536,
  };
}
