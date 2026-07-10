import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { deviceApprovalProofBytes } from "../src/device_approval_proof.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  WRAPPING_PUBLIC_KEY,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const KEY_ID = "a".repeat(64);
const IDEMPOTENCY_KEY = "device-approval-0001";
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305" as const;

describe("device approval stored state", () => {
  it("reports a malformed requester key as a persistence failure", async () => {
    await assertPersistenceFailure([{ device_id: "device-01", signing_public_key: "invalid" }]);
  });

  it("reports malformed approval metadata as a persistence failure", async () => {
    await assertPersistenceFailure([
      deviceRow(),
      approvalRow({ device_id: 2 }),
    ]);
  });

  it("reports an invalid approval status as a persistence failure", async () => {
    await assertPersistenceFailure([
      deviceRow(),
      approvalRow({ status: "corrupt" }),
    ]);
  });

  it("treats a valid pending row as an idempotency mismatch", async () => {
    const d1 = testD1Database({
      firstRows: [deviceRow(), approvalRow({ status: "pending", decided_at: null })],
    });
    const response = await approvalRequest(d1);

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "device_approval_forbidden" });
    assert.deepEqual(d1.batches, []);
  });

  it("reports malformed device state as a persistence failure", async () => {
    await assertPersistenceFailure([
      deviceRow(),
      null,
      deviceRow({ device_id: "device-02", approval_status: "pending", created_at: "invalid" }),
    ]);
  });

  it("reports a missing approved-device key row as a persistence failure", async () => {
    await assertPersistenceFailure([
      deviceRow(),
      approvalRow(),
      deviceRow({ device_id: "device-02", wrapping_public_key: null }),
    ]);
  });

  it("reports malformed envelope state as a persistence failure", async () => {
    await assertPersistenceFailure([
      deviceRow(),
      approvalRow(),
      deviceRow({ device_id: "device-02" }),
      approvalEnvelopeRow({ generation: "1" }),
    ]);
  });
});

async function assertPersistenceFailure(firstRows: unknown[]): Promise<void> {
  const d1 = testD1Database({ firstRows });
  const response = await approvalRequest(d1);
  assert.equal(response.status, 500);
  assert.deepEqual(await response.json(), { error: "device_approval_failed" });
  assert.deepEqual(d1.batches, []);
}

async function approvalRequest(d1: ReturnType<typeof testD1Database>): Promise<Response> {
  return handleRequest(
    new Request("https://elydora.test/api/devices/approve", {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(await approvalBody()),
    }),
    testEnv({ d1 }),
  );
}

async function approvalBody(): Promise<Record<string, unknown>> {
  const proofCreatedAt = Math.floor(Date.now() / 1000);
  const envelope = {
    version: 1 as const,
    suite: SUITE,
    encapped_key: "A".repeat(43),
    ciphertext: "B".repeat(64),
  };
  const action = {
    deviceId: "device-02",
    keyId: KEY_ID,
    generation: 1,
    envelope,
    idempotencyKey: IDEMPOTENCY_KEY,
    proofCreatedAt,
  };
  return {
    version: 2,
    device_id: action.deviceId,
    key_id: action.keyId,
    generation: action.generation,
    envelope,
    idempotency_key: action.idempotencyKey,
    proof_created_at: proofCreatedAt,
    approval_proof: await signDeviceMessage(
      deviceApprovalProofBytes("user-01", "device-01", action),
    ),
  };
}

function approvalRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    device_id: "device-02",
    requester_device_id: "device-01",
    status: "approved",
    decided_at: 1_780_000_300,
    ...overrides,
  };
}

function deviceRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
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

function approvalEnvelopeRow(overrides: Record<string, unknown>): Record<string, unknown> {
  return {
    key_id: KEY_ID,
    generation: 1,
    recipient_device_id: "device-02",
    approver_device_id: "device-01",
    envelope_version: 1,
    suite: SUITE,
    encapped_key: "A".repeat(43),
    ciphertext: "B".repeat(64),
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}
