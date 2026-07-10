import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { deviceApprovalProofBytes } from "../src/device_approval_proof.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  WRAPPING_PUBLIC_KEY,
  sessionDocument,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const DEVICE_APPROVAL_IDEMPOTENCY_KEY = "device-approval-0001";
const KEY_ID = "a".repeat(64);
const GENERATION = 1;
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";
const ENCAPPED_KEY = "A".repeat(43);
const CIPHERTEXT = "B".repeat(64);

describe("device approval routes", () => {
  it("matches the frozen cross-runtime approval proof vector", async () => {
    const body = await deviceApprovalBody({ proof_created_at: 1_780_000_300 });
    assert.equal(
      body.approval_proof,
      "f12fb7a5f7f20551bd22d0fcf8f5787d49f6202f89e42c332c248772fd9a59c82a9d8b6ac47ea84340170fc1555fc74d70a0d6ba3541df257882d46d6d79d901",
    );
  });

  it("approves a pending device from an approved current device", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        null,
        deviceRow({ device_id: "device-02", approval_status: "pending", approved_at: null }),
        deviceRow({
          device_id: "device-02",
          approval_status: "approved",
          approved_at: 1_780_000_300,
        }),
        approvalEnvelopeRow(),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceApprovalBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), {
      version: 1,
      user_id: "user-01",
      approved_by_device_id: "device-01",
      approved_at: 1_780_000_300,
      device: {
        device_id: "device-02",
        public_key: PUBLIC_KEY,
        wrapping_public_key: WRAPPING_PUBLIC_KEY,
        device_name: "MacBook Pro",
        platform: "macOS",
        approval_status: "approved",
        created_at: 1_780_000_000,
        approved_at: 1_780_000_300,
        last_active_at: 1_780_000_020,
        revoked_at: null,
        current: false,
      },
    });
    assert.equal(d1.batches[0], 3);
    assert.ok(d1.queries[0]?.includes("approval_status = 'approved'"));
    assert.ok(d1.queries[0]?.includes("key_protocol_version = 2"));
    assert.ok(d1.queries[1]?.includes("FROM device_approvals"));
    assert.ok(d1.queries[3]?.includes("INSERT INTO sync_vault_envelopes"));
    assert.ok(d1.queries[4]?.includes("INSERT INTO device_approvals"));
    assert.ok(d1.queries[5]?.includes("UPDATE user_devices"));
    assert.ok(d1.queries[5]?.includes("sync_vault_envelopes"));
    assert.ok(d1.queries[7]?.includes("current_key_id"));
    assert.deepEqual(d1.binds[0], ["user-01", "device-01"]);
    assert.deepEqual(d1.binds[1], ["user-01", DEVICE_APPROVAL_IDEMPOTENCY_KEY]);
    assert.deepEqual(d1.binds[2], ["user-01", "device-02"]);
    assert.deepEqual(d1.binds[3]?.slice(0, 5), [
      "user-01",
      "device-02",
      "device-01",
      KEY_ID,
      GENERATION,
    ]);
    assert.deepEqual(d1.binds[4]?.slice(0, 4), [
      "user-01",
      DEVICE_APPROVAL_IDEMPOTENCY_KEY,
      "device-02",
      "device-01",
    ]);
    assert.equal(d1.binds[4]?.[7], DEVICE_APPROVAL_IDEMPOTENCY_KEY);
    assert.deepEqual(d1.binds[6], ["user-01", "device-02"]);
    assert.deepEqual(d1.binds[7], ["user-01", "device-02"]);
  });

  it("returns the existing approval for an idempotent replay", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        {
          device_id: "device-02",
          requester_device_id: "device-01",
          status: "approved",
          decided_at: 1_780_000_300,
        },
        deviceRow({
          device_id: "device-02",
          approval_status: "approved",
          approved_at: 1_780_000_300,
        }),
        approvalEnvelopeRow(),
      ],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceApprovalBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 200);
    const body = (await response.json()) as { approved_at: number };
    assert.equal(body.approved_at, 1_780_000_300);
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 4);
    assert.deepEqual(d1.binds, [
      ["user-01", "device-01"],
      ["user-01", DEVICE_APPROVAL_IDEMPOTENCY_KEY],
      ["user-01", "device-02"],
      ["user-01", "device-02"],
    ]);
  });

  it("rejects an approval replay with different wrapped key material", async () => {
    const d1 = testD1Database({
      firstRows: [
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        {
          device_id: "device-02",
          requester_device_id: "device-01",
          status: "approved",
          decided_at: 1_780_000_300,
        },
        deviceRow({ device_id: "device-02", approval_status: "approved" }),
        approvalEnvelopeRow({ ciphertext: "C".repeat(64) }),
      ],
    });

    const response = await approvalRequest(d1, await deviceApprovalBody());

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects approval replays with a different current key or generation", async () => {
    for (const override of [{ key_id: "b".repeat(64) }, { generation: 2 }]) {
      const d1 = testD1Database({
        firstRows: [
          deviceRow({ device_id: "device-01", approval_status: "approved" }),
          {
            device_id: "device-02",
            requester_device_id: "device-01",
            status: "approved",
            decided_at: 1_780_000_300,
          },
          deviceRow({ device_id: "device-02", approval_status: "approved" }),
          approvalEnvelopeRow(),
        ],
      });

      const response = await approvalRequest(d1, await deviceApprovalBody(override));

      assert.equal(response.status, 403);
      assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
      assert.deepEqual(d1.batches, []);
    }
  });

  it("keeps the target pending when the current vault envelope cannot be written", async () => {
    const d1 = testD1Database({
      firstRows: [
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        null,
        deviceRow({ device_id: "device-02", approval_status: "pending", approved_at: null }),
        deviceRow({ device_id: "device-02", approval_status: "pending", approved_at: null }),
      ],
    });

    const response = await approvalRequest(d1, await deviceApprovalBody());

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "device_approval_conflict" });
    assert.ok(d1.queries[4]?.includes("WHERE EXISTS"));
    assert.ok(d1.queries[5]?.includes("AND EXISTS"));
  });

  it("rejects approval from a current device that is not approved", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [null] });
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceApprovalBody()),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects a tampered current-device proof before approval state reads", async () => {
    const d1 = testD1Database({
      firstRows: [deviceRow({ device_id: "device-01", approval_status: "approved" })],
    });
    const body = await deviceApprovalBody();
    body.approval_proof = "0".repeat(128);
    const response = await approvalRequest(d1, body);

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 1);
  });

  it("requires a recent proof for a new approval", async () => {
    const d1 = testD1Database({
      firstRows: [
        deviceRow({ device_id: "device-01", approval_status: "approved" }),
        null,
      ],
    });
    const response = await approvalRequest(
      d1,
      await deviceApprovalBody({ proof_created_at: 1_700_000_000 }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 2);
  });

  it("rejects self approval before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceApprovalBody({ device_id: "device-01" })),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(d1.queries, []);
  });

  it("rejects invalid approval payloads before D1 writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await deviceApprovalBody({ idempotency_key: "short" })),
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument("device-01")]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_device_approval" });
    assert.deepEqual(d1.queries, []);
  });

  it("rejects unauthenticated device approval before D1 writes", async () => {
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/devices/approve", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(await deviceApprovalBody()),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: "authorization_missing" });
    assert.deepEqual(d1.queries, []);
  });
});

async function deviceApprovalBody(
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  const body: Record<string, unknown> = {
    version: 2,
    device_id: "device-02",
    key_id: KEY_ID,
    generation: GENERATION,
    envelope: wrappedEnvelope(),
    idempotency_key: DEVICE_APPROVAL_IDEMPOTENCY_KEY,
    proof_created_at: Math.floor(Date.now() / 1000),
    ...overrides,
  };
  const envelope = body.envelope as {
    version: 1;
    suite: typeof SUITE;
    encapped_key: string;
    ciphertext: string;
  };
  body.approval_proof = await signDeviceMessage(deviceApprovalProofBytes(
    "user-01",
    "device-01",
    {
      deviceId: String(body.device_id),
      keyId: String(body.key_id),
      generation: Number(body.generation),
      envelope,
      idempotencyKey: String(body.idempotency_key),
      proofCreatedAt: Number(body.proof_created_at),
    },
  ));
  return body;
}

function deviceRow(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    device_id: "device-01",
    public_key: PUBLIC_KEY,
    signing_public_key: PUBLIC_KEY,
    wrapping_public_key: WRAPPING_PUBLIC_KEY,
    device_name: "MacBook Pro",
    platform: "macOS",
    approval_status: "approved",
    created_at: 1_780_000_000,
    approved_at: 1_780_000_010,
    last_active_at: 1_780_000_020,
    revoked_at: null,
    ...overrides,
  };
}

function wrappedEnvelope(): Record<string, unknown> {
  return {
    version: 1,
    suite: SUITE,
    encapped_key: ENCAPPED_KEY,
    ciphertext: CIPHERTEXT,
  };
}

function approvalEnvelopeRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    key_id: KEY_ID,
    generation: GENERATION,
    recipient_device_id: "device-02",
    approver_device_id: "device-01",
    envelope_version: 1,
    suite: SUITE,
    encapped_key: ENCAPPED_KEY,
    ciphertext: CIPHERTEXT,
    idempotency_key: DEVICE_APPROVAL_IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function approvalRequest(
  d1: ReturnType<typeof testD1Database>,
  body: Record<string, unknown>,
): Promise<Response> {
  return handleRequest(
    new Request("https://elydora.test/api/devices/approve", {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    }),
    testEnv({ d1 }),
  );
}
