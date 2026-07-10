import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  compareDeviceIds,
  deviceRevocationProofBytes,
  deviceRevocationRequest,
  pendingDeviceRevocationProofBytes,
} from "../src/device_revocation_schema.js";
import { DeviceSchemaError } from "../src/device_schema.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const USER_ID = "user-01", APPROVER_ID = "device-01", TARGET_ID = "device-02";
const OLD_KEY = "a".repeat(64), NEW_KEY = "b".repeat(64);
const IDEMPOTENCY_KEY = "device-revocation-0001";
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";

describe("device revocation proof schema", () => {
  it("rejects malformed envelope recipients and rotation metadata", async () => {
    for (const body of [
      approvedBody({ envelopes: [envelope(TARGET_ID)] }),
      approvedBody({ envelopes: [envelope(APPROVER_ID), envelope(APPROVER_ID)] }),
      approvedBody({ envelopes: [envelope(APPROVER_ID, { encapped_key: `${"A".repeat(42)}B` })] }),
      approvedBody({ new_generation: 3 }),
      approvedBody({ new_key_id: OLD_KEY }),
      { ...approvedBody(), mode: undefined },
      { ...pendingBody(), new_key_id: NEW_KEY },
    ]) {
      await assert.rejects(
        deviceRevocationRequest(request({ ...body, rotation_proof: "0".repeat(128) })),
        DeviceSchemaError,
      );
    }
  });

  it("uses one ASCII code-unit order for proof and exact recipient checks", async () => {
    const ids = ["a_1", "a:1", "a.1", "a-1", "A_1", "A-1"];
    const expected = ["A-1", "A_1", "a-1", "a.1", "a:1", "a_1"];
    assert.deepEqual([...ids].sort(compareDeviceIds), expected);
    const parsed = await deviceRevocationRequest(request(
      await signedBody(approvedBody({ envelopes: ids.map((id) => envelope(id)) })),
    ));
    assert.equal(parsed.mode, "approved_rotate");
    if (parsed.mode !== "approved_rotate") throw new Error("approved rotation expected");
    assert.deepEqual(parsed.envelopes.map((item) => item.recipientDeviceId), expected);
  });

  it("uses the frozen v2 approved rotation proof wire", async () => {
    const parsed = await deviceRevocationRequest(request(
      await signedBody(approvedBody({ envelopes: [envelope(APPROVER_ID)] })),
    ));
    if (parsed.mode !== "approved_rotate") throw new Error("approved rotation expected");
    const { rotationProof: _, ...unsigned } = parsed;
    assert.equal(new TextDecoder().decode(
      deviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
    ), [
      "28:elydora-device-revocation-v2",
      "7:user-01",
      "9:device-01",
      "9:device-02",
      `64:${OLD_KEY}`,
      "1:1",
      `64:${NEW_KEY}`,
      "1:2",
      "22:device-revocation-0001",
      "1:1",
      "9:device-01",
      "1:1",
      `45:${SUITE}`,
      `43:${"A".repeat(43)}`,
      `64:${"B".repeat(64)}`,
    ].join(""));
  });

  it("uses the frozen v2 pending revocation proof wire", async () => {
    const parsed = await deviceRevocationRequest(request(await signedBody(pendingBody())));
    if (parsed.mode !== "pending_revoke") throw new Error("pending revocation expected");
    const { pendingRevocationProof: _, ...unsigned } = parsed;
    assert.equal(new TextDecoder().decode(
      pendingDeviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
    ), [
      "36:elydora-pending-device-revocation-v2",
      "7:user-01",
      "9:device-01",
      "9:device-02",
      "22:device-revocation-0001",
    ].join(""));
  });

  it("rejects invalid and tampered proofs before revocation state reads", async () => {
    const cases = [
      approvedBody({ rotation_proof: "0".repeat(128) }),
      { ...await signedBody(approvedBody()), new_key_id: "c".repeat(64) },
      pendingBody({ pending_revocation_proof: "0".repeat(128) }),
    ];
    for (const body of cases) {
      const d1 = testD1Database({ firstRows: [approverRow()] });
      const response = await handleRequest(request(body, true), testEnv({ d1 }));
      assert.equal(response.status, 403);
      assert.equal(d1.queries.length, 1);
      assert.deepEqual(d1.batches, []);
    }
  });
});

function approvedBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 2,
    mode: "approved_rotate",
    device_id: TARGET_ID,
    previous_key_id: OLD_KEY,
    previous_generation: 1,
    new_key_id: NEW_KEY,
    new_generation: 2,
    envelopes: [envelope("device-03"), envelope(APPROVER_ID)],
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function pendingBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 2,
    mode: "pending_revoke",
    device_id: TARGET_ID,
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function envelope(
  recipient: string,
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  const other = recipient === "device-03";
  return {
    recipient_device_id: recipient,
    envelope: {
      version: 1,
      suite: SUITE,
      encapped_key: other ? `${"C".repeat(42)}E` : "A".repeat(43),
      ciphertext: other ? "D".repeat(64) : "B".repeat(64),
      ...overrides,
    },
  };
}

async function signedBody(body: Record<string, unknown>): Promise<Record<string, unknown>> {
  if (body.mode === "pending_revoke") {
    if (body.pending_revocation_proof !== undefined) return body;
    const draft = { ...body, pending_revocation_proof: "0".repeat(128) };
    const parsed = await deviceRevocationRequest(request(draft));
    if (parsed.mode !== "pending_revoke") throw new Error("pending revocation expected");
    const { pendingRevocationProof: _, ...unsigned } = parsed;
    return {
      ...body,
      pending_revocation_proof: await signDeviceMessage(
        pendingDeviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
      ),
    };
  }
  if (body.rotation_proof !== undefined) return body;
  const draft = { ...body, rotation_proof: "0".repeat(128) };
  const parsed = await deviceRevocationRequest(request(draft));
  if (parsed.mode !== "approved_rotate") throw new Error("approved rotation expected");
  const { rotationProof: _, ...unsigned } = parsed;
  return {
    ...body,
    rotation_proof: await signDeviceMessage(
      deviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
    ),
  };
}

function request(body: Record<string, unknown>, authenticated = false): Request {
  return new Request("https://elydora.test/api/devices/revoke", {
    method: "POST",
    headers: {
      ...(authenticated ? { authorization: `Bearer ${ACCESS_TOKEN}` } : {}),
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function approverRow(): Record<string, unknown> {
  return { device_id: APPROVER_ID, signing_public_key: PUBLIC_KEY };
}
