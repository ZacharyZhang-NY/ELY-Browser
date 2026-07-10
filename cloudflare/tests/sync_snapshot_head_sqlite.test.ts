import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync, type SQLInputValue } from "node:sqlite";
import { describe, it } from "node:test";

import {
  SYNC_SNAPSHOT_CANDIDATE_UPSERT_QUERY,
  SYNC_SNAPSHOT_ENCRYPTION_UPSERT_QUERY,
  SYNC_SNAPSHOT_HEAD_INSERT_QUERY,
  SYNC_SNAPSHOT_HEAD_QUERY,
  SYNC_SNAPSHOT_HEAD_UPDATE_QUERY,
} from "../src/sync_snapshot_sql.js";
import { SYNC_R2_MARK_REFERENCED_QUERY } from "../src/sync_r2_gc.js";
import {
  SyncSnapshotHeadSchemaError,
  type SyncSnapshotRow,
  snapshotDocumentFromRow,
} from "../src/sync_snapshot_head.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const KEY_ID = "1".repeat(64);
const NEXT_KEY_ID = "2".repeat(64);
const WRITE_TOKEN = "9".repeat(64);
const MIGRATIONS_DIR = join(process.cwd(), "migrations");

interface HeadRef {
  revision: number;
  snapshotId: string;
  payloadHash: string;
}

interface Candidate {
  snapshotId: string;
  payloadHash: string;
  contentHash: string;
  logicalClock: number;
  headRevision: number;
  base: HeadRef | null;
  keyId?: string;
  generation?: number;
}

describe("sync snapshot head SQLite guards", () => {
  it("commits a genesis head through the route SQL", () => {
    using database = databaseWithApprovedDevice();
    const genesis = candidate({ payloadHash: "a".repeat(64) });

    commitCandidate(database, genesis);

    assert.deepEqual(currentHead(database), {
      head_revision: 1,
      snapshot_id: "device-01",
      payload_hash: "a".repeat(64),
    });
  });

  it("allows one writer per base even when the stale writer has a higher clock", () => {
    using database = databaseWithApprovedDevice();
    const genesis = candidate({ payloadHash: "a".repeat(64) });
    commitCandidate(database, genesis);
    const base = headRef(genesis);
    const winner = candidate({
      payloadHash: "b".repeat(64),
      contentHash: "3".repeat(64),
      logicalClock: 11,
      headRevision: 2,
      base,
    });
    commitCandidate(database, winner);
    const loser = candidate({
      payloadHash: "c".repeat(64),
      contentHash: "4".repeat(64),
      logicalClock: 999,
      headRevision: 2,
      base,
    });

    assert.throws(
      () => commitCandidate(database, loser),
      /sync_r2_write_fenced/,
    );
    assert.deepEqual(currentHead(database), {
      head_revision: 2,
      snapshot_id: "device-01",
      payload_hash: "b".repeat(64),
    });
    assert.deepEqual(snapshotState(database, "device-01"), {
      payload_hash: "b".repeat(64),
      content_hash: "3".repeat(64),
      logical_clock: 11,
      head_revision: 2,
    });
  });

  it("rolls back candidate metadata when the final head guard aborts", () => {
    using database = databaseWithApprovedDevice();
    const genesis = candidate({ payloadHash: "a".repeat(64) });
    commitCandidate(database, genesis);
    const child = candidate({
      snapshotId: "device-02",
      payloadHash: "b".repeat(64),
      contentHash: "3".repeat(64),
      logicalClock: 11,
      headRevision: 2,
      base: headRef(genesis),
    });

    assert.throws(
      () => commitCandidate(database, child, "f".repeat(64)),
      /sync_r2_write_fenced/,
    );
    assert.equal(snapshotState(database, "device-02"), undefined);
    assert.deepEqual(currentHead(database), {
      head_revision: 1,
      snapshot_id: "device-01",
      payload_hash: "a".repeat(64),
    });
  });

  it("advances an old-generation base with the current rotated key", () => {
    using database = databaseWithApprovedDevice();
    const genesis = candidate({ payloadHash: "a".repeat(64) });
    commitCandidate(database, genesis);
    database.prepare(`
      UPDATE sync_vault_accounts
      SET current_key_id = ?, current_generation = 2, updated_at = 2
      WHERE user_id = ?
    `).run(NEXT_KEY_ID, USER_ID);
    const child = candidate({
      payloadHash: "b".repeat(64),
      contentHash: "3".repeat(64),
      logicalClock: 11,
      headRevision: 2,
      base: headRef(genesis),
      keyId: NEXT_KEY_ID,
      generation: 2,
    });

    commitCandidate(database, child);

    assert.deepEqual(snapshotState(database, DEVICE_ID), {
      payload_hash: "b".repeat(64),
      content_hash: "3".repeat(64),
      logical_clock: 11,
      head_revision: 2,
    });
  });

  it("surfaces a current head whose encryption row is missing", () => {
    using database = databaseWithApprovedDevice();
    const genesis = candidate({ payloadHash: "a".repeat(64) });
    commitCandidate(database, genesis);
    const deletion = database.prepare(`
      DELETE FROM sync_snapshot_encryption
      WHERE user_id = ? AND snapshot_id = ?
    `);
    assert.throws(() => deletion.run(USER_ID, DEVICE_ID), /FOREIGN KEY constraint failed/);
    database.exec("PRAGMA foreign_keys = OFF");
    deletion.run(USER_ID, DEVICE_ID);
    database.exec("PRAGMA foreign_keys = ON");
    const row = database.prepare(SYNC_SNAPSHOT_HEAD_QUERY).get(USER_ID) as
      | SyncSnapshotRow
      | undefined;

    assert.ok(row !== undefined);
    assert.throws(
      () => snapshotDocumentFromRow(row),
      (error) =>
        error instanceof SyncSnapshotHeadSchemaError &&
        error.message === "encryption_version_invalid",
    );
  });
});

function databaseWithApprovedDevice(): DatabaseSync {
  const database = new DatabaseSync(":memory:");
  database.exec("PRAGMA foreign_keys = ON");
  for (const fileName of readdirSync(MIGRATIONS_DIR).filter((name) => name.endsWith(".sql")).sort()) {
    database.exec(readFileSync(join(MIGRATIONS_DIR, fileName), "utf8"));
  }
  database.exec(`
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
  return database;
}

function commitCandidate(
  database: DatabaseSync,
  value: Candidate,
  headPayloadHash = value.payloadHash,
): void {
  const snapshotValues = candidateValues(value);
  const r2Key = candidateR2Key(value);
  database.prepare(`
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (?, ?, ?, 'snapshot', 'pending', ?, 1000, NULL, 0, 0, NULL, NULL, NULL, NULL)
  `).run(r2Key, USER_ID, "f".repeat(64), WRITE_TOKEN);
  database.exec("BEGIN IMMEDIATE");
  try {
    database.prepare(SYNC_SNAPSHOT_CANDIDATE_UPSERT_QUERY).run(
      ...snapshotValues,
      value.keyId ?? KEY_ID,
      value.generation ?? 1,
      WRITE_TOKEN,
      value.headRevision,
    );
    database.prepare(SYNC_SNAPSHOT_ENCRYPTION_UPSERT_QUERY).run(
      ...snapshotValues,
      2,
      value.generation ?? 1,
      value.keyId ?? KEY_ID,
      value.contentHash,
      WRITE_TOKEN,
      value.headRevision,
    );
    if (value.base === null) {
      database.prepare(SYNC_SNAPSHOT_HEAD_INSERT_QUERY).run(
        USER_ID,
        value.headRevision,
        value.snapshotId,
        headPayloadHash,
        value.headRevision,
        r2Key,
        USER_ID,
        WRITE_TOKEN,
        value.headRevision,
      );
    } else {
      database.prepare(SYNC_SNAPSHOT_HEAD_UPDATE_QUERY).run(
        value.headRevision,
        value.snapshotId,
        headPayloadHash,
        value.headRevision,
        USER_ID,
        r2Key,
        USER_ID,
        WRITE_TOKEN,
        value.headRevision,
      );
    }
    database.prepare(SYNC_R2_MARK_REFERENCED_QUERY).run(
      value.headRevision,
      value.headRevision,
      value.headRevision,
      r2Key,
      USER_ID,
      WRITE_TOKEN,
      value.headRevision,
    );
    database.exec("COMMIT");
  } catch (error) {
    database.exec("ROLLBACK");
    throw error;
  }
}

function candidateValues(value: Candidate): SQLInputValue[] {
  return [
    USER_ID,
    value.snapshotId,
    candidateR2Key(value),
    value.payloadHash,
    1,
    value.logicalClock,
    DEVICE_ID,
    26,
    value.headRevision,
    value.headRevision,
    value.base?.revision ?? null,
    value.base?.snapshotId ?? null,
    value.base?.payloadHash ?? null,
  ];
}

function candidateR2Key(value: Candidate): string {
  return `sync-snapshots/us-east/${"f".repeat(64)}/${value.snapshotId}/${value.payloadHash}.bin`;
}

function candidate(overrides: Partial<Candidate>): Candidate {
  return {
    snapshotId: DEVICE_ID,
    payloadHash: "a".repeat(64),
    contentHash: "2".repeat(64),
    logicalClock: 10,
    headRevision: 1,
    base: null,
    ...overrides,
  };
}

function headRef(value: Candidate): HeadRef {
  return {
    revision: value.headRevision,
    snapshotId: value.snapshotId,
    payloadHash: value.payloadHash,
  };
}

function currentHead(database: DatabaseSync): Record<string, unknown> | undefined {
  const row = database.prepare(`
    SELECT head_revision, snapshot_id, payload_hash
    FROM sync_snapshot_heads
    WHERE user_id = ?
  `).get(USER_ID) as Record<string, unknown> | undefined;
  return row === undefined ? undefined : { ...row };
}

function snapshotState(
  database: DatabaseSync,
  snapshotId: string,
): Record<string, unknown> | undefined {
  const row = database.prepare(`
    SELECT
      snapshot.payload_hash,
      encryption.content_hash,
      snapshot.logical_clock,
      snapshot.head_revision
    FROM sync_snapshots AS snapshot
    INNER JOIN sync_snapshot_encryption AS encryption
      ON encryption.user_id = snapshot.user_id
      AND encryption.snapshot_id = snapshot.snapshot_id
    WHERE snapshot.user_id = ? AND snapshot.snapshot_id = ?
  `).get(USER_ID, snapshotId) as Record<string, unknown> | undefined;
  return row === undefined ? undefined : { ...row };
}
