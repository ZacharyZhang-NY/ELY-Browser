import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import type { AuthContext } from "../src/auth.js";
import {
  authSessionCacheKvKey,
  authTokenHash,
  deleteAuthenticatedSession,
} from "../src/auth.js";
import { handleRequest } from "../src/index.js";
import {
  ACCESS_TOKEN,
  PUBLIC_KEY,
  testD1Database,
  testEnv,
} from "./devices_test_support.js";
import { SqliteD1Database, execute, query } from "./sqlite_d1_test_support.js";

const USER_ID = "user-01";
const DEVICE_ID = "device-01";
const SESSION_ID = "session-01";
const SIBLING_SESSION_ID = "session-02";
const SIBLING_TOKEN = "S".repeat(48);
const MIGRATIONS_DIR = join(process.cwd(), "migrations");

describe("session logout routes", () => {
  it("deletes the exact authenticated session and legacy cache key", async () => {
    const d1 = testD1Database({ runChanges: [1] });
    const kvDeletes: string[] = [];
    const response = await handleRequest(logoutRequest(), testEnv({ d1, kvDeletes }));

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { version: 1, signed_out: true });
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(d1.queries.length, 1);
    assert.match(d1.queries[0] ?? "", /DELETE FROM better_auth_session/);
    assert.deepEqual(d1.binds, [[SESSION_ID, USER_ID, ACCESS_TOKEN]]);
    assert.deepEqual(kvDeletes, [
      authSessionCacheKvKey("local", await authTokenHash(ACCESS_TOKEN)),
    ]);
    assert.deepEqual(d1.sessionConstraints, ["first-primary", "first-primary"]);
  });

  it("treats a concurrent exact deletion as signed out", async () => {
    const response = await handleRequest(
      logoutRequest(),
      testEnv({ d1: testD1Database({ runChanges: [0] }) }),
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { version: 1, signed_out: true });
  });

  it("maps D1 deletion failures to a stable server error", async () => {
    const response = await handleRequest(
      logoutRequest(),
      testEnv({ d1: testD1Database({ runError: new Error("d1 unavailable") }) }),
    );

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "session_logout_failed" });
  });

  it("keeps authoritative logout successful when legacy KV cleanup fails", async () => {
    const env = testEnv({ d1: testD1Database({ runChanges: [1] }) });
    env.ELY_KV.delete = () => Promise.reject(new Error("kv unavailable"));

    const response = await handleRequest(logoutRequest(), env);

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { version: 1, signed_out: true });
  });

  it("rejects unsupported methods before authentication", async () => {
    const d1 = testD1Database([]);
    const response = await handleRequest(
      new Request("https://elydora.test/api/session/logout"),
      testEnv({ d1 }),
    );

    assert.equal(response.status, 405);
    assert.equal(response.headers.get("allow"), "POST");
    assert.deepEqual(d1.authQueries, []);
  });

  it("cascades only the current session in real SQLite", async () => {
    await withDatabase(async (databasePath, env) => {
      const response = await handleRequest(logoutRequest(), env);

      assert.equal(response.status, 200);
      assert.deepEqual(query(databasePath, `
        SELECT id, token FROM better_auth_session ORDER BY id
      `), [{ id: SIBLING_SESSION_ID, token: SIBLING_TOKEN }]);
      assert.deepEqual(query(databasePath, `
        SELECT session_id FROM better_auth_session_device_context ORDER BY session_id
      `), [{ session_id: SIBLING_SESSION_ID }]);
      assert.deepEqual(query(databasePath, "SELECT challenge_id FROM device_rebind_challenges"), []);

      const rejected = await handleRequest(
        new Request("https://elydora.test/api/devices", {
          headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
        }),
        env,
      );
      assert.equal(rejected.status, 401);
      assert.deepEqual(await rejected.json(), { error: "session_not_found" });
    });
  });

  it("preserves a replacement token when an authenticated context becomes stale", async () => {
    await withDatabase(async (databasePath, env) => {
      const replacement = "R".repeat(48);
      execute(databasePath, `
        UPDATE better_auth_session SET token = '${replacement}' WHERE id = '${SESSION_ID}';
      `);

      const document = await deleteAuthenticatedSession(logoutRequest(), env, authContext());

      assert.deepEqual(document, { version: 1, signed_out: true });
      assert.deepEqual(query(databasePath, `
        SELECT id, token FROM better_auth_session ORDER BY id
      `), [
        { id: SESSION_ID, token: replacement },
        { id: SIBLING_SESSION_ID, token: SIBLING_TOKEN },
      ]);
    });
  });
});

function logoutRequest(): Request {
  return new Request("https://elydora.test/api/session/logout", {
    method: "POST",
    headers: { authorization: `Bearer ${ACCESS_TOKEN}` },
  });
}

function authContext(): AuthContext {
  return {
    userId: USER_ID,
    sessionId: SESSION_ID,
    tokenHash: "1".repeat(64),
    expiresAt: "2099-01-01T00:00:00.000Z",
    createdAt: "2026-01-01T00:00:00.000Z",
    deviceId: DEVICE_ID,
  };
}

async function withDatabase(
  action: (databasePath: string, env: ReturnType<typeof testEnv>) => Promise<void>,
): Promise<void> {
  const directory = mkdtempSync(join(tmpdir(), "ely-session-logout-"));
  const databasePath = join(directory, "logout.sqlite");
  try {
    for (const migration of readdirSync(MIGRATIONS_DIR).filter((name) => name.endsWith(".sql")).sort()) {
      execute(databasePath, readFileSync(join(MIGRATIONS_DIR, migration), "utf8"));
    }
    seedDatabase(databasePath);
    const env = testEnv({ d1: new SqliteD1Database(databasePath) });
    await action(databasePath, env);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function seedDatabase(databasePath: string): void {
  execute(databasePath, `
    INSERT INTO better_auth_user (
      id, name, email, emailVerified, createdAt, updatedAt
    ) VALUES (
      '${USER_ID}', 'ELY User', 'user@example.com', 1,
      '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
    );
    INSERT INTO user_devices (
      user_id, device_id, public_key, device_name, platform, approval_status,
      created_at, approved_at, last_active_at, revoked_at, idempotency_key
    ) VALUES (
      '${USER_ID}', '${DEVICE_ID}', '${PUBLIC_KEY}', 'MacBook Pro', 'macos', 'approved',
      1, 1, 1, NULL, 'device-register-01'
    );
    INSERT INTO user_device_keys (
      user_id, device_id, signing_public_key, wrapping_public_key,
      key_protocol_version, created_at
    ) VALUES (
      '${USER_ID}', '${DEVICE_ID}', '${PUBLIC_KEY}', '${"b".repeat(64)}', 2, 1
    );
    INSERT INTO better_auth_session (
      id, expiresAt, token, createdAt, updatedAt, userId
    ) VALUES
      ('${SESSION_ID}', '2099-01-01T00:00:00.000Z', '${ACCESS_TOKEN}',
       '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', '${USER_ID}'),
      ('${SIBLING_SESSION_ID}', '2099-01-01T00:00:00.000Z', '${SIBLING_TOKEN}',
       '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', '${USER_ID}');
    INSERT INTO better_auth_session_device_context (
      session_id, user_id, device_id, updated_at
    ) VALUES
      ('${SESSION_ID}', '${USER_ID}', '${DEVICE_ID}', 1),
      ('${SIBLING_SESSION_ID}', '${USER_ID}', '${DEVICE_ID}', 1);
    INSERT INTO device_rebind_challenges (
      challenge_id, user_id, session_id, device_id, challenge,
      created_at, expires_at, consumed_at, consumption_nonce
    ) VALUES (
      'challenge-01', '${USER_ID}', '${SESSION_ID}', '${DEVICE_ID}', '${"c".repeat(64)}',
      1, 2, NULL, NULL
    );
  `);
}
