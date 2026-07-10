import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { payloadBytes, SyncSnapshotRequestError } from "../src/sync_snapshot_codec.js";

describe("sync snapshot codec", () => {
  it("rejects oversized base64 before decoding", () => {
    const originalAtob = globalThis.atob;
    let decoded = false;
    globalThis.atob = () => {
      decoded = true;
      return "";
    };
    try {
      assert.throws(
        () => payloadBytes("AAAAAAAA", "data_base64", 3),
        (error) =>
          error instanceof SyncSnapshotRequestError &&
          error.message === "data_base64_size_invalid",
      );
    } finally {
      globalThis.atob = originalAtob;
    }
    assert.equal(decoded, false);
  });
});
