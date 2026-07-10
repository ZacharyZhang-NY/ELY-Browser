import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  WRAPPING_PUBLIC_KEY,
  deviceRegistrationBody,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

describe("device trust routes", () => {
  it("atomically approves the first v2 device and stores both public keys", async () => {
    const device = {
      device_id: "device-01",
      public_key: PUBLIC_KEY,
      wrapping_public_key: WRAPPING_PUBLIC_KEY,
      device_name: "MacBook Pro",
      platform: "macOS",
      approval_status: "approved",
      created_at: 1_780_000_100,
      approved_at: 1_780_000_100,
      last_active_at: 1_780_000_100,
      revoked_at: null,
    };
    const d1 = testD1Database({
      firstRows: [device],
      sessionRow: {
        id: "session-01",
        userId: "user-01",
        expiresAt: "2099-01-01T00:00:00.000Z",
        createdAt: new Date().toISOString(),
        deviceId: null,
      },
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/register", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceRegistrationBody()),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 201);
    assert.equal(((await response.json()) as { device: { approval_status: string } }).device.approval_status, "approved");
    assert.ok(d1.queries.some((query) => query.includes("NOT EXISTS")));
    assert.ok(
      d1.queries.some(
        (query) =>
          query.includes("user_device_keys") &&
          query.includes("device_name = ?") &&
          query.includes("idempotency_key = ?"),
      ),
    );
  });

  it("keeps subsequent v2 devices pending", async () => {
    const device = deviceRow({ approval_status: "pending", approved_at: null });
    const d1 = testD1Database({
      firstRows: [device],
      sessionRow: unboundSession(),
    });
    const response = await registerRequest(d1, await deviceRegistrationBody());

    assert.equal(response.status, 201);
    const body = (await response.json()) as { device: { approval_status: string } };
    assert.equal(body.device.approval_status, "pending");
  });

  it("requires a fresh session before registering an unbound device", async () => {
    const d1 = testD1Database({
      sessionRow: { ...unboundSession(), createdAt: "2020-01-01T00:00:00.000Z" },
    });
    const response = await registerRequest(d1, await deviceRegistrationBody());

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_registration_forbidden" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects v1 and non-canonical v2 registration keys before D1 writes", async () => {
    for (const registration of [
      { ...(await deviceRegistrationBody()), version: 1 },
      await deviceRegistrationBody({ public_key: PUBLIC_KEY.toUpperCase() }),
      await deviceRegistrationBody({ wrapping_public_key: WRAPPING_PUBLIC_KEY.toUpperCase() }),
    ]) {
      const d1 = testD1Database({ sessionRow: unboundSession() });
      const response = await registerRequest(d1, registration);

      assert.equal(response.status, 400);
      assert.deepEqual(d1.queries, []);
    }
  });

  it("rejects a tampered registration proof before D1 writes", async () => {
    const registration = await deviceRegistrationBody();
    registration.wrapping_public_key = "c".repeat(64);
    const d1 = testD1Database({ sessionRow: unboundSession() });
    const response = await registerRequest(d1, registration);

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_registration_forbidden" });
    assert.deepEqual(d1.queries, []);
  });

  it("preserves an existing session binding that wins a registration race", async () => {
    const d1 = testD1Database({
      firstRows: [deviceRow()],
      runChanges: [0],
      sessionRow: unboundSession(),
    });
    const response = await registerRequest(d1, await deviceRegistrationBody());

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "device_registration_conflict" });
    assert.ok(d1.queries.at(-1)?.includes("ON CONFLICT(session_id) DO NOTHING"));
  });

  it("issues a short-lived challenge only for an approved v2 device", async () => {
    const { challenge, d1 } = await issueChallenge();
    const nowSeconds = Math.floor(Date.now() / 1000);

    assert.match(challenge.challenge_id, /^[0-9a-f-]{36}$/);
    assert.match(challenge.challenge, /^elydora-device-rebind-v1\n/);
    assert.ok(challenge.expires_at - nowSeconds >= 299);
    assert.ok(challenge.expires_at - nowSeconds <= 300);
    assert.ok(d1.queries[0]?.includes("key_protocol_version = 2"));
    assert.ok(d1.queries[1]?.includes("ON CONFLICT(session_id) DO UPDATE"));
    assert.deepEqual(d1.binds[1]?.slice(1, 4), ["user-01", "session-01", "device-01"]);
  });

  it("rebinds an unbound session after a valid Ed25519 challenge signature", async () => {
    const { challenge } = await issueChallenge();
    const signature = await signDeviceMessage(new TextEncoder().encode(challenge.challenge));
    const d1 = rebindDatabase(challenge);
    const response = await rebindRequest(d1, challenge, signature);

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: "user-01",
      session_id: "session-01",
      device_id: "device-01",
      bound_at: d1.binds[1]?.[0],
    });
    assert.equal(d1.batches[0], 2);
    assert.ok(d1.queries[1]?.includes("consumed_at IS NULL"));
    assert.ok(d1.queries[1]?.includes("session_id = ?"));
    assert.ok(d1.queries[2]?.includes("ON CONFLICT(session_id) DO NOTHING"));
  });

  it("rejects invalid signatures without consuming the challenge", async () => {
    const { challenge } = await issueChallenge();
    const d1 = rebindDatabase(challenge);
    const response = await rebindRequest(d1, challenge, "00".repeat(64));

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_rebind_forbidden" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects expired and replayed challenges", async () => {
    const { challenge } = await issueChallenge();
    const signature = await signDeviceMessage(new TextEncoder().encode(challenge.challenge));
    const expiredD1 = rebindDatabase({ ...challenge, expires_at: 1 });
    const expiredResponse = await rebindRequest(expiredD1, challenge, signature);
    assert.equal(expiredResponse.status, 403);
    assert.deepEqual(expiredD1.batches, []);

    const replayD1 = rebindDatabase(challenge, [[0, 0]]);
    const replayResponse = await rebindRequest(replayD1, challenge, signature);
    assert.equal(replayResponse.status, 409);
    assert.deepEqual(await replayResponse.json(), { error: "device_rebind_conflict" });
  });
});

interface ChallengeDocument {
  challenge_id: string;
  device_id: string;
  challenge: string;
  expires_at: number;
}

async function registerRequest(
  d1: ReturnType<typeof testD1Database>,
  registration: Record<string, unknown>,
): Promise<Response> {
  return handleRequest(
    new Request("https://elydora.test/api/devices/register", {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(registration),
    }),
    testEnv({ d1 }),
  );
}

async function issueChallenge(): Promise<{
  challenge: ChallengeDocument;
  d1: ReturnType<typeof testD1Database>;
}> {
  const d1 = testD1Database({
    firstRows: [{ signing_public_key: PUBLIC_KEY }],
    sessionRow: unboundSession(),
  });
  const response = await handleRequest(
    new Request("https://elydora.test/api/devices/rebind/challenge", {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({ version: 1, device_id: "device-01" }),
    }),
    testEnv({ d1 }),
  );
  assert.equal(response.status, 201);
  return { challenge: (await response.json()) as ChallengeDocument, d1 };
}

function rebindDatabase(
  challenge: ChallengeDocument,
  batchChanges: number[][] = [[1, 1]],
): ReturnType<typeof testD1Database> {
  return testD1Database({
    batchChanges,
    firstRows: [
      {
        challenge: challenge.challenge,
        expires_at: challenge.expires_at,
        signing_public_key: PUBLIC_KEY,
      },
    ],
    sessionRow: unboundSession(),
  });
}

async function rebindRequest(
  d1: ReturnType<typeof testD1Database>,
  challenge: ChallengeDocument,
  signature: string,
): Promise<Response> {
  return handleRequest(
    new Request("https://elydora.test/api/devices/rebind", {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        version: 1,
        challenge_id: challenge.challenge_id,
        device_id: challenge.device_id,
        signature,
      }),
    }),
    testEnv({ d1 }),
  );
}

function unboundSession(): Record<string, unknown> {
  return {
    id: "session-01",
    userId: "user-01",
    expiresAt: "2099-01-01T00:00:00.000Z",
    createdAt: new Date().toISOString(),
    deviceId: null,
  };
}

function deviceRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    device_id: "device-01",
    public_key: PUBLIC_KEY,
    wrapping_public_key: WRAPPING_PUBLIC_KEY,
    device_name: "MacBook Pro",
    platform: "macOS",
    approval_status: "approved",
    created_at: 1_780_000_100,
    approved_at: 1_780_000_100,
    last_active_at: 1_780_000_100,
    revoked_at: null,
    ...overrides,
  };
}
