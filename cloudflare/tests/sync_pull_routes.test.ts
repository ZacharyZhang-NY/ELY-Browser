import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { ACCESS_TOKEN, sessionDocument, testD1Database, testEnv } from "./devices_test_support.js";

describe("retired sync pull route", () => {
  it("rejects legacy object reads after device authorization", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: "device-01" }] });
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=0", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 410);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), { error: "sync_object_protocol_retired" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("keeps retired object reads behind authentication", async () => {
    const d1 = testD1Database({});
    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/pull?cursor=0"),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });
});
