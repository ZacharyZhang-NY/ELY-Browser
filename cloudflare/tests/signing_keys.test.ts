import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  SigningKeysSchemaError,
  parsePublicSigningKeysDocument,
  publicSigningKeysKvKey,
} from "../src/signing_keys.js";

const PUBLIC_KEY = "a".repeat(64);

describe("public signing keys", () => {
  it("builds an environment-prefixed KV key", () => {
    assert.equal(publicSigningKeysKvKey("production"), "ely:production:public_signing_keys");
  });

  it("parses a valid key document", () => {
    const document = parsePublicSigningKeysDocument(
      JSON.stringify({
        version: 1,
        keys: [{ key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY.toUpperCase() }],
      }),
    );

    assert.deepEqual(document, {
      version: 1,
      keys: [{ key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY }],
    });
  });

  it("rejects duplicate key ids", () => {
    assert.throws(
      () =>
        parsePublicSigningKeysDocument(
          JSON.stringify({
            version: 1,
            keys: [
              { key_id: "elydora-alpha-plugins", public_key: PUBLIC_KEY },
              { key_id: "elydora-alpha-plugins", public_key: "b".repeat(64) },
            ],
          }),
        ),
      SigningKeysSchemaError,
    );
  });

  it("rejects malformed public keys", () => {
    assert.throws(
      () =>
        parsePublicSigningKeysDocument(
          JSON.stringify({
            version: 1,
            keys: [{ key_id: "elydora-alpha-plugins", public_key: "abcd" }],
          }),
        ),
      SigningKeysSchemaError,
    );
  });
});
