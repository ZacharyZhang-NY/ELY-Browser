import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { handleRequest } from "../src/index.js";
import {
  type SensitiveAction,
  recentDeviceActionProofBytes,
  recentDeviceActionRequestHash,
} from "../src/recent_device_action_proof.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  signDeviceMessage,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";

const NOW = 1_780_001_000;

interface ActionCase {
  action: SensitiveAction;
  path: string;
  confirmation: string;
  idempotencyKey: string;
  forbiddenError: string;
  failedError: string;
  existingEvent: Record<string, unknown>;
}

const ACTIONS: ActionCase[] = [
  {
    action: "sync.reset",
    path: "/api/sync/reset",
    confirmation: "delete-cloud-sync-data",
    idempotencyKey: "sync-reset-security-0001",
    forbiddenError: "sync_reset_forbidden",
    failedError: "sync_reset_failed",
    existingEvent: {
      actor_device_id: "device-01",
      outcome: "success",
      created_at: NOW,
    },
  },
  {
    action: "account.delete",
    path: "/api/account/delete",
    confirmation: "delete-elydora-account",
    idempotencyKey: "account-delete-security-0001",
    forbiddenError: "account_deletion_forbidden",
    failedError: "account_deletion_failed",
    existingEvent: {
      actor_device_id: "device-01",
      outcome: "success",
      subject_id: "2fb6b7445391dae3bf4fb63927132e773d8d00e5963b5270dddecc84e99811fa",
      created_at: NOW,
    },
  },
];

describe("destructive action proofs", () => {
  it("blocks a stolen bearer without the device private key", async () => {
    for (const action of ACTIONS) {
      const body = await actionBody(action, NOW);
      body.action_proof = "0".repeat(128);
      const response = await actionRequest(action, body, newActionRows(action));

      assert.equal(response.status, 403);
      assert.deepEqual(await response.json(), { error: action.forbiddenError });
    }
  });

  it("binds the action, session, and idempotency key", async () => {
    for (const action of ACTIONS) {
      for (const changed of ["action", "session", "idempotency"] as const) {
        const signedAction = changed === "action"
          ? action.action === "sync.reset" ? "account.delete" : "sync.reset"
          : action.action;
        const body = await actionBody(
          action,
          NOW,
          signedAction,
          changed === "session" ? "session-02" : "session-01",
        );
        if (changed === "idempotency") {
          body.idempotency_key = `${action.idempotencyKey}-changed`;
        }
        const response = await actionRequest(action, body, newActionRows(action));

        assert.equal(response.status, 403);
        assert.deepEqual(await response.json(), { error: action.forbiddenError });
      }
    }
  });

  it("requires freshness for a new destructive action", async () => {
    for (const action of ACTIONS) {
      const response = await actionRequest(
        action,
        await actionBody(action, NOW - 301),
        newActionRows(action, true),
      );

      assert.equal(response.status, 403);
      assert.deepEqual(await response.json(), { error: action.forbiddenError });
    }
  });

  it("verifies old proofs for exact idempotent replays without requiring freshness", async () => {
    for (const action of ACTIONS) {
      const body = await actionBody(action, NOW - 301);
      const event = {
        ...action.existingEvent,
        ...(action.action === "account.delete"
          ? { metadata_hash: await requestHash(action, body) }
          : {}),
      };
      const response = await actionRequest(action, body, replayRows(action, event));

      assert.equal(response.status, 200);
    }
  });

  it("maps a malformed stored signing key to a persistence failure", async () => {
    for (const action of ACTIONS) {
      const rows = newActionRows(action);
      rows[rows.length - 1] = { signing_public_key: "invalid" };
      const response = await actionRequest(action, await actionBody(action, NOW), rows);

      assert.equal(response.status, 500);
      assert.deepEqual(await response.json(), { error: action.failedError });
    }
  });

  it("opens a fresh primary session after a concurrent replay abort", async () => {
    const action = ACTIONS[0];
    assert.ok(action !== undefined);
    const proofCreatedAt = Math.floor(Date.now() / 1000);
    const d1 = testD1Database({
      firstRows: [
        { device_id: "device-01" },
        { signing_public_key: PUBLIC_KEY },
        null,
        { objects: 0, changes: 0, snapshots: 0, tombstones: 0 },
        action.existingEvent,
      ],
      allRows: [],
      batchError: new Error("UNIQUE constraint failed: audit_events.event_id"),
    });
    const response = await handleRequest(
      new Request(`https://elydora.test${action.path}`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(await actionBody(action, proofCreatedAt)),
      }),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(d1.sessionConstraints, ["first-primary", "first-primary", "first-primary"]);
  });

  it("requires the exact timestamp and session for an account deletion replay", async () => {
    const action = ACTIONS[1];
    assert.ok(action !== undefined);
    const body = await actionBody(action, NOW - 301);
    const event = {
      ...action.existingEvent,
      metadata_hash: await requestHash(action, body),
    };

    const changedTimestamp = { ...body, proof_created_at: NOW - 300 };
    const timestampResponse = await actionRequest(action, changedTimestamp, [event]);
    assert.equal(timestampResponse.status, 400);

    const d1 = testD1Database({
      firstRows: [event],
      sessionRow: {
        id: "session-02",
        userId: "user-01",
        expiresAt: "2099-01-01T00:00:00.000Z",
        createdAt: "2026-01-01T00:00:00.000Z",
        deviceId: "device-01",
      },
    });
    const sessionResponse = await handleRequest(
      new Request(`https://elydora.test${action.path}`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${ACCESS_TOKEN}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      }),
      testEnv({ d1 }),
    );
    assert.equal(sessionResponse.status, 400);
  });
});

async function actionRequest(
  action: ActionCase,
  body: Record<string, unknown>,
  firstRows: unknown[],
): Promise<Response> {
  return handleRequest(
    new Request(`https://elydora.test${action.path}`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${ACCESS_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    }),
    testEnv({ d1: testD1Database({ firstRows }) }),
  );
}

function newActionRows(action: ActionCase, includeEventMiss = false): unknown[] {
  if (action.action === "account.delete") {
    return [null, { signing_public_key: PUBLIC_KEY }];
  }
  return [
    { device_id: "device-01" },
    { signing_public_key: PUBLIC_KEY },
    ...(includeEventMiss ? [null] : []),
  ];
}

function replayRows(action: ActionCase, event: Record<string, unknown>): unknown[] {
  return action.action === "account.delete"
    ? [event]
    : [{ device_id: "device-01" }, { signing_public_key: PUBLIC_KEY }, event];
}

async function actionBody(
  action: ActionCase,
  proofCreatedAt: number,
  signedAction = action.action,
  signedSessionId = "session-01",
): Promise<Record<string, unknown>> {
  const body = {
    version: 2,
    confirmation: action.confirmation,
    idempotency_key: action.idempotencyKey,
    proof_created_at: proofCreatedAt,
    action_proof: "",
  };
  body.action_proof = await signDeviceMessage(recentDeviceActionProofBytes({
    action: signedAction,
    userId: "user-01",
    sessionId: signedSessionId,
    deviceId: "device-01",
    confirmation: body.confirmation,
    idempotencyKey: body.idempotency_key,
    proofCreatedAt,
  }));
  return body;
}

function requestHash(
  action: ActionCase,
  body: Record<string, unknown>,
): Promise<string> {
  return recentDeviceActionRequestHash({
    action: action.action,
    userId: "user-01",
    sessionId: "session-01",
    deviceId: "device-01",
    confirmation: String(body.confirmation),
    idempotencyKey: String(body.idempotency_key),
    proofCreatedAt: Number(body.proof_created_at),
    actionProof: String(body.action_proof),
  });
}
