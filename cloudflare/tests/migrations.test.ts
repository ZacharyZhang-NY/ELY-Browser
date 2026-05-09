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
];
const USER_SCOPED_TABLES = [
  "user_devices",
  "device_approvals",
  "sync_objects",
  "sync_change_log",
  "sync_snapshots",
  "sync_tombstones",
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
        "plugin_packages",
        "plugin_registry",
        "plugin_reviews",
        "release_manifests",
        "sync_change_log",
        "sync_objects",
        "sync_snapshots",
        "sync_tombstones",
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
