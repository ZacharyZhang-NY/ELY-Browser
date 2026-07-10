import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import {
  deviceRevocationRequest,
  deviceRevocationRequestHash,
  deviceRevocationProofBytes,
  pendingDeviceRevocationProofBytes,
  pendingDeviceRevocationRequestHash,
} from "../src/device_revocation_schema.js";
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

const USER_ID = "user-01", APPROVER_DEVICE_ID = "device-01";
const TARGET_DEVICE_ID = "device-02", OTHER_DEVICE_ID = "device-03";
const IDEMPOTENCY_KEY = "device-revocation-0001";
const PREVIOUS_KEY_ID = "a".repeat(64), NEW_KEY_ID = "b".repeat(64);
const PREVIOUS_GENERATION = 1, NEW_GENERATION = 2;
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";
describe("device revocation routes", () => {
  it("atomically rotates the vault and revokes an approved device", async () => {
    const body = deviceRevocationBody();
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        null,
        currentVaultKeyRow(),
        deviceRow({ device_id: TARGET_DEVICE_ID }),
        await rotationResultRow(body, { r2_object_count: 2, r2_item_count: 2 }),
        deviceRow({ device_id: TARGET_DEVICE_ID, approval_status: "revoked", revoked_at: 1_780_000_400 }),
      ],
      allRowSets: [
        [{ device_id: APPROVER_DEVICE_ID }, { device_id: OTHER_DEVICE_ID }],
        [{ object_count: 2 }],
      ],
      batchChanges: [[1, 1, 1, 1]],
    });

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 200);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), revocationDocument());
    assert.deepEqual(d1.batches, [4]);
    assert.ok(d1.queries[6]?.includes("INSERT INTO sync_vault_rotations"));
    assert.ok(d1.queries[7]?.includes("INSERT INTO sync_vault_rotation_envelopes"));
    assert.ok(d1.queries[5]?.includes("COUNT(*) AS object_count"));
    assert.ok(d1.queries[9]?.includes("SET completed_at = ?"));
    assert.deepEqual(d1.binds[6]?.slice(0, 2), [USER_ID, IDEMPOTENCY_KEY]);
    assert.match(String(d1.binds[6]?.[2]), /^device-revoke:[a-f0-9]{64}$/);
    assert.deepEqual(d1.binds[6]?.slice(3, 5), [TARGET_DEVICE_ID, APPROVER_DEVICE_ID]);
  });

  it("revokes a pending target without rotating or cleaning the vault", async () => {
    const body = pendingRevocationBody();
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        null,
        deviceRow({ approval_status: "pending", approved_at: null }),
        await pendingResultRow(body),
        deviceRow({ approval_status: "revoked", approved_at: null, revoked_at: 1_780_000_400 }),
      ],
      batchChanges: [[1, 1]],
    });

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), pendingRevocationDocument());
    assert.deepEqual(d1.batches, [2]);
    assert.ok(d1.queries[3]?.includes("INSERT INTO pending_device_revocations"));
    assert.ok(d1.queries[4]?.includes("SET completed_at = ?"));
    assert.equal(d1.queries.some((query) => query.includes("sync_vault_accounts")), false);
  });

  it("returns an exact pending revocation replay and rejects mismatches", async () => {
    const body = pendingRevocationBody();
    const replay = testD1Database({
      firstRows: [
        approverRow(),
        await pendingResultRow(body),
        deviceRow({ approval_status: "revoked", approved_at: null, revoked_at: 1_780_000_400 }),
      ],
    });
    assert.equal((await revocationResponse(replay, body)).status, 200);
    assert.deepEqual(replay.batches, []);

    const mismatch = testD1Database({
      firstRows: [approverRow(), { ...await pendingResultRow(body), request_hash: "f".repeat(64) }],
    });
    assert.equal((await revocationResponse(mismatch, body)).status, 409);
    assert.deepEqual(mismatch.batches, []);
  });

  it("rejects pending mode for an approved target and trigger races", async () => {
    const body = pendingRevocationBody();
    const approved = testD1Database({
      firstRows: [approverRow(), null, deviceRow()],
    });
    assert.equal((await revocationResponse(approved, body)).status, 409);
    assert.deepEqual(approved.batches, []);

    const race = testD1Database({
      firstRows: [approverRow(), null, deviceRow({ approval_status: "pending", approved_at: null })],
      batchError: new Error("pending_device_revocation_guard_failed"),
    });
    assert.equal((await revocationResponse(race, body)).status, 409);
  });

  it("returns an exact idempotent replay", async () => {
    const body = deviceRevocationBody();
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        await rotationResultRow(body),
        deviceRow({ device_id: TARGET_DEVICE_ID, approval_status: "revoked", revoked_at: 1_780_000_400 }),
      ],
    });

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), revocationDocument());
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 3);
  });

  it("rejects a replay with different rotation metadata", async () => {
    const body = deviceRevocationBody();
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        await rotationResultRow(body, { new_key_id: "c".repeat(64) }),
      ],
    });

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "device_revocation_conflict" });
    assert.deepEqual(d1.batches, []);
  });

  it("rejects missing and extra recipient envelopes", async () => {
    const cases = [
      {
        body: deviceRevocationBody({ envelopes: [rotationEnvelope(APPROVER_DEVICE_ID)] }),
        rows: [{ device_id: APPROVER_DEVICE_ID }, { device_id: OTHER_DEVICE_ID }],
      },
      {
        body: deviceRevocationBody(),
        rows: [{ device_id: APPROVER_DEVICE_ID }],
      },
    ];
    for (const testCase of cases) {
      const d1 = preflightD1(testCase.rows);
      const response = await revocationResponse(d1, testCase.body);
      assert.equal(response.status, 409);
      assert.deepEqual(await response.json(), { error: "device_revocation_conflict" });
      assert.deepEqual(d1.batches, []);
    }
  });

  it("rejects stale vault metadata before target reads", async () => {
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        null,
        { key_id: "c".repeat(64), generation: PREVIOUS_GENERATION },
      ],
    });

    const response = await revocationResponse(d1, deviceRevocationBody());

    assert.equal(response.status, 409);
    assert.deepEqual(d1.batches, []);
    assert.equal(d1.queries.length, 3);
  });

  it("fails a rotation race when the D1 guard aborts", async () => {
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        null,
        currentVaultKeyRow(),
        deviceRow({ device_id: TARGET_DEVICE_ID }),
      ],
      allRowSets: [
        [{ device_id: APPROVER_DEVICE_ID }, { device_id: OTHER_DEVICE_ID }],
        [{ object_count: 0 }],
      ],
      batchError: new Error("sync_vault_rotation_guard_failed"),
    });

    const response = await revocationResponse(d1, deviceRevocationBody());

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "device_revocation_conflict" });
  });

  it("fails closed when a zero-change finalize has no exact completed replay", async () => {
    const body = deviceRevocationBody();
    const d1 = testD1Database({
      firstRows: [
        approverRow(),
        null,
        currentVaultKeyRow(),
        deviceRow({ device_id: TARGET_DEVICE_ID }),
        null,
      ],
      allRowSets: [
        [{ device_id: APPROVER_DEVICE_ID }, { device_id: OTHER_DEVICE_ID }],
        [{ object_count: 0 }],
      ],
      batchChanges: [[1, 1, 1, 0]],
    });

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 409);
    assert.deepEqual(d1.batches, [4]);
  });

  it("accepts a zero-change finalize that resolves to the exact concurrent replay", async () => {
    const body = deviceRevocationBody();
    const d1 = successfulD1(body, "approved", [[1, 1, 1, 0]]);

    const response = await revocationResponse(d1, body);

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), revocationDocument());
  });

  it("rejects unapproved, self, and unauthenticated revocation", async () => {
    const unapproved = testD1Database({ firstRows: [null] });
    assert.equal((await revocationResponse(unapproved, deviceRevocationBody())).status, 403);

    const self = testD1Database([]);
    const selfBody = deviceRevocationBody({
      device_id: APPROVER_DEVICE_ID,
      envelopes: [rotationEnvelope(OTHER_DEVICE_ID)],
    });
    assert.equal((await revocationResponse(self, selfBody)).status, 403);
    assert.deepEqual(self.queries, []);

    const anonymous = testD1Database([]);
    const response = await handleRequest(revocationRequest(deviceRevocationBody(), false), testEnv({ d1: anonymous }));
    assert.equal(response.status, 401);
    assert.deepEqual(anonymous.queries, []);
  });
});

function successfulD1(
  body: Record<string, unknown>,
  targetStatus: "pending" | "approved",
  batchChanges: number[][] = [[1, 1, 1, 1]],
): ReturnType<typeof testD1Database> {
  return testD1Database({
    firstRows: [
      approverRow(),
      null,
      currentVaultKeyRow(),
      deviceRow({ device_id: TARGET_DEVICE_ID, approval_status: targetStatus }),
      rotationResultRow(body),
      deviceRow({ device_id: TARGET_DEVICE_ID, approval_status: "revoked", revoked_at: 1_780_000_400 }),
    ],
    allRowSets: [
      [{ device_id: APPROVER_DEVICE_ID }, { device_id: OTHER_DEVICE_ID }],
      [{ object_count: 0 }],
    ],
    batchChanges,
  });
}

function preflightD1(recipientRows: Record<string, unknown>[]): ReturnType<typeof testD1Database> {
  return testD1Database({
    firstRows: [
      approverRow(),
      null,
      currentVaultKeyRow(),
      deviceRow({ device_id: TARGET_DEVICE_ID }),
    ],
    allRowSets: [recipientRows, [{ object_count: 0 }]],
  });
}

function revocationResponse(
  d1: ReturnType<typeof testD1Database>,
  body: Record<string, unknown>,
): Promise<Response> {
  return signedRevocationBody(body).then((signedBody) =>
    handleRequest(revocationRequest(signedBody), testEnv({ d1 })),
  );
}

function revocationRequest(body: Record<string, unknown>, authenticated = true): Request {
  return new Request("https://elydora.test/api/devices/revoke", {
    method: "POST",
    headers: {
      ...(authenticated ? { authorization: `Bearer ${ACCESS_TOKEN}` } : {}),
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function deviceRevocationBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 2,
    mode: "approved_rotate",
    device_id: TARGET_DEVICE_ID,
    previous_key_id: PREVIOUS_KEY_ID,
    previous_generation: PREVIOUS_GENERATION,
    new_key_id: NEW_KEY_ID,
    new_generation: NEW_GENERATION,
    envelopes: [rotationEnvelope(OTHER_DEVICE_ID), rotationEnvelope(APPROVER_DEVICE_ID)],
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function pendingRevocationBody(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 2,
    mode: "pending_revoke",
    device_id: TARGET_DEVICE_ID,
    idempotency_key: IDEMPOTENCY_KEY,
    ...overrides,
  };
}

function rotationEnvelope(
  recipientDeviceId: string,
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  const other = recipientDeviceId === OTHER_DEVICE_ID;
  return {
    recipient_device_id: recipientDeviceId,
    envelope: {
      version: 1,
      suite: SUITE,
      encapped_key: other ? `${"C".repeat(42)}E` : "A".repeat(43),
      ciphertext: other ? "D".repeat(64) : "B".repeat(64),
      ...overrides,
    },
  };
}

function currentVaultKeyRow(): Record<string, unknown> { return { key_id: PREVIOUS_KEY_ID, generation: PREVIOUS_GENERATION }; }

async function rotationResultRow(
  body: Record<string, unknown>,
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  const parsed = await deviceRevocationRequest(revocationRequest(await signedRevocationBody(body)));
  if (parsed.mode !== "approved_rotate") throw new Error("approved rotation expected");
  const requestHash = await deviceRevocationRequestHash(USER_ID, APPROVER_DEVICE_ID, parsed);
  return {
    target_device_id: TARGET_DEVICE_ID,
    approver_device_id: APPROVER_DEVICE_ID,
    previous_key_id: PREVIOUS_KEY_ID,
    previous_generation: PREVIOUS_GENERATION,
    new_key_id: NEW_KEY_ID,
    new_generation: NEW_GENERATION,
    request_hash: requestHash,
    envelope_count: 2,
    r2_object_count: 0,
    completed_at: 1_780_000_400,
    current_key_id: NEW_KEY_ID,
    current_generation: NEW_GENERATION,
    target_status: "revoked",
    revoked_at: 1_780_000_400,
    active_session_count: 0,
    item_count: 2,
    r2_item_count: 0,
    persisted_count: 2,
    audit_count: 1,
    ...overrides,
  };
}

async function pendingResultRow(body: Record<string, unknown>): Promise<Record<string, unknown>> {
  const parsed = await deviceRevocationRequest(revocationRequest(await signedRevocationBody(body)));
  if (parsed.mode !== "pending_revoke") throw new Error("pending revocation expected");
  return {
    target_device_id: TARGET_DEVICE_ID,
    approver_device_id: APPROVER_DEVICE_ID,
    request_hash: await pendingDeviceRevocationRequestHash(USER_ID, APPROVER_DEVICE_ID, parsed),
    completed_at: 1_780_000_400,
    target_status: "revoked",
    revoked_at: 1_780_000_400,
    active_session_count: 0,
    audit_count: 1,
  };
}

async function signedRevocationBody(
  body: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  if (body.mode === "pending_revoke") {
    if (body.pending_revocation_proof !== undefined) return body;
    const draft = { ...body, pending_revocation_proof: "0".repeat(128) };
    try {
      const parsed = await deviceRevocationRequest(revocationRequest(draft));
      if (parsed.mode !== "pending_revoke") return draft;
      const { pendingRevocationProof: _, ...unsigned } = parsed;
      return {
        ...body,
        pending_revocation_proof: await signDeviceMessage(
          pendingDeviceRevocationProofBytes(USER_ID, APPROVER_DEVICE_ID, unsigned),
        ),
      };
    } catch {
      return draft;
    }
  }
  if (body.rotation_proof !== undefined) return body;
  const draft = { ...body, rotation_proof: "0".repeat(128) };
  try {
    const parsed = await deviceRevocationRequest(revocationRequest(draft));
    if (parsed.mode !== "approved_rotate") return draft;
    const { rotationProof: _, ...unsigned } = parsed;
    return {
      ...body,
      rotation_proof: await signDeviceMessage(
        deviceRevocationProofBytes(USER_ID, APPROVER_DEVICE_ID, unsigned),
      ),
    };
  } catch {
    return draft;
  }
}

function approverRow(): Record<string, unknown> { return { device_id: APPROVER_DEVICE_ID, signing_public_key: PUBLIC_KEY }; }

function deviceRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    device_id: TARGET_DEVICE_ID,
    public_key: PUBLIC_KEY,
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

function revocationDocument(): Record<string, unknown> {
  return {
    version: 2,
    mode: "approved_rotate",
    user_id: USER_ID,
    revoked_by_device_id: APPROVER_DEVICE_ID,
    revoked_at: 1_780_000_400,
    key_id: NEW_KEY_ID,
    generation: NEW_GENERATION,
    device: {
      device_id: TARGET_DEVICE_ID,
      public_key: PUBLIC_KEY,
      wrapping_public_key: WRAPPING_PUBLIC_KEY,
      device_name: "MacBook Pro",
      platform: "macOS",
      approval_status: "revoked",
      created_at: 1_780_000_000,
      approved_at: 1_780_000_010,
      last_active_at: 1_780_000_020,
      revoked_at: 1_780_000_400,
      current: false,
    },
  };
}

function pendingRevocationDocument(): Record<string, unknown> {
  const document = revocationDocument();
  delete document.key_id;
  delete document.generation;
  return {
    ...document,
    mode: "pending_revoke",
    device: {
      ...(document.device as Record<string, unknown>),
      approval_status: "revoked",
      approved_at: null,
    },
  };
}
