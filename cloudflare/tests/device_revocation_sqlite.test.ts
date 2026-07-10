import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

const MIGRATIONS_DIR = join(process.cwd(), "migrations");
const OLD_KEY = "a".repeat(64);
const NEW_KEY = "b".repeat(64);
const HASH = "c".repeat(64);
const USER_HASH = "d".repeat(64);
const WRAPPING_KEY = "e".repeat(64);
const SIGNING_KEY = "f".repeat(64);
const PAYLOAD_R2_KEY = `sync-payloads/us/${USER_HASH}/bookmarks/object-01/${HASH}.bin`;
const SNAPSHOT_R2_KEY = `sync-snapshots/us/${USER_HASH}/snapshot-01/${HASH}.bin`;

describe("sync vault rotation migration", () => {
  it("finalizes atomically and retains the old head until replacement", () => {
    withDatabase((databasePath) => {
      execute(databasePath, seedSql());
      execute(databasePath, validRotationSql());

      assert.deepEqual(query(databasePath, `
        SELECT current_key_id, current_generation FROM sync_vault_accounts WHERE user_id = 'user-01'
      `), [{ current_key_id: NEW_KEY, current_generation: 2 }]);
      assert.deepEqual(query(databasePath, `
        SELECT approval_status, revoked_at FROM user_devices
        WHERE user_id = 'user-01' AND device_id = 'device-02'
      `), [{ approval_status: "revoked", revoked_at: 200 }]);
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT COUNT(*) FROM sync_vault_envelopes
            WHERE user_id = 'user-01' AND key_id = '${NEW_KEY}' AND generation = 2) AS envelopes,
          (SELECT COUNT(*) FROM audit_events
            WHERE event_id = 'device-revoke:user-01:rotation-key-0001') AS audits,
          (SELECT COUNT(*) FROM sync_vault_rotation_r2_objects
            WHERE user_id = 'user-01' AND rotation_idempotency_key = 'rotation-key-0001') AS r2_manifest,
          (SELECT COUNT(*) FROM sync_objects WHERE user_id = 'user-01') AS objects,
          (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = 'user-01') AS snapshots,
          (SELECT COUNT(*) FROM sync_snapshot_encryption WHERE user_id = 'user-01') AS encryption
      `), [{ envelopes: 2, audits: 1, r2_manifest: 2, objects: 1, snapshots: 1, encryption: 1 }]);
    });
  });

  it("rolls back every mutation when the staged recipient set is incomplete", () => {
    withDatabase((databasePath) => {
      execute(databasePath, seedSql());
      assert.throws(
        () => execute(databasePath, invalidRotationSql()),
        /sync_vault_rotation_guard_failed/,
      );
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT current_generation FROM sync_vault_accounts WHERE user_id = 'user-01') AS generation,
          (SELECT approval_status FROM user_devices
            WHERE user_id = 'user-01' AND device_id = 'device-02') AS target_status,
          (SELECT COUNT(*) FROM audit_events WHERE user_id = 'user-01') AS audits,
          (SELECT COUNT(*) FROM sync_vault_rotations WHERE user_id = 'user-01') AS rotations
      `), [{ generation: 1, target_status: "approved", audits: 0, rotations: 0 }]);
    });
  });

  it("quarantines approved protocol-v1 devices when 0011 is applied", () => {
    withDatabase((databasePath) => {
      execute(databasePath, `
        INSERT INTO better_auth_user
          (id, name, email, emailVerified, createdAt, updatedAt)
        VALUES ('legacy-user', 'Legacy', 'legacy@example.com', 1, '2026-01-01', '2026-01-01');
        INSERT INTO user_devices
          (user_id, device_id, public_key, device_name, platform, approval_status,
           created_at, approved_at, last_active_at, revoked_at, idempotency_key)
        VALUES
          ('legacy-user', 'legacy-device', '${SIGNING_KEY}', 'Legacy', 'macOS', 'approved',
           10, 11, 12, NULL, 'legacy-register-0001');
        INSERT INTO user_device_keys
          (user_id, device_id, signing_public_key, wrapping_public_key,
           key_protocol_version, created_at)
        VALUES ('legacy-user', 'legacy-device', '${SIGNING_KEY}', NULL, 1, 10);
      `);
      execute(databasePath, readFileSync(join(MIGRATIONS_DIR, "0011_sync_vault_rotation.sql"), "utf8"));
      assert.deepEqual(query(databasePath, `
        SELECT approval_status, revoked_at IS NOT NULL AS has_revoked_at
        FROM user_devices WHERE user_id = 'legacy-user' AND device_id = 'legacy-device'
      `), [{ approval_status: "revoked", has_revoked_at: 1 }]);
    });
  });
});

function seedSql(): string {
  return `
    INSERT INTO better_auth_user
      (id, name, email, emailVerified, createdAt, updatedAt)
    VALUES ('user-01', 'User', 'user@example.com', 1, '2026-01-01', '2026-01-01');
    INSERT INTO user_devices
      (user_id, device_id, public_key, device_name, platform, approval_status,
       created_at, approved_at, last_active_at, revoked_at, idempotency_key)
    VALUES
      ('user-01', 'device-01', '${SIGNING_KEY}', 'Approver', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0001'),
      ('user-01', 'device-02', '${SIGNING_KEY}', 'Target', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0002'),
      ('user-01', 'device-03', '${SIGNING_KEY}', 'Remaining', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0003');
    INSERT INTO user_device_keys
      (user_id, device_id, signing_public_key, wrapping_public_key,
       key_protocol_version, created_at)
    VALUES
      ('user-01', 'device-01', '${SIGNING_KEY}', '${WRAPPING_KEY}', 2, 10),
      ('user-01', 'device-02', '${SIGNING_KEY}', '${WRAPPING_KEY}', 2, 10),
      ('user-01', 'device-03', '${SIGNING_KEY}', '${WRAPPING_KEY}', 2, 10);
    INSERT INTO sync_vault_accounts
      (user_id, current_key_id, current_generation, created_at, updated_at)
    VALUES ('user-01', '${OLD_KEY}', 1, 20, 20);
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${PAYLOAD_R2_KEY}', 'user-01', '${USER_HASH}', 'payload', 'pending',
      '${"1".repeat(64)}', 1000, NULL, 30, 30, NULL, NULL, NULL, NULL
    );
    INSERT INTO sync_objects
      (user_id, object_id, object_type, payload_inline, payload_r2_key, payload_hash,
       schema_rev, logical_clock, device_id, created_at, updated_at, deleted_at)
    VALUES
      ('user-01', 'object-01', 'bookmarks', NULL, '${PAYLOAD_R2_KEY}', '${HASH}',
       1, 1, 'device-01', 30, 30, NULL);
    UPDATE sync_r2_gc_candidates
    SET state = 'referenced', referenced_at = 30, updated_at = 30
    WHERE r2_key = '${PAYLOAD_R2_KEY}';
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${SNAPSHOT_R2_KEY}', 'user-01', '${USER_HASH}', 'snapshot', 'pending',
      '${"2".repeat(64)}', 1000, NULL, 40, 40, NULL, NULL, NULL, NULL
    );
    INSERT INTO sync_snapshots
      (user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
       device_id, size_bytes, created_at)
    VALUES
      ('user-01', 'snapshot-01', '${SNAPSHOT_R2_KEY}', '${HASH}', 1, 1,
       'device-01', 64, 40);
    INSERT INTO sync_snapshot_encryption
      (user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash)
    VALUES ('user-01', 'snapshot-01', 1, 1, '${OLD_KEY}', '${HASH}');
  `;
}

function validRotationSql(): string {
  return `
    BEGIN IMMEDIATE;
    ${rotationHeaderSql(2, 2)}
    ${rotationEnvelopeSql("device-01", "1".repeat(64), "A".repeat(43), "B".repeat(64))}
    ${rotationEnvelopeSql("device-03", "2".repeat(64), `${"C".repeat(42)}E`, "D".repeat(64))}
    UPDATE sync_vault_rotations SET completed_at = 200
    WHERE user_id = 'user-01' AND idempotency_key = 'rotation-key-0001';
    COMMIT;
  `;
}

function invalidRotationSql(): string {
  return `
    BEGIN IMMEDIATE;
    ${rotationHeaderSql(2, 2)}
    ${rotationEnvelopeSql("device-01", "1".repeat(64), "A".repeat(43), "B".repeat(64))}
    UPDATE sync_vault_rotations SET completed_at = 200
    WHERE user_id = 'user-01' AND idempotency_key = 'rotation-key-0001';
    COMMIT;
  `;
}

function rotationHeaderSql(envelopeCount: number, r2Count: number): string {
  return `
    INSERT INTO sync_vault_rotations
      (user_id, idempotency_key, audit_event_id, target_device_id, approver_device_id,
       previous_key_id, previous_generation, new_key_id, new_generation, request_hash,
       envelope_count, r2_object_count, created_at, completed_at)
    VALUES
      ('user-01', 'rotation-key-0001', 'device-revoke:user-01:rotation-key-0001',
       'device-02', 'device-01', '${OLD_KEY}', 1, '${NEW_KEY}', 2, '${HASH}',
       ${envelopeCount}, ${r2Count}, 100, NULL);
  `;
}

function rotationEnvelopeSql(
  recipient: string,
  idempotencyKey: string,
  encappedKey: string,
  ciphertext: string,
): string {
  return `
    INSERT INTO sync_vault_rotation_envelopes
      (user_id, rotation_idempotency_key, recipient_device_id, envelope_idempotency_key,
       envelope_version, suite, encapped_key, ciphertext)
    VALUES
      ('user-01', 'rotation-key-0001', '${recipient}', '${idempotencyKey}', 1,
       'HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305', '${encappedKey}', '${ciphertext}');
  `;
}

function withDatabase(assertions: (databasePath: string) => void): void {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-rotation-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    const migrations = readdirSync(MIGRATIONS_DIR)
      .filter((name) => name.endsWith(".sql"))
      .sort()
      .map((name) => readFileSync(join(MIGRATIONS_DIR, name), "utf8"))
      .join("\n");
    execute(databasePath, migrations);
    assertions(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function execute(databasePath: string, sql: string): void {
  execFileSync("sqlite3", [databasePath], {
    input: `.bail on\nPRAGMA foreign_keys = ON;\n${sql}`,
    stdio: ["pipe", "pipe", "pipe"],
  });
}

function query(databasePath: string, sql: string): Record<string, unknown>[] {
  const output = execFileSync("sqlite3", ["-json", databasePath, sql], { encoding: "utf8" });
  return JSON.parse(output) as Record<string, unknown>[];
}
