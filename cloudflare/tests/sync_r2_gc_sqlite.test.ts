import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import type { ElyR2Object, ElyR2PutOptions, Env } from "../src/bindings.js";
import { recentDeviceActionProofBytes } from "../src/recent_device_action_proof.js";
import {
  SYNC_R2_ANONYMIZE_USER_QUERY,
  SYNC_R2_FENCE_USER_QUERY,
  abandonSyncR2Write,
  claimSyncR2SnapshotWrite,
  collectSyncR2Garbage,
} from "../src/sync_r2_gc.js";
import { inventorySyncR2Objects } from "../src/sync_r2_inventory.js";
import { syncResetDocument } from "../src/sync_reset.js";
import { PUBLIC_KEY, signDeviceMessage } from "./devices_test_support.js";
import { SqliteD1Database, execute, query } from "./sqlite_d1_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const KEY_ID = "1".repeat(64);
const OWNER_HASH = createHash("sha256").update(USER_ID).digest("hex");
const HASH_A = "a".repeat(64);
const HASH_B = "b".repeat(64);
const TOKEN_A = "c".repeat(64);
const TOKEN_B = "d".repeat(64);
const NOW = 1_800_000_000;
const MIGRATIONS_DIR = join(process.cwd(), "migrations");

describe("sync R2 GC SQLite state machine", () => {
  it("commits a leased candidate and deletes it only after D1 references are fenced", async () => {
    await withDatabase(async (databasePath, database, bucket, env) => {
      const key = snapshotKey(HASH_A);
      const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await bucket.put(key, bytes("ciphertext-a"));
      commitGenesis(databasePath, key, HASH_A, lease.writeToken);

      assert.equal(candidateState(databasePath, key), "referenced");
      assert.equal(await collectSyncR2Garbage(env, NOW + 100_000), 0);

      await database.batch([
        database.prepare(SYNC_R2_FENCE_USER_QUERY).bind(NOW + 1, NOW + 1, NOW + 1, USER_ID),
        database.prepare("DELETE FROM sync_snapshot_heads WHERE user_id = ?").bind(USER_ID),
        database.prepare("DELETE FROM sync_snapshot_encryption WHERE user_id = ?").bind(USER_ID),
        database.prepare("DELETE FROM sync_snapshots WHERE user_id = ?").bind(USER_ID),
      ]);
      assert.equal(candidateState(databasePath, key), "ready");
      assert.equal(await collectSyncR2Garbage(env, NOW + 1), 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.deepEqual(bucket.deletes, [key]);
    });
  });

  it("turns a CAS loser into an immediately collectible ready candidate", async () => {
    await withDatabase(async (databasePath, _database, bucket, env) => {
      const key = snapshotKey(HASH_A);
      const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await bucket.put(key, bytes("ciphertext-a"));

      await abandonSyncR2Write(env, USER_ID, OWNER_HASH, key, lease.writeToken, NOW + 1);

      assert.equal(candidateState(databasePath, key), "ready");
      assert.equal(await collectSyncR2Garbage(env, NOW + 1), 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.deepEqual(bucket.deletes, [key]);
    });
  });

  it("resets sync state while preserving the vault generation and device envelopes", async () => {
    await withDatabase(async (databasePath, _database, bucket, env) => {
      seedVaultEnvelope(databasePath);
      const key = snapshotKey(HASH_A);
      const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await bucket.put(key, bytes("ciphertext-a"));
      commitGenesis(databasePath, key, HASH_A, lease.writeToken);

      const document = await syncResetDocument(
        await resetRequest("sync-reset-000001", NOW + 1),
        env,
        authContext(),
        NOW + 1,
      );

      assert.equal(document.deleted.snapshots, 1);
      assert.equal(document.deleted.r2_objects, 1);
      assert.deepEqual(query(databasePath, `
        SELECT current_key_id, current_generation FROM sync_vault_accounts
        WHERE user_id = '${USER_ID}'
      `), [{ current_key_id: KEY_ID, current_generation: 1 }]);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM sync_vault_envelopes WHERE user_id = '${USER_ID}'
      `)[0]?.count, 1);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM user_devices WHERE user_id = '${USER_ID}'
      `)[0]?.count, 1);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM sync_snapshots WHERE user_id = '${USER_ID}'
      `)[0]?.count, 0);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.deepEqual(bucket.deletes, [key]);
    });
  });

  it("keeps a fenced pending lease until a late R2 put can be collected", async () => {
    await withDatabase(async (databasePath, _database, bucket, env) => {
      const key = snapshotKey(HASH_A);
      await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await syncResetDocument(
        await resetRequest("sync-reset-000002", NOW + 1),
        env,
        authContext(),
        NOW + 1,
      );

      assert.equal(candidateState(databasePath, key), "ready");
      assert.equal(await collectSyncR2Garbage(env, NOW + 1, { userId: USER_ID }), 0);
      await bucket.put(key, bytes("late-pending-ciphertext"));
      assert.equal(await collectSyncR2Garbage(env, NOW + 600, { userId: USER_ID }), 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.equal(bucket.has(key), false);
      assert.deepEqual(bucket.deletes, [key]);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM sync_vault_accounts WHERE user_id = '${USER_ID}'
      `)[0]?.count, 1);
    });
  });

  it("rolls back reset when its authenticated authority changes before the batch", async () => {
    for (const beforeBatchSql of [
      "DELETE FROM better_auth_session WHERE id = 'session-01';",
      `UPDATE user_devices SET revoked_at = ${NOW} WHERE device_id = '${DEVICE_ID}';`,
    ]) {
      await withDatabase(async (databasePath, _database, bucket, env) => {
        const key = snapshotKey(HASH_A);
        const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
        await bucket.put(key, bytes("ciphertext-a"));
        commitGenesis(databasePath, key, HASH_A, lease.writeToken);
        const request = await resetRequest("sync-reset-authority-race", NOW + 1);
        const racedEnv = {
          ...env,
          ELY_DB: new SqliteD1Database(databasePath, beforeBatchSql),
        } as Env;

        await assert.rejects(
          () => syncResetDocument(request, racedEnv, authContext(), NOW + 1),
          /device_action_gate_failed/,
        );

        assert.deepEqual(query(databasePath, `SELECT
          (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = '${USER_ID}') AS snapshots,
          (SELECT COUNT(*) FROM sync_snapshot_encryption WHERE user_id = '${USER_ID}') AS encryption,
          (SELECT COUNT(*) FROM sync_snapshot_heads WHERE user_id = '${USER_ID}') AS heads,
          (SELECT COUNT(*) FROM audit_events WHERE event_type = 'sync.reset') AS audits
        `), [{ snapshots: 1, encryption: 1, heads: 1, audits: 0 }]);
        assert.equal(candidateState(databasePath, key), "referenced");
        assert.equal(bucket.has(key), true);
      });
    }
  });

  it("retries an idempotent R2 deletion after a crash before D1 finalization", async () => {
    await withDatabase(async (databasePath, _database, bucket, env) => {
      const key = snapshotKey(HASH_A);
      const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await bucket.put(key, bytes("ciphertext-a"));
      await abandonSyncR2Write(env, USER_ID, OWNER_HASH, key, lease.writeToken, NOW + 1);
      bucket.crashAfterNextDelete = true;

      await assert.rejects(() => collectSyncR2Garbage(env, NOW + 1), /simulated_delete_crash/);
      assert.equal(candidateState(databasePath, key), "deleting");
      assert.equal(bucket.has(key), false);

      assert.equal(await collectSyncR2Garbage(env, NOW + 62), 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.deepEqual(bucket.deletes, [key, key]);
    });
  });

  it("fences an upload that overlaps account deletion and clears the raw owner id", async () => {
    await withDatabase(async (databasePath, database, bucket, env) => {
      const key = snapshotKey(HASH_A);
      const lease = await claimSyncR2SnapshotWrite(env, snapshotClaim(key), NOW, TOKEN_A);
      await database.batch([
        database.prepare(SYNC_R2_FENCE_USER_QUERY).bind(NOW + 1, NOW + 1, NOW + 1, USER_ID),
        database.prepare(SYNC_R2_ANONYMIZE_USER_QUERY).bind(NOW + 1, USER_ID, OWNER_HASH),
      ]);
      assert.equal(await collectSyncR2Garbage(env, NOW + 1, { ownerHash: OWNER_HASH }), 0);
      await bucket.put(key, bytes("late-ciphertext"));

      assert.throws(
        () => commitGenesis(databasePath, key, HASH_A, lease.writeToken),
        /sync_r2_write_fenced/,
      );
      assert.deepEqual(candidateOwner(databasePath, key), {
        user_id: null,
        owner_hash: OWNER_HASH,
        state: "ready",
      });
      assert.equal(await collectSyncR2Garbage(
        env,
        lease.leaseExpiresAt,
        { ownerHash: OWNER_HASH },
      ), 1);
    });
  });

  it("rejects a pre-put claim after reset removes the vault authority", async () => {
    await withDatabase(async (_databasePath, database, _bucket, env) => {
      await database.prepare("DELETE FROM sync_vault_accounts WHERE user_id = ?")
        .bind(USER_ID)
        .run();

      await assert.rejects(
        () => claimSyncR2SnapshotWrite(
          env,
          snapshotClaim(snapshotKey(HASH_A)),
          NOW,
          TOKEN_A,
        ),
        /sync_r2_write_fenced/,
      );
    });
  });

  it("inventories historical snapshot and sync-payload orphans", async () => {
    await withDatabase(async (databasePath, _database, bucket, env) => {
      const snapshot = snapshotKey(HASH_A);
      const payload = payloadKey(HASH_B);
      await bucket.put(snapshot, bytes("snapshot-orphan"));
      await bucket.put(payload, bytes("payload-orphan"));

      assert.equal(await inventorySyncR2Objects(env, NOW, 100), 1);
      assert.equal(await inventorySyncR2Objects(env, NOW + 1, 100), 1);
      assert.equal(candidateState(databasePath, snapshot), "ready");
      assert.equal(candidateState(databasePath, payload), "ready");
      assert.equal(await collectSyncR2Garbage(env, NOW + 1), 2);
      assert.deepEqual(bucket.deletes.sort(), [payload, snapshot].sort());
    });
  });
});

async function withDatabase(
  assertions: (
    databasePath: string,
    database: SqliteD1Database,
    bucket: TestBucket,
    env: Env,
  ) => Promise<void>,
): Promise<void> {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-r2-gc-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    for (const fileName of readdirSync(MIGRATIONS_DIR).filter((name) => name.endsWith(".sql")).sort()) {
      execute(databasePath, readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"));
    }
    seedAuthority(databasePath);
    const database = new SqliteD1Database(databasePath);
    const bucket = new TestBucket();
    const env = { ELY_DB: database, ELY_STORAGE: bucket } as unknown as Env;
    await assertions(databasePath, database, bucket, env);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function seedAuthority(databasePath: string): void {
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
      '${USER_ID}', '${DEVICE_ID}', '${PUBLIC_KEY}', 'Mac', 'macOS',
      'approved', 1, 1, 1, NULL, 'device-register-0001'
    );
    INSERT INTO user_device_keys (
      user_id, device_id, signing_public_key, wrapping_public_key,
      key_protocol_version, created_at
    ) VALUES (
      '${USER_ID}', '${DEVICE_ID}', '${PUBLIC_KEY}', '${"f".repeat(64)}', 2, 1
    );
    INSERT INTO better_auth_session (
      id, expiresAt, token, createdAt, updatedAt, userId
    ) VALUES (
      'session-01', '2099-01-01T00:00:00Z', 'session-token-01',
      '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '${USER_ID}'
    );
    INSERT INTO better_auth_session_device_context (
      session_id, user_id, device_id, updated_at
    ) VALUES ('session-01', '${USER_ID}', '${DEVICE_ID}', 1);
    INSERT INTO sync_vault_accounts (
      user_id, current_key_id, current_generation, created_at, updated_at
    ) VALUES ('${USER_ID}', '${KEY_ID}', 1, 1, 1);
  `);
}

function seedVaultEnvelope(databasePath: string): void {
  execute(databasePath, `
    INSERT INTO sync_vault_envelopes (
      user_id, recipient_device_id, approver_device_id, key_id, generation,
      envelope_version, suite, encapped_key, ciphertext, idempotency_key, created_at
    ) VALUES (
      '${USER_ID}', '${DEVICE_ID}', '${DEVICE_ID}', '${KEY_ID}', 1, 1,
      'HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305',
      '${"A".repeat(43)}', '${"B".repeat(64)}', 'vault-bootstrap-0001', 1
    );
  `);
}

function commitGenesis(databasePath: string, r2Key: string, payloadHash: string, token: string): void {
  execute(databasePath, `
    BEGIN IMMEDIATE;
    INSERT INTO sync_snapshots (
      user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
      device_id, size_bytes, created_at, head_revision,
      base_head_revision, base_snapshot_id, base_payload_hash
    ) VALUES (
      '${USER_ID}', '${DEVICE_ID}', '${r2Key}', '${payloadHash}', 1, 1,
      '${DEVICE_ID}', 12, ${NOW}, 1, NULL, NULL, NULL
    );
    INSERT INTO sync_snapshot_encryption (
      user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash
    ) VALUES ('${USER_ID}', '${DEVICE_ID}', 2, 1, '${KEY_ID}', '${HASH_B}');
    INSERT INTO sync_snapshot_heads (
      user_id, head_revision, snapshot_id, payload_hash, updated_at
    ) VALUES ('${USER_ID}', 1, '${DEVICE_ID}', '${payloadHash}', ${NOW});
    UPDATE sync_r2_gc_candidates
    SET state = 'referenced', lease_expires_at = ${NOW},
      updated_at = ${NOW}, referenced_at = ${NOW}
    WHERE r2_key = '${r2Key}' AND user_id = '${USER_ID}'
      AND state = 'pending' AND write_token = '${token}'
      AND lease_expires_at >= ${NOW};
    COMMIT;
  `);
}

function snapshotClaim(r2Key: string) {
  return {
    userId: USER_ID,
    deviceId: DEVICE_ID,
    r2Key,
    ownerHash: OWNER_HASH,
    keyId: KEY_ID,
    generation: 1,
    headRevision: 1,
    baseHead: null,
  } as const;
}

function authContext() {
  return {
    userId: USER_ID,
    deviceId: DEVICE_ID,
    sessionId: "session-01",
    tokenHash: HASH_B,
    expiresAt: "2099-01-01T00:00:00Z",
    createdAt: "2026-01-01T00:00:00Z",
  } as const;
}

async function resetRequest(idempotencyKey: string, proofCreatedAt: number): Promise<Request> {
  const confirmation = "delete-cloud-sync-data";
  const actionProof = await signDeviceMessage(recentDeviceActionProofBytes({
    action: "sync.reset",
    userId: USER_ID,
    sessionId: authContext().sessionId,
    deviceId: DEVICE_ID,
    confirmation,
    idempotencyKey,
    proofCreatedAt,
  }));
  return new Request("https://elydora.test/api/sync/reset", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      version: 2,
      confirmation,
      idempotency_key: idempotencyKey,
      proof_created_at: proofCreatedAt,
      action_proof: actionProof,
    }),
  });
}

function candidateState(databasePath: string, key: string): unknown {
  return query(databasePath, `
    SELECT state FROM sync_r2_gc_candidates WHERE r2_key = '${key}'
  `)[0]?.state;
}

function candidateOwner(databasePath: string, key: string): Record<string, unknown> | undefined {
  return query(databasePath, `
    SELECT user_id, owner_hash, state FROM sync_r2_gc_candidates WHERE r2_key = '${key}'
  `)[0];
}

function snapshotKey(hash: string): string {
  return `sync-snapshots/us-east/${OWNER_HASH}/${DEVICE_ID}/${hash}.bin`;
}

function payloadKey(hash: string): string {
  return `sync-payloads/us-east/${OWNER_HASH}/bookmarks/object-01/${hash}.bin`;
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

class TestBucket {
  readonly deletes: string[] = [];
  crashAfterNextDelete = false;
  private readonly values = new Map<string, ArrayBuffer>();

  get(key: string): Promise<ElyR2Object | null> {
    const value = this.values.get(key);
    return Promise.resolve(value === undefined ? null : object(value));
  }

  put(key: string, value: ArrayBuffer, _options?: ElyR2PutOptions): Promise<ElyR2Object> {
    this.values.set(key, value);
    return Promise.resolve(object(value));
  }

  async delete(key: string): Promise<void> {
    this.deletes.push(key);
    this.values.delete(key);
    if (this.crashAfterNextDelete) {
      this.crashAfterNextDelete = false;
      throw new Error("simulated_delete_crash");
    }
  }

  list(options: { prefix: string; cursor?: string; limit: number }) {
    const keys = [...this.values.keys()].filter((key) => key.startsWith(options.prefix)).sort();
    return Promise.resolve({
      objects: keys.slice(0, options.limit).map((key) => ({ key })),
      truncated: false as const,
    });
  }

  has(key: string): boolean {
    return this.values.has(key);
  }
}

function object(value: ArrayBuffer): ElyR2Object {
  return { arrayBuffer: () => Promise.resolve(value) };
}
