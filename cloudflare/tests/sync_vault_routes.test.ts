import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { authSessionCacheKvKey, authTokenHash } from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  SyncVaultConflictError,
  SyncVaultNotFoundError,
  assertCurrentSyncVaultKey,
  parseWrappedAccountKey,
  syncVaultRecipientEnvelopeStatement,
} from "../src/sync_vault.js";
import { syncVaultBootstrapProofBytes } from "../src/sync_vault_bootstrap_proof.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  sessionDocument,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const KEY_ID = "a".repeat(64);
const GENERATION = 1;
const HISTORICAL_KEY_ID = "c".repeat(64);
const HISTORICAL_GENERATION = 3;
const IDEMPOTENCY_KEY = "sync-vault-bootstrap-0001";
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";
const ENCAPPED_KEY = "A".repeat(43);
const CIPHERTEXT = "B".repeat(64);

describe("sync vault routes", () => {
  it("bootstraps the current approved device envelope", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [approvedDeviceRow(), signingKeyRow(), currentEnvelopeRow()],
    });

    const response = await handleRequest(
      vaultBootstrapRequest(await vaultBootstrapBody()),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 201);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(await response.json(), vaultDocument());
    assert.equal(d1.batches[0], 2);
    assert.ok(d1.queries[1]?.includes("keys.signing_public_key"));
    assert.ok(d1.queries[1]?.includes("keys.key_protocol_version = 2"));
    assert.ok(d1.queries[2]?.includes("INSERT INTO sync_vault_accounts"));
    assert.ok(d1.queries[3]?.includes("INSERT INTO sync_vault_envelopes"));
    assert.ok(d1.queries[4]?.includes("FROM sync_vault_accounts AS accounts"));
    assert.deepEqual(d1.binds[4], [USER_ID, DEVICE_ID]);
  });

  it("returns the current device envelope", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [{ device_id: DEVICE_ID }, currentEnvelopeRow()],
    });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/vault", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), vaultDocument());
    assert.equal(d1.batches.length, 0);
    assert.deepEqual(d1.binds[1], [USER_ID, DEVICE_ID]);
  });

  it("returns an exact historical envelope for the authenticated device", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        approvedDeviceRow(),
        currentEnvelopeRow({ key_id: HISTORICAL_KEY_ID, generation: HISTORICAL_GENERATION }),
      ],
    });

    const response = await handleRequest(
      vaultGetRequest(`?generation=${HISTORICAL_GENERATION}&key_id=${HISTORICAL_KEY_ID}`),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), vaultDocument({
      key_id: HISTORICAL_KEY_ID,
      generation: HISTORICAL_GENERATION,
    }));
    assert.ok(d1.queries[1]?.includes("FROM sync_vault_envelopes"));
    assert.ok(d1.queries[1]?.includes("recipient_device_id = ?"));
    assert.deepEqual(d1.binds[1], [
      USER_ID,
      DEVICE_ID,
      HISTORICAL_KEY_ID,
      HISTORICAL_GENERATION,
    ]);
  });

  it("isolates historical envelopes by recipient and exact generation", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    for (const generation of [HISTORICAL_GENERATION, HISTORICAL_GENERATION + 1]) {
      const d1 = testD1Database({ firstRows: [approvedDeviceRow(), null] });
      const response = await handleRequest(
        vaultGetRequest(`?key_id=${HISTORICAL_KEY_ID}&generation=${generation}`),
        testEnv({
          d1,
          kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
        }),
      );

      assert.equal(response.status, 404);
      assert.deepEqual(d1.binds[1], [USER_ID, DEVICE_ID, HISTORICAL_KEY_ID, generation]);
    }
  });

  it("rejects partial, duplicate, and extra historical queries", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const queries = [
      `?generation=${HISTORICAL_GENERATION}`,
      `?key_id=${HISTORICAL_KEY_ID}`,
      `?key_id=${HISTORICAL_KEY_ID}&generation=3&generation=3`,
      `?key_id=${HISTORICAL_KEY_ID}&generation=3&extra=1`,
    ];
    for (const query of queries) {
      const d1 = testD1Database({ firstRows: [approvedDeviceRow()] });
      const response = await handleRequest(
        vaultGetRequest(query),
        testEnv({
          d1,
          kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
        }),
      );

      assert.equal(response.status, 400);
      assert.equal(d1.queries.length, 1);
    }
  });

  it("rejects malformed opaque envelopes before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      vaultBootstrapRequest(
        await vaultBootstrapBody({ envelope: { ...wrappedEnvelope(), ciphertext: "B".repeat(63) } }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_vault" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects noncanonical encapped keys before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      vaultBootstrapRequest(
        await vaultBootstrapBody({ envelope: { ...wrappedEnvelope(), encapped_key: `${"A".repeat(42)}B` } }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_vault" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects unknown envelope fields before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      vaultBootstrapRequest(
        await vaultBootstrapBody({ envelope: { ...wrappedEnvelope(), plaintext_key: KEY_ID } }),
      ),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects noninitial bootstrap generations before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }] });

    const response = await handleRequest(
      vaultBootstrapRequest(await vaultBootstrapBody({ generation: 2 })),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_vault" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects a bootstrap replay with different stored ciphertext", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({
      firstRows: [
        approvedDeviceRow(),
        signingKeyRow(),
        currentEnvelopeRow({ ciphertext: "C".repeat(64) }),
      ],
    });

    const response = await handleRequest(
      vaultBootstrapRequest(await vaultBootstrapBody()),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), { error: "sync_vault_conflict" });
  });

  it("rejects tampered bootstrap fields before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const body = await vaultBootstrapBody();
    body.key_id = "b".repeat(64);
    const d1 = testD1Database({ firstRows: [approvedDeviceRow(), signingKeyRow()] });

    const response = await handleRequest(
      vaultBootstrapRequest(body),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 403);
    assert.deepEqual(await response.json(), { error: "sync_vault_forbidden" });
    assert.equal(d1.queries.length, 2);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects bootstrap v1 before signing-key reads", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [approvedDeviceRow()] });

    const response = await handleRequest(
      vaultBootstrapRequest(await vaultBootstrapBody({ version: 1 })),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: "invalid_sync_vault" });
    assert.equal(d1.queries.length, 1);
    assert.deepEqual(d1.batches, []);
  });

  it("rejects missing approved v2 signing keys before vault writes", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [approvedDeviceRow(), null] });

    const response = await handleRequest(
      vaultBootstrapRequest(await vaultBootstrapBody()),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 403);
    assert.equal(d1.queries.length, 2);
    assert.deepEqual(d1.batches, []);
  });

  it("uses the frozen v2 bootstrap proof wire", () => {
    assert.equal(
      new TextDecoder().decode(syncVaultBootstrapProofBytes(
        USER_ID,
        DEVICE_ID,
        bootstrapProofInput(),
      )),
      "31:elydora-sync-vault-bootstrap-v2" +
        "7:user-01" +
        "9:device-01" +
        `64:${KEY_ID}` +
        "1:1" +
        "1:1" +
        `45:${SUITE}` +
        `43:${ENCAPPED_KEY}` +
        `64:${CIPHERTEXT}` +
        "25:sync-vault-bootstrap-0001",
    );
  });

  it("returns not found when the current device has no envelope", async () => {
    const tokenHash = await authTokenHash(ACCESS_TOKEN);
    const d1 = testD1Database({ firstRows: [{ device_id: DEVICE_ID }, null] });

    const response = await handleRequest(
      new Request("https://elydora.test/api/sync/vault", {
        headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
      }),
      testEnv({
        d1,
        kvEntries: [[authSessionCacheKvKey("local", tokenHash), sessionDocument(DEVICE_ID)]],
      }),
    );

    assert.equal(response.status, 404);
    assert.deepEqual(await response.json(), { error: "sync_vault_not_found" });
  });

  it("validates snapshot key metadata against the current vault key", async () => {
    const matching = testD1Database({ firstRows: [{ key_id: KEY_ID, generation: GENERATION }] });
    await assertCurrentSyncVaultKey(testEnv({ d1: matching }), USER_ID, KEY_ID, GENERATION);

    const mismatched = testD1Database({ firstRows: [{ key_id: KEY_ID, generation: GENERATION }] });
    await assert.rejects(
      assertCurrentSyncVaultKey(testEnv({ d1: mismatched }), USER_ID, "b".repeat(64), GENERATION),
      SyncVaultConflictError,
    );

    const missing = testD1Database({ firstRows: [null] });
    await assert.rejects(
      assertCurrentSyncVaultKey(testEnv({ d1: missing }), USER_ID, KEY_ID, GENERATION),
      SyncVaultNotFoundError,
    );
  });

  it("builds a recipient envelope write guarded by device trust and the current vault key", () => {
    const d1 = testD1Database([]);
    syncVaultRecipientEnvelopeStatement(
      testEnv({ d1 }),
      USER_ID,
      "device-02",
      DEVICE_ID,
      KEY_ID,
      GENERATION,
      parseWrappedAccountKey(wrappedEnvelope()),
      "sync-vault-recipient-0001",
      1_780_000_400,
    );

    assert.ok(d1.queries[0]?.includes("accounts.current_key_id = ?"));
    assert.ok(d1.queries[0]?.includes("recipient.approval_status = ?"));
    assert.ok(d1.queries[0]?.includes("approver.approval_status = 'approved'"));
    assert.deepEqual(d1.binds[0], [
      USER_ID,
      "device-02",
      DEVICE_ID,
      KEY_ID,
      GENERATION,
      1,
      SUITE,
      ENCAPPED_KEY,
      CIPHERTEXT,
      "sync-vault-recipient-0001",
      1_780_000_400,
      "device-02",
      "pending",
      DEVICE_ID,
      USER_ID,
      KEY_ID,
      GENERATION,
    ]);
  });
});

function vaultBootstrapRequest(body: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/vault/bootstrap", {
    method: "POST",
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

function vaultGetRequest(query = ""): Request {
  return new Request(`https://elydora.test/api/sync/vault${query}`, {
    headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
  });
}

async function vaultBootstrapBody(
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  return {
    version: 2,
    key_id: KEY_ID,
    generation: GENERATION,
    envelope: wrappedEnvelope(),
    idempotency_key: IDEMPOTENCY_KEY,
    bootstrap_proof: await signDeviceMessage(
      syncVaultBootstrapProofBytes(USER_ID, DEVICE_ID, bootstrapProofInput()),
    ),
    ...overrides,
  };
}

function bootstrapProofInput() {
  return {
    keyId: KEY_ID,
    generation: GENERATION,
    envelope: parseWrappedAccountKey(wrappedEnvelope()),
    idempotencyKey: IDEMPOTENCY_KEY,
  };
}

function approvedDeviceRow(): Record<string, unknown> {
  return { device_id: DEVICE_ID };
}

function signingKeyRow(): Record<string, unknown> {
  return { signing_public_key: PUBLIC_KEY };
}

function wrappedEnvelope(): Record<string, unknown> {
  return {
    version: 1,
    suite: SUITE,
    encapped_key: ENCAPPED_KEY,
    ciphertext: CIPHERTEXT,
  };
}

function currentEnvelopeRow(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    key_id: KEY_ID,
    generation: GENERATION,
    recipient_device_id: DEVICE_ID,
    approver_device_id: DEVICE_ID,
    envelope_version: 1,
    suite: SUITE,
    encapped_key: ENCAPPED_KEY,
    ciphertext: CIPHERTEXT,
    idempotency_key: IDEMPOTENCY_KEY,
    created_at: 1_780_000_300,
    ...overrides,
  };
}

function vaultDocument(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: 1,
    user_id: USER_ID,
    key_id: KEY_ID,
    generation: GENERATION,
    recipient_device_id: DEVICE_ID,
    approver_device_id: DEVICE_ID,
    envelope: wrappedEnvelope(),
    created_at: 1_780_000_300,
    ...overrides,
  };
}
