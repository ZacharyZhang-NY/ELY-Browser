import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import type { ElyR2Object, ElyR2PutOptions, Env } from "../src/bindings.js";
import { accountDeletionDocument } from "../src/account_deletion.js";
import { authSessionCacheKvKey } from "../src/auth.js";
import { purgeLegacySessionCache } from "../src/legacy_auth_kv_cleanup.js";
import {
  recentDeviceActionProofBytes,
  type SensitiveAction,
} from "../src/recent_device_action_proof.js";
import { collectSyncR2Garbage } from "../src/sync_r2_gc.js";
import { maintainSyncR2Storage } from "../src/sync_r2_maintenance.js";
import { syncResetDocument } from "../src/sync_reset.js";
import { PUBLIC_KEY, signDeviceMessage } from "./devices_test_support.js";
import { SqliteD1Database, execute, query } from "./sqlite_d1_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const KEY_ID = "1".repeat(64);
const TOKEN_HASH = "2".repeat(64);
const OWNER_HASH = createHash("sha256").update(USER_ID).digest("hex");
const NOW = 1_800_000_000;
const MIGRATIONS_DIR = join(process.cwd(), "migrations");

describe("account deletion and reset GC drains", () => {
  it("drains 101 reset candidates through bounded batches", async () => {
    await withDatabase(async (databasePath, bucket, _kv, env) => {
      const keys = seedReadyCandidates(databasePath, bucket, 101);

      const document = await syncResetDocument(
        await resetRequest("sync-reset-101-items", NOW),
        env,
        authContext(),
        NOW,
      );
      const replay = await syncResetDocument(
        await resetRequest("sync-reset-101-items", NOW),
        env,
        authContext(),
        NOW + 1_000,
      );

      assert.equal(document.deleted.r2_objects, 101);
      assert.equal(replay.reset_at, NOW);
      assert.equal(replay.deleted.r2_objects, 0);
      assert.equal(deletedCandidateCount(databasePath), 101);
      assert.equal(bucket.size, 0);
      assert.deepEqual(bucket.deletes.sort(), keys.sort());
    });
  });

  it("releases rotation staging on reset and finalizes cleanup during maintenance", async () => {
    await withDatabase(async (databasePath, bucket, _kv, env) => {
      const [key] = seedReadyCandidates(databasePath, bucket, 1);
      assert.ok(key !== undefined);
      seedCompletedRotation(databasePath, key);

      const document = await syncResetDocument(
        await resetRequest("sync-reset-rotation", NOW),
        env,
        authContext(),
        NOW,
      );

      assert.equal(document.deleted.r2_objects, 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.deepEqual(query(databasePath, `
        SELECT cleanup_snapshot_id, storage_cleaned_at
        FROM sync_vault_rotations
        WHERE user_id = '${USER_ID}' AND idempotency_key = 'rotation-reset-0001'
      `), [{ cleanup_snapshot_id: "sync-reset", storage_cleaned_at: null }]);

      await maintainSyncR2Storage(env, NOW + 1);

      assert.deepEqual(query(databasePath, `
        SELECT storage_cleaned_at
        FROM sync_vault_rotations
        WHERE user_id = '${USER_ID}' AND idempotency_key = 'rotation-reset-0001'
      `), [{ storage_cleaned_at: NOW + 1 }]);
    });
  });

  it("drains 101 account candidates after anonymizing their owner", async () => {
    await withDatabase(async (databasePath, bucket, _kv, env) => {
      seedReadyCandidates(databasePath, bucket, 101);
      const context = authContext();
      const request = await accountDeleteRequest("account-delete-101-items", NOW);
      const replayRequest = request.clone();
      const document = await accountDeletionDocument(
        request,
        env,
        context,
        NOW,
      );
      const replay = await accountDeletionDocument(replayRequest, env, context, NOW + 1_000);

      assert.equal(document.deleted.r2_objects, 101);
      assert.equal(replay.account_hash, document.account_hash);
      assert.equal(replay.deleted_at, NOW);
      assert.equal(replay.deleted.users, 0);
      assert.equal(deletedCandidateCount(databasePath), 101);
      assert.equal(bucket.size, 0);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM sync_r2_gc_candidates WHERE user_id IS NOT NULL
      `)[0]?.count, 0);
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM better_auth_user WHERE id = '${USER_ID}'
      `)[0]?.count, 0);
      assert.deepEqual(query(databasePath, `
        SELECT user_id, outcome FROM audit_events WHERE event_type = 'account.delete'
      `), [{ user_id: null, outcome: "success" }]);
    });
  });

  it("returns account deletion success while scheduled cleanup retries R2 and KV failures", async () => {
    await withDatabase(async (databasePath, bucket, kv, env) => {
      const [key] = seedReadyCandidates(databasePath, bucket, 1);
      assert.ok(key !== undefined);
      const legacyKey = authSessionCacheKvKey("local", TOKEN_HASH);
      kv.values.set(legacyKey, "legacy-session");
      bucket.failDeletes = true;
      kv.failDeletes = true;

      const document = await accountDeletionDocument(
        await accountDeleteRequest("account-delete-cleanup-failure", NOW),
        env,
        authContext(),
        NOW,
      );

      assert.equal(document.deleted.kv_session_cache, 0);
      assert.equal(candidateState(databasePath, key), "deleting");
      assert.equal(query(databasePath, `
        SELECT COUNT(*) AS count FROM better_auth_user WHERE id = '${USER_ID}'
      `)[0]?.count, 0);

      bucket.failDeletes = false;
      kv.failDeletes = false;
      assert.equal(await collectSyncR2Garbage(env, NOW + 61, { ownerHash: OWNER_HASH }), 1);
      assert.equal(await purgeLegacySessionCache(env), 1);
      assert.equal(candidateState(databasePath, key), "deleted");
      assert.equal(bucket.size, 0);
      assert.equal(kv.values.size, 0);
    });
  });

  it("rolls back account deletion when its authenticated authority changes before the batch", async () => {
    for (const beforeBatchSql of [
      "DELETE FROM better_auth_session WHERE id = 'session-01';",
      `UPDATE user_device_keys SET signing_public_key = '${"9".repeat(64)}'
       WHERE user_id = '${USER_ID}' AND device_id = '${DEVICE_ID}';`,
    ]) {
      await withDatabase(async (databasePath, bucket, _kv, env) => {
        const [key] = seedReadyCandidates(databasePath, bucket, 1);
        assert.ok(key !== undefined);
        const request = await accountDeleteRequest("account-delete-authority-race", NOW);
        const racedEnv = {
          ...env,
          ELY_DB: new SqliteD1Database(databasePath, beforeBatchSql),
        } as Env;

        await assert.rejects(
          () => accountDeletionDocument(request, racedEnv, authContext(), NOW),
          /device_action_gate_failed/,
        );

        assert.deepEqual(query(databasePath, `SELECT
          (SELECT COUNT(*) FROM better_auth_user WHERE id = '${USER_ID}') AS users,
          (SELECT COUNT(*) FROM user_devices WHERE user_id = '${USER_ID}') AS devices,
          (SELECT COUNT(*) FROM user_device_keys WHERE user_id = '${USER_ID}') AS device_keys,
          (SELECT COUNT(*) FROM sync_vault_accounts WHERE user_id = '${USER_ID}') AS vaults,
          (SELECT COUNT(*) FROM audit_events WHERE event_type = 'account.delete') AS audits
        `), [{ users: 1, devices: 1, device_keys: 1, vaults: 1, audits: 0 }]);
        assert.equal(candidateState(databasePath, key), "ready");
        assert.equal(bucket.size, 1);
      });
    }
  });

  it("continues R2 inventory and GC when legacy KV purge fails", async () => {
    await withDatabase(async (databasePath, bucket, kv, env) => {
      const hash = "f".repeat(64);
      const key = `sync-payloads/us-east/${OWNER_HASH}/bookmarks/object-01/${hash}.bin`;
      bucket.values.set(key, new Uint8Array([1]).buffer);
      kv.failLists = true;

      await assert.rejects(() => maintainSyncR2Storage(env, NOW), AggregateError);

      assert.equal(bucket.size, 0);
      assert.equal(candidateState(databasePath, key), "deleted");
    });
  });
});

async function withDatabase(
  run: (databasePath: string, bucket: TestBucket, kv: TestKv, env: Env) => Promise<void>,
): Promise<void> {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-account-reset-gc-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    for (const fileName of readdirSync(MIGRATIONS_DIR).filter((name) => name.endsWith(".sql")).sort()) {
      execute(databasePath, readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"));
    }
    seedAuthority(databasePath);
    const bucket = new TestBucket();
    const kv = new TestKv();
    const env = {
      ELY_DB: new SqliteD1Database(databasePath),
      ELY_STORAGE: bucket,
      ELY_KV: kv,
      ELY_ENVIRONMENT: "local",
    } as unknown as Env;
    await run(databasePath, bucket, kv, env);
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
      '${USER_ID}', '${DEVICE_ID}', '${PUBLIC_KEY}', '${"4".repeat(64)}', 2, 1
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

function seedReadyCandidates(databasePath: string, bucket: TestBucket, count: number): string[] {
  const keys = Array.from({ length: count }, (_, index) => snapshotKey(index + 1));
  execute(databasePath, keys.map((key, index) => `
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${key}', '${USER_ID}', '${OWNER_HASH}', 'snapshot', 'ready', NULL,
      0, NULL, 1, 1, NULL, 1, NULL, NULL
    );
  `).join("\n"));
  for (const [index, key] of keys.entries()) {
    bucket.values.set(key, new Uint8Array([index % 256]).buffer);
  }
  return keys;
}

function seedCompletedRotation(databasePath: string, r2Key: string): void {
  execute(databasePath, `
    INSERT INTO user_devices (
      user_id, device_id, public_key, device_name, platform,
      approval_status, created_at, approved_at, last_active_at, revoked_at, idempotency_key
    ) VALUES (
      '${USER_ID}', 'device-02', '${"5".repeat(64)}', 'Old Mac', 'macOS',
      'revoked', 1, 1, 1, 2, 'device-register-0002'
    );
    INSERT INTO sync_vault_rotations (
      user_id, idempotency_key, audit_event_id, target_device_id, approver_device_id,
      previous_key_id, previous_generation, new_key_id, new_generation, request_hash,
      envelope_count, r2_object_count, created_at, completed_at
    ) VALUES (
      '${USER_ID}', 'rotation-reset-0001', 'rotation-reset-audit', 'device-02', '${DEVICE_ID}',
      '${KEY_ID}', 1, '${"6".repeat(64)}', 2, '${"7".repeat(64)}', 1, 1, 1, 2
    );
    INSERT INTO sync_vault_rotation_r2_objects (
      user_id, rotation_idempotency_key, r2_key
    ) VALUES ('${USER_ID}', 'rotation-reset-0001', '${r2Key}');
  `);
}

function resetRequest(idempotencyKey: string, proofCreatedAt: number): Promise<Request> {
  return actionRequest(
    "sync.reset",
    "delete-cloud-sync-data",
    idempotencyKey,
    proofCreatedAt,
    "/api/sync/reset",
  );
}

function accountDeleteRequest(idempotencyKey: string, proofCreatedAt: number): Promise<Request> {
  return actionRequest(
    "account.delete",
    "delete-elydora-account",
    idempotencyKey,
    proofCreatedAt,
    "/api/account/delete",
  );
}

async function actionRequest(
  action: SensitiveAction,
  confirmation: string,
  idempotencyKey: string,
  proofCreatedAt: number,
  path: string,
): Promise<Request> {
  const actionProof = await signDeviceMessage(recentDeviceActionProofBytes({
    action,
    userId: USER_ID,
    sessionId: authContext().sessionId,
    deviceId: DEVICE_ID,
    confirmation,
    idempotencyKey,
    proofCreatedAt,
  }));
  return new Request(`https://elydora.test${path}`, {
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

function authContext() {
  return {
    userId: USER_ID,
    deviceId: DEVICE_ID,
    sessionId: "session-01",
    tokenHash: TOKEN_HASH,
    expiresAt: "2099-01-01T00:00:00Z",
    createdAt: "2026-01-01T00:00:00Z",
  } as const;
}

function snapshotKey(index: number): string {
  const hash = index.toString(16).padStart(64, "0");
  return `sync-snapshots/us-east/${OWNER_HASH}/snapshot-${index}/${hash}.bin`;
}

function deletedCandidateCount(databasePath: string): unknown {
  return query(databasePath, `
    SELECT COUNT(*) AS count FROM sync_r2_gc_candidates WHERE state = 'deleted'
  `)[0]?.count;
}

function candidateState(databasePath: string, key: string): unknown {
  return query(databasePath, `
    SELECT state FROM sync_r2_gc_candidates WHERE r2_key = '${key}'
  `)[0]?.state;
}

class TestBucket {
  readonly deletes: string[] = [];
  readonly values = new Map<string, ArrayBuffer>();
  failDeletes = false;

  get(key: string): Promise<ElyR2Object | null> {
    const value = this.values.get(key);
    return Promise.resolve(value === undefined ? null : object(value));
  }

  put(key: string, value: ArrayBuffer, _options?: ElyR2PutOptions): Promise<ElyR2Object> {
    this.values.set(key, value);
    return Promise.resolve(object(value));
  }

  delete(key: string): Promise<void> {
    if (this.failDeletes) return Promise.reject(new Error("r2_delete_failed"));
    this.deletes.push(key);
    this.values.delete(key);
    return Promise.resolve();
  }

  list(options: { prefix: string; cursor?: string; limit: number }) {
    const objects = [...this.values.keys()]
      .filter((key) => key.startsWith(options.prefix))
      .slice(0, options.limit)
      .map((key) => ({ key }));
    return Promise.resolve({ objects, truncated: false as const });
  }

  get size(): number {
    return this.values.size;
  }
}

class TestKv {
  readonly values = new Map<string, string>();
  failDeletes = false;
  failLists = false;

  get(key: string): Promise<string | null> {
    return Promise.resolve(this.values.get(key) ?? null);
  }

  put(key: string, value: string): Promise<void> {
    this.values.set(key, value);
    return Promise.resolve();
  }

  delete(key: string): Promise<void> {
    if (this.failDeletes) return Promise.reject(new Error("kv_delete_failed"));
    this.values.delete(key);
    return Promise.resolve();
  }

  list(options: { prefix: string; cursor?: string; limit: number }) {
    if (this.failLists) return Promise.reject(new Error("kv_list_failed"));
    const keys = [...this.values.keys()]
      .filter((key) => key.startsWith(options.prefix))
      .slice(0, options.limit)
      .map((name) => ({ name }));
    return Promise.resolve({ keys, list_complete: true as const });
  }
}

function object(value: ArrayBuffer): ElyR2Object {
  return { arrayBuffer: () => Promise.resolve(value) };
}
