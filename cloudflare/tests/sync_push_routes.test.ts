import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  type RecordedR2Put,
  sessionDocument,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

describe("retired sync push route", () => {
  it("rejects legacy object writes before D1 and R2 persistence", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: "device-01" }] });
    const r2Puts: RecordedR2Put[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/push", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ version: 1, payload: "legacy-plaintext" }),
      }),
      testEnv({
        d1,
        r2Puts,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 410);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), { error: "sync_object_protocol_retired" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
    assert.deepEqual(r2Puts, []);
  });
});
