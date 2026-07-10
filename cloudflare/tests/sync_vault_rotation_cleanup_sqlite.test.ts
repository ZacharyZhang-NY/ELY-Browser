import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { cleanupRotatedVaultStorage } from "../src/sync_vault_rotation_cleanup.js";
import { testEnv } from "./devices_test_support.js";
import { SqliteD1Database, execute, query } from "./sqlite_d1_test_support.js";

const MIGRATIONS_DIR = join(process.cwd(), "migrations");
const USER_ID = "user-01", APPROVER_ID = "device-01", TARGET_ID = "device-02";
const OLD_KEY = "a".repeat(64), NEW_KEY = "b".repeat(64);
const OLD_HASH = "c".repeat(64), NEW_HASH = "d".repeat(64), USER_HASH = "e".repeat(64);
const OLD_PAYLOAD_KEY = `sync-payloads/us/${USER_HASH}/bookmarks/object-01/${OLD_HASH}.bin`;
const OLD_SNAPSHOT_KEY = `sync-snapshots/us/${USER_HASH}/snapshot-01/${OLD_HASH}.bin`;
const NEW_SNAPSHOT_KEY = `sync-snapshots/us/${USER_HASH}/snapshot-02/${NEW_HASH}.bin`;
const CLEANUP_AT = 300;

describe("sync vault rotation cleanup real D1 flow", () => {
  it("cleans staged storage with a higher-clock non-head history row", async () => {
    await withDatabase(true, async (databasePath) => {
      seedHighClockNonHead(databasePath);
      const r2Deletes: string[] = [];
      await cleanupRotatedVaultStorage(
        testEnv({ d1: new SqliteD1Database(databasePath), r2Deletes }),
        USER_ID,
        "snapshot-02",
        NEW_KEY,
        2,
        CLEANUP_AT,
      );
      assert.deepEqual(r2Deletes, [OLD_PAYLOAD_KEY, OLD_SNAPSHOT_KEY]);
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT COUNT(*) FROM sync_objects WHERE user_id = '${USER_ID}') AS objects,
          (SELECT COUNT(*) FROM sync_snapshots
            WHERE user_id = '${USER_ID}' AND snapshot_id = 'snapshot-01') AS old_snapshot,
          (SELECT COUNT(*) FROM sync_snapshots
            WHERE user_id = '${USER_ID}' AND snapshot_id = 'snapshot-02') AS new_snapshot,
          (SELECT COUNT(*) FROM sync_snapshots
            WHERE user_id = '${USER_ID}' AND snapshot_id = 'snapshot-high') AS non_head,
          (SELECT head_revision FROM sync_snapshot_heads
            WHERE user_id = '${USER_ID}') AS head_revision,
          (SELECT storage_cleaned_at FROM sync_vault_rotations
            WHERE user_id = '${USER_ID}') AS storage_cleaned_at
      `), [{
        objects: 0,
        old_snapshot: 0,
        new_snapshot: 1,
        non_head: 1,
        head_revision: 2,
        storage_cleaned_at: CLEANUP_AT,
      }]);
    });
  });

  it("preserves the old head and R2 state for a CAS loser", async () => {
    await withDatabase(false, async (databasePath) => {
      const r2Deletes: string[] = [];
      await cleanupRotatedVaultStorage(
        testEnv({ d1: new SqliteD1Database(databasePath), r2Deletes }),
        USER_ID,
        "snapshot-02",
        NEW_KEY,
        2,
        CLEANUP_AT,
      );
      assert.deepEqual(r2Deletes, []);
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT COUNT(*) FROM sync_objects WHERE user_id = '${USER_ID}') AS objects,
          (SELECT COUNT(*) FROM sync_snapshots
            WHERE user_id = '${USER_ID}' AND snapshot_id = 'snapshot-01') AS old_snapshot,
          (SELECT snapshot_id FROM sync_snapshot_heads
            WHERE user_id = '${USER_ID}') AS head_snapshot_id,
          (SELECT cleanup_snapshot_id FROM sync_vault_rotations
            WHERE user_id = '${USER_ID}') AS cleanup_snapshot_id
      `), [{
        objects: 1,
        old_snapshot: 1,
        head_snapshot_id: "snapshot-01",
        cleanup_snapshot_id: null,
      }]);
    });
  });
});

async function withDatabase(
  commitReplacement: boolean,
  run: (databasePath: string) => Promise<void>,
): Promise<void> {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-rotation-cleanup-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    const migrations = readdirSync(MIGRATIONS_DIR)
      .filter((name) => name.endsWith(".sql"))
      .sort()
      .map((name) => readFileSync(join(MIGRATIONS_DIR, name), "utf8"))
      .join("\n");
    execute(databasePath, migrations);
    execute(databasePath, seedSql(commitReplacement));
    await run(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function seedSql(commitReplacement: boolean): string {
  return `
    INSERT INTO better_auth_user
      (id, name, email, emailVerified, createdAt, updatedAt)
    VALUES ('${USER_ID}', 'User', 'user@example.com', 1, '2026-01-01', '2026-01-01');
    INSERT INTO user_devices
      (user_id, device_id, public_key, device_name, platform, approval_status,
       created_at, approved_at, last_active_at, revoked_at, idempotency_key)
    VALUES
      ('${USER_ID}', '${APPROVER_ID}', '${"1".repeat(64)}', 'Approver', 'macOS',
       'approved', 10, 11, 12, NULL, 'device-register-0001'),
      ('${USER_ID}', '${TARGET_ID}', '${"2".repeat(64)}', 'Target', 'macOS',
       'approved', 10, 11, 12, NULL, 'device-register-0002'),
      ('${USER_ID}', 'device-03', '${"3".repeat(64)}', 'Remaining', 'macOS',
       'approved', 10, 11, 12, NULL, 'device-register-0003');
    INSERT INTO user_device_keys
      (user_id, device_id, signing_public_key, wrapping_public_key,
       key_protocol_version, created_at)
    VALUES
      ('${USER_ID}', '${APPROVER_ID}', '${"1".repeat(64)}', '${"4".repeat(64)}', 2, 10),
      ('${USER_ID}', '${TARGET_ID}', '${"2".repeat(64)}', '${"5".repeat(64)}', 2, 10),
      ('${USER_ID}', 'device-03', '${"3".repeat(64)}', '${"6".repeat(64)}', 2, 10);
    INSERT INTO sync_vault_accounts
      (user_id, current_key_id, current_generation, created_at, updated_at)
    VALUES ('${USER_ID}', '${OLD_KEY}', 1, 20, 20);
    ${ledgerSql(OLD_PAYLOAD_KEY, "payload", "1".repeat(64), 30)}
    INSERT INTO sync_objects
      (user_id, object_id, object_type, payload_inline, payload_r2_key, payload_hash,
       schema_rev, logical_clock, device_id, created_at, updated_at, deleted_at)
    VALUES ('${USER_ID}', 'object-01', 'bookmarks', NULL, '${OLD_PAYLOAD_KEY}', '${OLD_HASH}',
      1, 1, '${APPROVER_ID}', 30, 30, NULL);
    UPDATE sync_r2_gc_candidates
    SET state = 'referenced', referenced_at = 30, updated_at = 30
    WHERE r2_key = '${OLD_PAYLOAD_KEY}';
    ${ledgerSql(OLD_SNAPSHOT_KEY, "snapshot", "2".repeat(64), 40)}
    INSERT INTO sync_snapshots
      (user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
       device_id, size_bytes, created_at, head_revision,
       base_head_revision, base_snapshot_id, base_payload_hash)
    VALUES ('${USER_ID}', 'snapshot-01', '${OLD_SNAPSHOT_KEY}', '${OLD_HASH}', 1, 1,
      '${APPROVER_ID}', 64, 40, 1, NULL, NULL, NULL);
    INSERT INTO sync_snapshot_encryption
      (user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash)
    VALUES ('${USER_ID}', 'snapshot-01', 2, 1, '${OLD_KEY}', '${OLD_HASH}');
    INSERT INTO sync_snapshot_heads
      (user_id, head_revision, snapshot_id, payload_hash, updated_at)
    VALUES ('${USER_ID}', 1, 'snapshot-01', '${OLD_HASH}', 40);
    UPDATE sync_r2_gc_candidates
    SET state = 'referenced', referenced_at = 40, updated_at = 40
    WHERE r2_key = '${OLD_SNAPSHOT_KEY}';
    ${rotationSql()}
    ${ledgerSql(NEW_SNAPSHOT_KEY, "snapshot", "3".repeat(64), 250)}
    INSERT INTO sync_snapshots
      (user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
       device_id, size_bytes, created_at, head_revision,
       base_head_revision, base_snapshot_id, base_payload_hash)
    VALUES ('${USER_ID}', 'snapshot-02', '${NEW_SNAPSHOT_KEY}', '${NEW_HASH}', 1, 2,
      '${APPROVER_ID}', 64, 250, 2, 1, 'snapshot-01', '${OLD_HASH}');
    INSERT INTO sync_snapshot_encryption
      (user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash)
    VALUES ('${USER_ID}', 'snapshot-02', 2, 2, '${NEW_KEY}', '${NEW_HASH}');
    ${commitReplacement ? `
      UPDATE sync_snapshot_heads
      SET head_revision = 2, snapshot_id = 'snapshot-02',
          payload_hash = '${NEW_HASH}', updated_at = 250
      WHERE user_id = '${USER_ID}';
      UPDATE sync_r2_gc_candidates
      SET state = 'referenced', lease_expires_at = 250,
          referenced_at = 250, updated_at = 250
      WHERE r2_key = '${NEW_SNAPSHOT_KEY}';
    ` : ""}
  `;
}

function ledgerSql(r2Key: string, kind: "payload" | "snapshot", token: string, now: number): string {
  return `
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${r2Key}', '${USER_ID}', '${USER_HASH}', '${kind}', 'pending', '${token}',
      1000, NULL, ${now}, ${now}, NULL, NULL, NULL, NULL
    );
  `;
}

function seedHighClockNonHead(databasePath: string): void {
  const hash = "f".repeat(64);
  const r2Key = `sync-snapshots/us/${USER_HASH}/snapshot-high/${hash}.bin`;
  execute(databasePath, `
    ${ledgerSql(r2Key, "snapshot", "4".repeat(64), 260)}
    INSERT INTO sync_snapshots (
      user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
      device_id, size_bytes, created_at, head_revision,
      base_head_revision, base_snapshot_id, base_payload_hash
    ) VALUES (
      '${USER_ID}', 'snapshot-high', '${r2Key}', '${hash}', 1,
      ${Number.MAX_SAFE_INTEGER}, '${APPROVER_ID}', 64, 260, 0, NULL, NULL, NULL
    );
    INSERT INTO sync_snapshot_encryption (
      user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash
    ) VALUES ('${USER_ID}', 'snapshot-high', 2, 2, '${NEW_KEY}', '${hash}');
  `);
}

function rotationSql(): string {
  return `
    INSERT INTO sync_vault_rotations
      (user_id, idempotency_key, audit_event_id, target_device_id, approver_device_id,
       previous_key_id, previous_generation, new_key_id, new_generation, request_hash,
       envelope_count, r2_object_count, created_at, completed_at)
    VALUES ('${USER_ID}', 'rotation-key-0001', 'rotation-audit-0001', '${TARGET_ID}',
      '${APPROVER_ID}', '${OLD_KEY}', 1, '${NEW_KEY}', 2, '${"7".repeat(64)}', 2, 2, 100, NULL);
    INSERT INTO sync_vault_rotation_envelopes
      (user_id, rotation_idempotency_key, recipient_device_id, envelope_idempotency_key,
       envelope_version, suite, encapped_key, ciphertext)
    VALUES
      ('${USER_ID}', 'rotation-key-0001', '${APPROVER_ID}', '${"8".repeat(64)}', 1,
       'HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305', '${"A".repeat(43)}', '${"B".repeat(64)}'),
      ('${USER_ID}', 'rotation-key-0001', 'device-03', '${"9".repeat(64)}', 1,
       'HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305', '${"C".repeat(42)}E', '${"D".repeat(64)}');
    UPDATE sync_vault_rotations SET completed_at = 200
    WHERE user_id = '${USER_ID}' AND idempotency_key = 'rotation-key-0001';
  `;
}
