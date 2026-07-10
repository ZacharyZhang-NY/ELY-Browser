import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

const MIGRATIONS_DIR = join(process.cwd(), "migrations");
const EXPECTED_MIGRATIONS = [
  "0001_devices.sql",
  "0002_sync.sql",
  "0003_plugins.sql",
  "0004_releases.sql",
  "0005_audit.sql",
  "0006_better_auth.sql",
  "0007_better_auth_session_device_context.sql",
  "0008_sync_encryption.sql",
  "0009_sync_vault.sql",
  "0010_device_trust.sql",
  "0011_sync_vault_rotation.sql",
  "0012_sync_snapshot_head.sql",
  "0013_sync_r2_gc.sql",
];
const USER_SCOPED_TABLES = [
  "user_devices",
  "device_approvals",
  "device_rebind_challenges",
  "pending_device_revocations",
  "sync_objects",
  "sync_r2_gc_candidates",
  "sync_change_log",
  "sync_snapshots",
  "sync_snapshot_encryption",
  "sync_snapshot_heads",
  "sync_tombstones",
  "sync_vault_accounts",
  "sync_vault_envelopes",
  "sync_vault_rotation_envelopes",
  "sync_vault_rotation_r2_objects",
  "sync_vault_rotations",
  "user_device_keys",
];

describe("D1 migrations", () => {
  it("keep the PRD migration order explicit", () => {
    assert.deepEqual(migrationFiles(), EXPECTED_MIGRATIONS);
  });

  it("replays cleanly and creates the PRD custom tables", () => {
    withReplayedDatabase((databasePath) => {
      const tables = sqliteJson<{ name: string }>(
        databasePath,
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
      ).map((row) => row.name);

      for (const table of [
        "audit_events",
        "better_auth_account",
        "better_auth_session",
        "better_auth_session_device_context",
        "better_auth_user",
        "better_auth_verification",
        "device_approvals",
        "device_rebind_challenges",
        "pending_device_revocations",
        "plugin_packages",
        "plugin_registry",
        "plugin_reviews",
        "release_manifests",
        "sync_change_log",
        "sync_objects",
        "sync_r2_gc_candidates",
        "sync_r2_inventory_cursors",
        "sync_snapshots",
        "sync_snapshot_encryption",
        "sync_snapshot_heads",
        "sync_tombstones",
        "sync_vault_accounts",
        "sync_vault_envelopes",
        "sync_vault_rotation_envelopes",
        "sync_vault_rotation_r2_objects",
        "sync_vault_rotations",
        "user_device_keys",
        "user_devices",
      ]) {
        assert.ok(tables.includes(table), table);
      }
    });
  });

  it("creates Better Auth tables compatible with the Worker auth schema", () => {
    withReplayedDatabase((databasePath) => {
      assert.deepEqual(
        requiredColumns(databasePath, "better_auth_user", [
          "id",
          "name",
          "email",
          "emailVerified",
          "createdAt",
          "updatedAt",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "better_auth_session", [
          "id",
          "expiresAt",
          "token",
          "createdAt",
          "updatedAt",
          "userId",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "better_auth_account", [
          "id",
          "accountId",
          "providerId",
          "userId",
          "password",
          "createdAt",
          "updatedAt",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "better_auth_verification", [
          "id",
          "identifier",
          "value",
          "expiresAt",
          "createdAt",
          "updatedAt",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "better_auth_session_device_context", [
          "session_id",
          "user_id",
          "device_id",
          "updated_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "user_device_keys", [
          "user_id",
          "device_id",
          "signing_public_key",
          "wrapping_public_key",
          "key_protocol_version",
          "created_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "device_rebind_challenges", [
          "challenge_id",
          "user_id",
          "session_id",
          "device_id",
          "challenge",
          "created_at",
          "expires_at",
          "consumed_at",
          "consumption_nonce",
        ]),
        [],
      );
    });
  });

  it("keeps user-scoped D1 tables partitioned by user_id", () => {
    withReplayedDatabase((databasePath) => {
      for (const table of USER_SCOPED_TABLES) {
        assert.ok(tableColumns(databasePath, table).includes("user_id"), table);
      }
    });
  });

  it("keeps sync facts compatible with cursor pull and tombstones", () => {
    withReplayedDatabase((databasePath) => {
      assert.deepEqual(
        requiredColumns(databasePath, "sync_objects", [
          "object_id",
          "object_type",
          "logical_clock",
          "device_id",
          "deleted_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_change_log", [
          "change_id",
          "object_id",
          "object_type",
          "operation",
          "logical_clock",
          "device_id",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_snapshots", [
          "head_revision",
          "base_head_revision",
          "base_snapshot_id",
          "base_payload_hash",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_snapshot_encryption", [
          "user_id",
          "snapshot_id",
          "encryption_version",
          "vault_generation",
          "key_id",
          "content_hash",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_snapshot_heads", [
          "user_id",
          "head_revision",
          "snapshot_id",
          "payload_hash",
          "updated_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_r2_gc_candidates", [
          "r2_key",
          "user_id",
          "owner_hash",
          "object_kind",
          "state",
          "write_token",
          "lease_expires_at",
          "gc_token",
          "deleted_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_vault_accounts", [
          "user_id",
          "current_key_id",
          "current_generation",
          "created_at",
          "updated_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_vault_envelopes", [
          "user_id",
          "recipient_device_id",
          "approver_device_id",
          "key_id",
          "generation",
          "envelope_version",
          "suite",
          "encapped_key",
          "ciphertext",
          "idempotency_key",
          "created_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "pending_device_revocations", [
          "user_id",
          "idempotency_key",
          "target_device_id",
          "approver_device_id",
          "request_hash",
          "completed_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_vault_rotations", [
          "user_id",
          "idempotency_key",
          "target_device_id",
          "approver_device_id",
          "previous_key_id",
          "previous_generation",
          "new_key_id",
          "new_generation",
          "request_hash",
          "envelope_count",
          "r2_object_count",
          "completed_at",
          "cleanup_snapshot_id",
          "cleanup_started_at",
          "storage_cleaned_at",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_vault_rotation_envelopes", [
          "user_id",
          "rotation_idempotency_key",
          "recipient_device_id",
          "envelope_idempotency_key",
          "envelope_version",
          "suite",
          "encapped_key",
          "ciphertext",
        ]),
        [],
      );
      assert.deepEqual(
        requiredColumns(databasePath, "sync_vault_rotation_r2_objects", [
          "user_id",
          "rotation_idempotency_key",
          "r2_key",
        ]),
        [],
      );
    });
  });

  it("backfills one deterministic legacy encrypted head per user", () => {
    withDatabaseBeforeSnapshotHeadMigration((databasePath) => {
      execFileSync("sqlite3", [databasePath], {
        input: `
          INSERT INTO sync_snapshots (
            user_id, snapshot_id, r2_key, payload_hash, schema_rev,
            logical_clock, device_id, size_bytes, created_at
          ) VALUES
            ('user-01', 'snapshot-b', 'key-b', '${"b".repeat(64)}', 1, 2, 'device-01', 1, 100),
            ('user-01', 'snapshot-a', 'key-a', '${"a".repeat(64)}', 1, 3, 'device-01', 1, 100),
            ('user-01', 'snapshot-c', 'key-c', '${"c".repeat(64)}', 1, 1, 'device-01', 1, 90);
          INSERT INTO sync_snapshot_encryption (
            user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash
          ) VALUES
            ('user-01', 'snapshot-a', 1, 1, '${"1".repeat(64)}', '${"2".repeat(64)}'),
            ('user-01', 'snapshot-b', 1, 1, '${"1".repeat(64)}', '${"3".repeat(64)}'),
            ('user-01', 'snapshot-c', 1, 1, '${"1".repeat(64)}', '${"4".repeat(64)}');
        `,
      });
      execFileSync("sqlite3", [databasePath], {
        input: `PRAGMA foreign_keys = ON;\n${readFileSync(
          join(MIGRATIONS_DIR, "0012_sync_snapshot_head.sql"),
          "utf8",
        )}`,
      });

      assert.deepEqual(
        sqliteJson(databasePath, `
          SELECT head_revision, snapshot_id, payload_hash
          FROM sync_snapshot_heads
          WHERE user_id = 'user-01'
        `),
        [{ head_revision: 1, snapshot_id: "snapshot-a", payload_hash: "a".repeat(64) }],
      );
      assert.deepEqual(
        sqliteJson(databasePath, `
          SELECT snapshot_id, head_revision
          FROM sync_snapshots
          WHERE user_id = 'user-01'
          ORDER BY snapshot_id
        `),
        [
          { snapshot_id: "snapshot-a", head_revision: 1 },
          { snapshot_id: "snapshot-b", head_revision: 0 },
          { snapshot_id: "snapshot-c", head_revision: 0 },
        ],
      );
      assert.deepEqual(
        sqliteJson(databasePath, `
          SELECT DISTINCT encryption_version
          FROM sync_snapshot_encryption
        `),
        [{ encryption_version: 1 }],
      );
    });
  });
});

function migrationFiles(): string[] {
  return readdirSync(MIGRATIONS_DIR).filter((fileName) => fileName.endsWith(".sql")).sort();
}

function withReplayedDatabase(assertions: (databasePath: string) => void): void {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-d1-migrations-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    const sql = migrationFiles()
      .map((fileName) => readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"))
      .join("\n");
    execFileSync("sqlite3", [databasePath], { input: sql });
    assertions(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function withDatabaseBeforeSnapshotHeadMigration(
  assertions: (databasePath: string) => void,
): void {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-d1-before-head-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    const sql = migrationFiles()
      .filter((fileName) => fileName < "0012_sync_snapshot_head.sql")
      .map((fileName) => readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"))
      .join("\n");
    execFileSync("sqlite3", [databasePath], { input: sql });
    assertions(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function tableColumns(databasePath: string, table: string): string[] {
  return sqliteJson<{ name: string }>(databasePath, `PRAGMA table_info(${table})`).map(
    (row) => row.name,
  );
}

function requiredColumns(databasePath: string, table: string, columns: string[]): string[] {
  const existingColumns = new Set(tableColumns(databasePath, table));
  return columns.filter((column) => !existingColumns.has(column));
}

function sqliteJson<T>(databasePath: string, sql: string): T[] {
  const output = execFileSync("sqlite3", ["-json", databasePath, sql], {
    encoding: "utf8",
  });
  return JSON.parse(output) as T[];
}
