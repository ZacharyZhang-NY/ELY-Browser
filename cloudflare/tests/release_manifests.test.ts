import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  ReleaseManifestSchemaError,
  parseReleaseManifestDocument,
  releaseManifestKvKey,
} from "../src/release_manifests.js";

const SHA256 = "a".repeat(64);
const SIGNATURE = "b".repeat(128);

describe("release manifests", () => {
  it("builds an environment-prefixed KV key", () => {
    assert.equal(releaseManifestKvKey("production"), "ely:production:release_manifest_cache");
  });

  it("parses a valid release manifest", () => {
    const document = parseReleaseManifestDocument(validManifest());

    assert.deepEqual(document, {
      version: 1,
      channel: "stable",
      generated_at: "2026-05-08T00:00:00.000Z",
      artifacts: [
        {
          platform: "macos",
          architecture: "aarch64",
          version: "0.1.0",
          url: "https://downloads.elydora.com/ely-browser/0.1.0/macos-aarch64.zip",
          sha256: SHA256,
          signature: SIGNATURE,
          size_bytes: 1048576,
        },
      ],
    });
  });

  it("rejects duplicate platform architecture artifacts", () => {
    assert.throws(
      () =>
        parseReleaseManifestDocument(
          validManifest({
            artifacts: [
              validArtifact(),
              { ...validArtifact(), version: "0.1.1", sha256: "c".repeat(64) },
            ],
          }),
        ),
      ReleaseManifestSchemaError,
    );
  });

  it("rejects non-https artifact URLs", () => {
    assert.throws(
      () =>
        parseReleaseManifestDocument(
          validManifest({
            artifacts: [{ ...validArtifact(), url: "http://downloads.elydora.com/app.zip" }],
          }),
        ),
      ReleaseManifestSchemaError,
    );
  });

  it("rejects malformed artifact signatures", () => {
    assert.throws(
      () =>
        parseReleaseManifestDocument(
          validManifest({ artifacts: [{ ...validArtifact(), signature: "abcd" }] }),
        ),
      ReleaseManifestSchemaError,
    );
  });
});

function validManifest(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    version: 1,
    channel: "stable",
    generated_at: "2026-05-08T00:00:00.000Z",
    artifacts: [validArtifact()],
    ...overrides,
  });
}

function validArtifact(): Record<string, unknown> {
  return {
    platform: "macos",
    architecture: "aarch64",
    version: "0.1.0",
    url: "https://downloads.elydora.com/ely-browser/0.1.0/macos-aarch64.zip",
    sha256: SHA256,
    signature: SIGNATURE,
    size_bytes: 1048576,
  };
}
