import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyAnalyticsDataPoint } from "../src/bindings.js";
import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import { ACCESS_TOKEN, sessionDocument, testEnv } from "./devices_test_support.js";

const DEVICE_ID = "device-01";

describe("telemetry routes", () => {
  it("records a minimized authenticated diagnostic event", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const auditEvents: ElyAnalyticsDataPoint[] = [];
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      telemetryRequest({
        version: 1,
        event_type: "sync_error",
        occurred_at: 1_780_000_000,
        app_version: "0.1.0",
        platform: "macos",
        error_code: "sync.pull.5xx",
        component: "sync",
      }),
      testEnv({
        auditEvents,
        diagnosticEvents,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 202);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), { version: 1, accepted: true });
    assert.equal(diagnosticEvents.length, 1);
    assert.deepEqual(diagnosticEvents[0]?.indexes, ["local"]);
    assert.deepEqual(diagnosticEvents[0]?.blobs, [
      "sync_error",
      "0.1.0",
      "macos",
      "",
      "sync.pull.5xx",
      "sync",
      "",
      "",
      "user-01",
      DEVICE_ID,
    ]);
    assert.equal(diagnosticEvents[0]?.doubles?.[0], 1_780_000_000);
    assert.ok((diagnosticEvents[0]?.doubles?.[1] ?? 0) > 0);
    assert.deepEqual(auditEvents[0]?.blobs?.slice(0, 4), [
      "telemetry.events",
      "POST",
      "/api/telemetry/events",
      "handled",
    ]);
  });

  it("rejects diagnostic events containing sensitive fields", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      telemetryRequest({
        version: 1,
        event_type: "app_startup",
        occurred_at: 1_780_000_000,
        app_version: "0.1.0",
        platform: "macos",
        outcome: "success",
        url: "https://example.test/private?q=token",
      }),
      testEnv({
        diagnosticEvents,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "telemetry_sensitive_field" });
    assert.equal(diagnosticEvents.length, 0);
  });

  it("rejects unknown diagnostic fields", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      telemetryRequest({
        version: 1,
        event_type: "update_result",
        occurred_at: 1_780_000_000,
        app_version: "0.1.0",
        platform: "macos",
        outcome: "failure",
        metadata: { release_channel: "stable" },
      }),
      testEnv({
        diagnosticEvents,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "telemetry_unknown_field" });
    assert.equal(diagnosticEvents.length, 0);
  });

  it("rejects unsupported diagnostic event types", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      telemetryRequest({
        version: 1,
        event_type: "page_view",
        occurred_at: 1_780_000_000,
        app_version: "0.1.0",
        platform: "macos",
      }),
      testEnv({
        diagnosticEvents,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "telemetry_event_type_invalid" });
    assert.equal(diagnosticEvents.length, 0);
  });

  it("rejects unauthenticated diagnostic events before recording telemetry", async () => {
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/telemetry/events", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          version: 1,
          event_type: "app_crash",
          occurred_at: 1_780_000_000,
          app_version: "0.1.0",
          platform: "macos",
        }),
      }),
      testEnv({ diagnosticEvents }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.equal(diagnosticEvents.length, 0);
  });

  it("rejects unsupported methods before recording telemetry", async () => {
    const diagnosticEvents: ElyAnalyticsDataPoint[] = [];
    const response = await handleRequest(
      new Request("https://elydora.test/api/telemetry/events", { method: "GET" }),
      testEnv({ diagnosticEvents }),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "POST");
    assert.deepEqual(await response.json(), { error: "method_not_allowed" });
    assert.equal(diagnosticEvents.length, 0);
  });
});

function telemetryRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/telemetry/events", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}
