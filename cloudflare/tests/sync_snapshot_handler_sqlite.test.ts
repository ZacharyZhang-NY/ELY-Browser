import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import type { AuthContext } from "../src/auth.js";
import { syncSnapshotUploadDocument } from "../src/sync_snapshot.js";
import { type RecordedR2Put, testEnv } from "./devices_test_support.js";
import { SqliteD1Database, execute } from "./sqlite_d1_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const KEY_ID = "1".repeat(64);
const CONTENT_HASH = "2".repeat(64);
const MIGRATIONS_DIR = join(process.cwd(), "migrations");

describe("sync snapshot handler real D1 flow", () => {
  it("commits genesis, same-id child, and exact duplicate through the five-statement batch", async () => {
    await withDatabase(async (databasePath) => {
      const d1 = new SqliteD1Database(databasePath);
      const r2Puts: RecordedR2Put[] = [];
      const env = testEnv({ d1, r2Puts });
      const genesisPayload = bytes("genesis ciphertext");
      const genesisHash = sha256(genesisPayload);
      const genesis = await syncSnapshotUploadDocument(
        request(body(genesisPayload, genesisHash, 1, null, 10)),
        env,
        authContext(),
        10,
      );
      const base = {
        revision: genesis.snapshot.head_revision,
        snapshot_id: genesis.snapshot.snapshot_id,
        payload_hash: genesis.snapshot.payload_hash,
      };
      const childPayload = bytes("child ciphertext");
      const childHash = sha256(childPayload);
      const childRequest = request(body(childPayload, childHash, 2, base, 11));
      const child = await syncSnapshotUploadDocument(childRequest, env, authContext(), 11);
      const duplicate = await syncSnapshotUploadDocument(
        request(body(childPayload, childHash, 2, base, 11)),
        env,
        authContext(),
        12,
      );

      assert.equal(genesis.snapshot.head_revision, 1);
      assert.equal(child.snapshot.head_revision, 2);
      assert.deepEqual(child.snapshot.base_head, base);
      assert.deepEqual(duplicate, child);
      assert.deepEqual(d1.batches, [5, 5]);
      assert.deepEqual(d1.sessionConstraints, [
        "first-primary",
        "first-primary",
        "first-primary",
        "first-primary",
        "first-primary",
        "first-primary",
      ]);
      assert.equal(r2Puts.length, 2);
      assert.deepEqual(d1.rows(`
        SELECT state, COUNT(*) AS count
        FROM sync_r2_gc_candidates
        WHERE user_id = '${USER_ID}' AND object_kind = 'snapshot'
        GROUP BY state
        ORDER BY state ASC
      `), [
        { state: "ready", count: 1 },
        { state: "referenced", count: 1 },
      ]);
      assert.deepEqual(d1.rows(`
        SELECT head.head_revision, head.snapshot_id, head.payload_hash,
               snapshot.logical_clock, encryption.content_hash
        FROM sync_snapshot_heads AS head
        INNER JOIN sync_snapshots AS snapshot
          ON snapshot.user_id = head.user_id AND snapshot.snapshot_id = head.snapshot_id
        INNER JOIN sync_snapshot_encryption AS encryption
          ON encryption.user_id = snapshot.user_id
          AND encryption.snapshot_id = snapshot.snapshot_id
        WHERE head.user_id = '${USER_ID}'
      `), [{
        head_revision: 2,
        snapshot_id: DEVICE_ID,
        payload_hash: childHash,
        logical_clock: 11,
        content_hash: CONTENT_HASH,
      }]);
    });
  });
});

async function withDatabase(assertions: (databasePath: string) => Promise<void>): Promise<void> {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-snapshot-handler-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    for (const fileName of readdirSync(MIGRATIONS_DIR).filter((name) => name.endsWith(".sql")).sort()) {
      execute(databasePath, readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"));
    }
    execute(databasePath, `
      INSERT INTO better_auth_user (
        id, name, email, emailVerified, createdAt, updatedAt
      ) VALUES (
        '${USER_ID}', 'User', 'user@example.com', 1,
        '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
      );
      INSERT INTO user_devices (
        user_id, device_id, public_key, device_name, platform,
        approval_status, created_at, approved_at, last_active_at, revoked_at, idempotency_key
      ) VALUES (
        '${USER_ID}', '${DEVICE_ID}', '${"d".repeat(64)}', 'Mac', 'macOS',
        'approved', 1, 1, 1, NULL, 'device-register-0001'
      );
      INSERT INTO user_device_keys (
        user_id, device_id, signing_public_key, wrapping_public_key,
        key_protocol_version, created_at
      ) VALUES (
        '${USER_ID}', '${DEVICE_ID}', '${"d".repeat(64)}', '${"e".repeat(64)}', 2, 1
      );
      INSERT INTO sync_vault_accounts (
        user_id, current_key_id, current_generation, created_at, updated_at
      ) VALUES ('${USER_ID}', '${KEY_ID}', 1, 1, 1);
    `);
    await assertions(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function body(
  payload: ArrayBuffer,
  payloadHash: string,
  headRevision: number,
  baseHead: Record<string, unknown> | null,
  logicalClock: number,
): Record<string, unknown> {
  return {
    version: 3,
    snapshot_id: DEVICE_ID,
    region: "us-east",
    payload_hash: payloadHash,
    encryption_version: 2,
    vault_generation: 1,
    key_id: KEY_ID,
    content_hash: CONTENT_HASH,
    schema_rev: 1,
    logical_clock: logicalClock,
    head_revision: headRevision,
    base_head: baseHead,
    data_base64: Buffer.from(payload).toString("base64"),
  };
}

function request(value: Record<string, unknown>): Request {
  return new Request("https://elydora.test/api/sync/snapshot", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(value),
  });
}

function authContext(): AuthContext {
  return {
    userId: USER_ID,
    sessionId: "session-01",
    tokenHash: "f".repeat(64),
    expiresAt: "2099-01-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    deviceId: DEVICE_ID,
  };
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

function sha256(payload: ArrayBuffer): string {
  return createHash("sha256").update(new Uint8Array(payload)).digest("hex");
}
