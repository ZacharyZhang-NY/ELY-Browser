import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { DeviceConflictError } from "../src/device_schema.js";
import { revokeDeviceDocument } from "../src/device_revocation.js";
import {
  type ApprovedDeviceRevocationRequest,
  type PendingDeviceRevocationRequest,
  deviceRevocationProofBytes,
  pendingDeviceRevocationProofBytes,
} from "../src/device_revocation_schema.js";
import {
  PUBLIC_KEY,
  WRAPPING_PUBLIC_KEY,
  signDeviceMessage,
  testEnv,
} from "./devices_test_support.js";
import { SqliteD1Database, execute, query } from "./sqlite_d1_test_support.js";

const MIGRATIONS_DIR = join(process.cwd(), "migrations");
const USER_ID = "user-01", APPROVER_ID = "device-01";
const TARGET_ID = "device-02", REMAINING_ID = "device-03";
const OLD_KEY = "a".repeat(64), NEW_KEY = "b".repeat(64);
const HASH = "c".repeat(64), USER_HASH = "d".repeat(64);
const IDEMPOTENCY_KEY = "rotation-key-0001", NOW = 200;
const SUITE = "HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305";
const PAYLOAD_R2_KEY = `sync-payloads/us/${USER_HASH}/bookmarks/object-01/${HASH}.bin`;
const SNAPSHOT_R2_KEY = `sync-snapshots/us/${USER_HASH}/snapshot-01/${HASH}.bin`;
describe("device revocation real D1 flow", () => {
  it("executes the handler queries and trigger atomically", async () => {
    await withDatabase(async (databasePath) => {
      const database = new SqliteD1Database(databasePath);
      const document = await revokeDeviceDocument(
        await revocationRequest(),
        testEnv({ d1: database }),
        authContext(),
        NOW,
      );

      assert.equal(document.mode, "approved_rotate");
      if (document.mode !== "approved_rotate") throw new Error("approved rotation expected");
      assert.equal(document.generation, 2);
      assert.equal(document.key_id, NEW_KEY);
      assert.equal(document.device.approval_status, "revoked");
      assert.equal(document.device.revoked_at, NOW);
      assert.deepEqual(database.batches, [4]);
      assert.deepEqual(query(databasePath, `
        SELECT current_key_id, current_generation
        FROM sync_vault_accounts WHERE user_id = '${USER_ID}'
      `), [{ current_key_id: NEW_KEY, current_generation: 2 }]);
      assert.deepEqual(query(databasePath, `
        SELECT recipient_device_id, approver_device_id, key_id, generation,
               envelope_version, suite, encapped_key, ciphertext, created_at
        FROM sync_vault_envelopes
        WHERE user_id = '${USER_ID}' AND key_id = '${NEW_KEY}'
        ORDER BY recipient_device_id
      `), [
        envelopeRow(APPROVER_ID, "A".repeat(43), "B".repeat(64)),
        envelopeRow(REMAINING_ID, `${"C".repeat(42)}E`, "D".repeat(64)),
      ]);
      assert.deepEqual(query(databasePath, `
        SELECT target.approval_status, target.revoked_at,
               rotation.previous_generation, rotation.new_generation,
               rotation.envelope_count, rotation.r2_object_count,
               rotation.completed_at
        FROM user_devices AS target
        INNER JOIN sync_vault_rotations AS rotation
          ON rotation.user_id = target.user_id
         AND rotation.target_device_id = target.device_id
        WHERE target.user_id = '${USER_ID}' AND target.device_id = '${TARGET_ID}'
      `), [{
        approval_status: "revoked",
        revoked_at: NOW,
        previous_generation: 1,
        new_generation: 2,
        envelope_count: 2,
        r2_object_count: 2,
        completed_at: NOW,
      }]);
      assert.deepEqual(query(databasePath, `
        SELECT actor_device_id, event_type, subject_type, subject_id, outcome, created_at,
               event_id = 'device-revoke:' || (SELECT request_hash FROM sync_vault_rotations
                 WHERE user_id = '${USER_ID}' AND idempotency_key = '${IDEMPOTENCY_KEY}')
                 AS event_id_matches,
               metadata_hash = (SELECT request_hash FROM sync_vault_rotations
                 WHERE user_id = '${USER_ID}' AND idempotency_key = '${IDEMPOTENCY_KEY}')
                 AS request_hash_matches
        FROM audit_events WHERE user_id = '${USER_ID}'
      `), [{
        actor_device_id: APPROVER_ID,
        event_type: "device.revoke",
        subject_type: "device",
        subject_id: TARGET_ID,
        outcome: "success",
        created_at: NOW,
        event_id_matches: 1,
        request_hash_matches: 1,
      }]);
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT COUNT(*) FROM sync_objects WHERE user_id = '${USER_ID}') AS objects,
          (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = '${USER_ID}') AS snapshots,
          (SELECT COUNT(*) FROM sync_snapshot_encryption WHERE user_id = '${USER_ID}') AS encryption,
          (SELECT COUNT(*) FROM sync_vault_rotation_r2_objects
            WHERE user_id = '${USER_ID}') AS staged_r2,
          (SELECT COUNT(*) FROM better_auth_session
            WHERE id = 'target-session') AS target_sessions
      `), [{ objects: 1, snapshots: 1, encryption: 1, staged_r2: 2, target_sessions: 0 }]);
      assert.deepEqual(query(databasePath, `
        SELECT r2_key FROM sync_vault_rotation_r2_objects
        WHERE user_id = '${USER_ID}' AND rotation_idempotency_key = '${IDEMPOTENCY_KEY}'
        ORDER BY r2_key
      `), [{ r2_key: PAYLOAD_R2_KEY }, { r2_key: SNAPSHOT_R2_KEY }]);
    });
  });

  it("rolls back the rotation when the recipient set changes before batch", async () => {
    await withDatabase(async (databasePath) => {
      const database = new SqliteD1Database(databasePath, raceDeviceSql());
      await assert.rejects(
        revokeDeviceDocument(
          await revocationRequest(),
          testEnv({ d1: database }),
          authContext(),
          NOW,
        ),
        (error: unknown) =>
          error instanceof DeviceConflictError && error.message === "device_revocation_race",
      );
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT current_generation FROM sync_vault_accounts
            WHERE user_id = '${USER_ID}') AS generation,
          (SELECT approval_status FROM user_devices
            WHERE user_id = '${USER_ID}' AND device_id = '${TARGET_ID}') AS target_status,
          (SELECT COUNT(*) FROM sync_vault_rotations WHERE user_id = '${USER_ID}') AS rotations,
          (SELECT COUNT(*) FROM sync_vault_envelopes
            WHERE user_id = '${USER_ID}' AND key_id = '${NEW_KEY}') AS envelopes,
          (SELECT COUNT(*) FROM audit_events WHERE user_id = '${USER_ID}') AS audits
      `), [{
        generation: 1,
        target_status: "approved",
        rotations: 0,
        envelopes: 0,
        audits: 0,
      }]);
      assert.deepEqual(query(databasePath, `
        SELECT COUNT(*) AS target_sessions FROM better_auth_session
        WHERE id = 'target-session'
      `), [{ target_sessions: 1 }]);
    });
  });

  it("revokes a pending device without changing vault or sync state", async () => {
    await withDatabase(async (databasePath) => {
      execute(databasePath, `
        UPDATE user_devices SET approval_status = 'pending', approved_at = NULL
        WHERE user_id = '${USER_ID}' AND device_id = '${TARGET_ID}';
      `);
      const database = new SqliteD1Database(databasePath);
      const document = await revokeDeviceDocument(
        await pendingRevocationRequest(),
        testEnv({ d1: database }),
        authContext(),
        NOW,
      );
      assert.equal(document.mode, "pending_revoke");
      assert.equal(document.device.approval_status, "revoked");
      assert.deepEqual(database.batches, [2]);
      assert.deepEqual(query(databasePath, `
        SELECT
          (SELECT current_generation FROM sync_vault_accounts
            WHERE user_id = '${USER_ID}') AS generation,
          (SELECT COUNT(*) FROM sync_vault_rotations WHERE user_id = '${USER_ID}') AS rotations,
          (SELECT COUNT(*) FROM pending_device_revocations
            WHERE user_id = '${USER_ID}') AS pending_revocations,
          (SELECT COUNT(*) FROM sync_objects WHERE user_id = '${USER_ID}') AS objects,
          (SELECT COUNT(*) FROM sync_snapshots WHERE user_id = '${USER_ID}') AS snapshots,
          (SELECT COUNT(*) FROM better_auth_session
            WHERE id = 'target-session') AS target_sessions
      `), [{
        generation: 1,
        rotations: 0,
        pending_revocations: 1,
        objects: 1,
        snapshots: 1,
        target_sessions: 0,
      }]);
    });
  });
});

async function revocationRequest(): Promise<Request> {
  const envelopes: ApprovedDeviceRevocationRequest["envelopes"] = [
    rotationEnvelope(APPROVER_ID, "A".repeat(43), "B".repeat(64)),
    rotationEnvelope(REMAINING_ID, `${"C".repeat(42)}E`, "D".repeat(64)),
  ];
  const unsigned: Omit<ApprovedDeviceRevocationRequest, "rotationProof"> = {
    mode: "approved_rotate",
    deviceId: TARGET_ID,
    previousKeyId: OLD_KEY,
    previousGeneration: 1,
    newKeyId: NEW_KEY,
    newGeneration: 2,
    envelopes,
    idempotencyKey: IDEMPOTENCY_KEY,
  };
  return new Request("https://elydora.test/api/devices/revoke", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      version: 2,
      mode: "approved_rotate",
      device_id: TARGET_ID,
      previous_key_id: OLD_KEY,
      previous_generation: 1,
      new_key_id: NEW_KEY,
      new_generation: 2,
      envelopes: envelopes.map((item) => ({
        recipient_device_id: item.recipientDeviceId,
        envelope: item.envelope,
      })),
      idempotency_key: IDEMPOTENCY_KEY,
      rotation_proof: await signDeviceMessage(
        deviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
      ),
    }),
  });
}

async function pendingRevocationRequest(): Promise<Request> {
  const unsigned: Omit<PendingDeviceRevocationRequest, "pendingRevocationProof"> = {
    mode: "pending_revoke",
    deviceId: TARGET_ID,
    idempotencyKey: IDEMPOTENCY_KEY,
  };
  return new Request("https://elydora.test/api/devices/revoke", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      version: 2,
      mode: "pending_revoke",
      device_id: TARGET_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      pending_revocation_proof: await signDeviceMessage(
        pendingDeviceRevocationProofBytes(USER_ID, APPROVER_ID, unsigned),
      ),
    }),
  });
}

function rotationEnvelope(
  recipientDeviceId: string,
  encappedKey: string,
  ciphertext: string,
): ApprovedDeviceRevocationRequest["envelopes"][number] {
  return {
    recipientDeviceId,
    envelope: { version: 1, suite: SUITE, encapped_key: encappedKey, ciphertext },
  };
}

function envelopeRow(
  recipientDeviceId: string,
  encappedKey: string,
  ciphertext: string,
): Record<string, unknown> {
  return {
    recipient_device_id: recipientDeviceId,
    approver_device_id: APPROVER_ID,
    key_id: NEW_KEY,
    generation: 2,
    envelope_version: 1,
    suite: SUITE,
    encapped_key: encappedKey,
    ciphertext,
    created_at: NOW,
  };
}

function authContext() {
  return {
    userId: USER_ID,
    sessionId: "session-01",
    tokenHash: "0".repeat(64),
    expiresAt: "2099-01-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    deviceId: APPROVER_ID,
  };
}

function seedSql(): string {
  return `
    INSERT INTO better_auth_user
      (id, name, email, emailVerified, createdAt, updatedAt)
    VALUES ('${USER_ID}', 'User', 'user@example.com', 1, '2026-01-01', '2026-01-01');
    INSERT INTO user_devices
      (user_id, device_id, public_key, device_name, platform, approval_status,
       created_at, approved_at, last_active_at, revoked_at, idempotency_key)
    VALUES
      ('${USER_ID}', '${APPROVER_ID}', '${PUBLIC_KEY}', 'Approver', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0001'),
      ('${USER_ID}', '${TARGET_ID}', '${PUBLIC_KEY}', 'Target', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0002'),
      ('${USER_ID}', '${REMAINING_ID}', '${PUBLIC_KEY}', 'Remaining', 'macOS', 'approved',
       10, 11, 12, NULL, 'device-register-0003');
    INSERT INTO user_device_keys
      (user_id, device_id, signing_public_key, wrapping_public_key,
       key_protocol_version, created_at)
    VALUES
      ('${USER_ID}', '${APPROVER_ID}', '${PUBLIC_KEY}', '${WRAPPING_PUBLIC_KEY}', 2, 10),
      ('${USER_ID}', '${TARGET_ID}', '${PUBLIC_KEY}', '${WRAPPING_PUBLIC_KEY}', 2, 10),
      ('${USER_ID}', '${REMAINING_ID}', '${PUBLIC_KEY}', '${WRAPPING_PUBLIC_KEY}', 2, 10);
    INSERT INTO better_auth_session
      (id, expiresAt, token, createdAt, updatedAt, userId)
    VALUES
      ('target-session', '2099-01-01', 'target-session-token',
       '2026-01-01', '2026-01-01', '${USER_ID}');
    INSERT INTO better_auth_session_device_context
      (session_id, user_id, device_id, updated_at)
    VALUES ('target-session', '${USER_ID}', '${TARGET_ID}', 15);
    INSERT INTO sync_vault_accounts
      (user_id, current_key_id, current_generation, created_at, updated_at)
    VALUES ('${USER_ID}', '${OLD_KEY}', 1, 20, 20);
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${PAYLOAD_R2_KEY}', '${USER_ID}', '${USER_HASH}', 'payload', 'pending',
      '${"1".repeat(64)}', 1000, NULL, 30, 30, NULL, NULL, NULL, NULL
    );
    INSERT INTO sync_objects
      (user_id, object_id, object_type, payload_inline, payload_r2_key, payload_hash,
       schema_rev, logical_clock, device_id, created_at, updated_at, deleted_at)
    VALUES ('${USER_ID}', 'object-01', 'bookmarks', NULL, '${PAYLOAD_R2_KEY}', '${HASH}',
      1, 1, '${APPROVER_ID}', 30, 30, NULL);
    UPDATE sync_r2_gc_candidates
    SET state = 'referenced', referenced_at = 30, updated_at = 30
    WHERE r2_key = '${PAYLOAD_R2_KEY}';
    INSERT INTO sync_r2_gc_candidates (
      r2_key, user_id, owner_hash, object_kind, state, write_token,
      lease_expires_at, gc_token, created_at, updated_at, referenced_at,
      ready_at, delete_started_at, deleted_at
    ) VALUES (
      '${SNAPSHOT_R2_KEY}', '${USER_ID}', '${USER_HASH}', 'snapshot', 'pending',
      '${"2".repeat(64)}', 1000, NULL, 40, 40, NULL, NULL, NULL, NULL
    );
    INSERT INTO sync_snapshots
      (user_id, snapshot_id, r2_key, payload_hash, schema_rev, logical_clock,
       device_id, size_bytes, created_at)
    VALUES ('${USER_ID}', 'snapshot-01', '${SNAPSHOT_R2_KEY}', '${HASH}', 1, 1,
      '${APPROVER_ID}', 64, 40);
    INSERT INTO sync_snapshot_encryption
      (user_id, snapshot_id, encryption_version, vault_generation, key_id, content_hash)
    VALUES ('${USER_ID}', 'snapshot-01', 1, 1, '${OLD_KEY}', '${HASH}');
  `;
}

function raceDeviceSql(): string {
  return `
    INSERT INTO user_devices
      (user_id, device_id, public_key, device_name, platform, approval_status,
       created_at, approved_at, last_active_at, revoked_at, idempotency_key)
    VALUES ('${USER_ID}', 'device-04', '${PUBLIC_KEY}', 'Race', 'macOS', 'approved',
      100, 101, 102, NULL, 'device-register-0004');
    INSERT INTO user_device_keys
      (user_id, device_id, signing_public_key, wrapping_public_key,
       key_protocol_version, created_at)
    VALUES ('${USER_ID}', 'device-04', '${PUBLIC_KEY}', '${WRAPPING_PUBLIC_KEY}', 2, 100);
  `;
}

async function withDatabase(run: (databasePath: string) => Promise<void>): Promise<void> {
  const tempDir = mkdtempSync(join(tmpdir(), "ely-revoke-handler-"));
  try {
    const databasePath = join(tempDir, "ely.db");
    const migrations = readdirSync(MIGRATIONS_DIR)
      .filter((name) => name.endsWith(".sql"))
      .sort()
      .map((name) => readFileSync(join(MIGRATIONS_DIR, name), "utf8"))
      .join("\n");
    execute(databasePath, migrations);
    execute(databasePath, seedSql());
    await run(databasePath);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}
