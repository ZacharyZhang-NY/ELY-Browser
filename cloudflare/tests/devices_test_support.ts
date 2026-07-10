import type {
  ElyAnalyticsDataPoint,
  ElyD1Database,
  ElyD1PreparedStatement,
  ElyR2PutOptions,
  Env,
} from "../src/bindings.js";
import { deviceRegistrationProofBytes } from "../src/device_registration_proof.js";

export const ACCESS_TOKEN = "D".repeat(48);
export const PUBLIC_KEY = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
export const WRAPPING_PUBLIC_KEY = "b".repeat(64);
const SIGNING_PRIVATE_KEY_PKCS8 =
  "302e020100300506032b657004220420" +
  "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

export interface TestEnvOptions {
  auditEvents?: ElyAnalyticsDataPoint[];
  diagnosticEvents?: ElyAnalyticsDataPoint[];
  d1?: RecordedD1Database;
  kvEntries?: [string, string][];
  kvDeletes?: string[];
  kvPuts?: [string, string][];
  kvReads?: string[];
  r2Deletes?: string[];
  r2Gets?: string[];
  r2Objects?: [string, ArrayBuffer][];
  r2Puts?: RecordedR2Put[];
}

export interface RecordedD1Database extends ElyD1Database {
  authBinds: unknown[][];
  authQueries: string[];
  batches: number[];
  binds: unknown[][];
  queries: string[];
  sessionConstraints?: string[];
}

export interface RecordedR2Put {
  key: string;
  payload: ArrayBuffer;
  options: ElyR2PutOptions;
}

export async function deviceRegistrationBody(
  overrides: Record<string, unknown> = {},
): Promise<Record<string, unknown>> {
  const body: Record<string, unknown> = {
    version: 2,
    device_id: "device-01",
    public_key: PUBLIC_KEY,
    wrapping_public_key: WRAPPING_PUBLIC_KEY,
    device_name: "MacBook Pro",
    platform: "macOS",
    idempotency_key: "device-register-0001",
    ...overrides,
  };
  body.registration_proof = await signDeviceMessage(
    deviceRegistrationProofBytes({
      deviceId: stringValue(body.device_id),
      publicKey: stringValue(body.public_key),
      wrappingPublicKey: stringValue(body.wrapping_public_key),
      deviceName: stringValue(body.device_name),
      platform: stringValue(body.platform),
      idempotencyKey: stringValue(body.idempotency_key),
    }),
  );
  return body;
}

export async function signDeviceMessage(message: Uint8Array): Promise<string> {
  const privateKey = await crypto.subtle.importKey(
    "pkcs8",
    hexBytes(SIGNING_PRIVATE_KEY_PKCS8),
    { name: "Ed25519" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    { name: "Ed25519" },
    privateKey,
    message,
  );
  return hexString(new Uint8Array(signature));
}

interface TestD1DatabaseOptions {
  allRows?: unknown[];
  allRowSets?: unknown[][];
  batchChanges?: number[][];
  batchError?: Error;
  batchRowSets?: unknown[][][];
  firstRows?: unknown[];
  runError?: Error;
  runChanges?: number[];
  sessionRow?: unknown | null;
}

const DEFAULT_AUTH_SESSION_ROW = {
  id: "session-01",
  userId: "user-01",
  expiresAt: "2099-01-01T00:00:00.000Z",
  createdAt: new Date().toISOString(),
  deviceId: "device-01",
};

export function testEnv(options: TestEnvOptions): Env {
  const values = new Map(options.kvEntries ?? []);
  return {
    ELY_ENVIRONMENT: "local",
    ELY_AUTH_BASE_URL: "https://elydora.test",
    ELY_AUTH_SECRET: "test-auth-secret-for-worker-routes",
    ELY_DB: options.d1 ?? testD1Database([]),
    ELY_KV: {
      get(key: string): Promise<string | null> {
        options.kvReads?.push(key);
        return Promise.resolve(values.get(key) ?? null);
      },
      put(key: string, value: string): Promise<void> {
        options.kvPuts?.push([key, value]);
        values.set(key, value);
        return Promise.resolve();
      },
      delete(key: string): Promise<void> {
        options.kvDeletes?.push(key);
        values.delete(key);
        return Promise.resolve();
      },
    },
    ELY_STORAGE: testR2Bucket(
      options.r2Puts,
      options.r2Objects,
      options.r2Gets,
      options.r2Deletes,
    ),
    ELY_RATE_LIMITER: {
      limit(): Promise<{ success: boolean }> {
        return Promise.resolve({ success: true });
      },
    },
    ELY_API_AUDIT: {
      writeDataPoint(event?: ElyAnalyticsDataPoint): void {
        if (event !== undefined) {
          options.auditEvents?.push(event);
        }
      },
    },
    ELY_DIAGNOSTICS: {
      writeDataPoint(event?: ElyAnalyticsDataPoint): void {
        if (event !== undefined) {
          options.diagnosticEvents?.push(event);
        }
      },
    },
  };
}

export function testD1Database(rows: unknown[] | TestD1DatabaseOptions): RecordedD1Database {
  const authBinds: unknown[][] = [];
  const authQueries: string[] = [];
  const binds: unknown[][] = [];
  const batches: number[] = [];
  const queries: string[] = [];
  const sessionConstraints: string[] = [];
  const allRows = Array.isArray(rows) ? rows : rows.allRows ?? [];
  const firstRows = Array.isArray(rows) ? rows : rows.firstRows ?? [];
  const sessionRow =
    !Array.isArray(rows) && Object.hasOwn(rows, "sessionRow")
      ? rows.sessionRow ?? null
      : DEFAULT_AUTH_SESSION_ROW;
  let firstIndex = 0;
  let allIndex = 0;
  let batchIndex = 0;
  let runIndex = 0;
  const database: RecordedD1Database = {
    authBinds,
    authQueries,
    batches,
    binds,
    queries,
    sessionConstraints,
    prepare(query: string) {
      const isAuthSessionQuery = query.includes("WHERE session.token = ?");
      (isAuthSessionQuery ? authQueries : queries).push(query);
      return testD1PreparedStatement(
        () => (!Array.isArray(rows) ? rows.allRowSets?.[allIndex++] : undefined) ?? allRows,
        firstRows,
        () => firstIndex++,
        isAuthSessionQuery ? authBinds : binds,
        isAuthSessionQuery,
        sessionRow,
        () => (!Array.isArray(rows) ? rows.runChanges?.[runIndex++] : undefined) ?? 1,
        !Array.isArray(rows) ? rows.runError : undefined,
      );
    },
    batch<T>(statements: ElyD1PreparedStatement[]) {
      batches.push(statements.length);
      if (!Array.isArray(rows) && rows.batchError !== undefined) {
        return Promise.reject(rows.batchError);
      }
      const currentBatchIndex = batchIndex++;
      const configuredChanges = !Array.isArray(rows)
        ? rows.batchChanges?.[currentBatchIndex]
        : undefined;
      const configuredRows = !Array.isArray(rows)
        ? rows.batchRowSets?.[currentBatchIndex]
        : undefined;
      const results = statements.map((_, index) => ({
        results: configuredRows?.[index] ?? [],
        meta: { changes: configuredChanges?.[index] ?? 1 },
      }));
      return Promise.resolve(results as T[]);
    },
    exec() {
      return Promise.resolve({});
    },
    withSession(constraint) {
      sessionConstraints.push(constraint);
      return database;
    },
  };
  return database;
}

export function sessionDocument(deviceId: string | null = "device-01"): string {
  return JSON.stringify({
    version: 1,
    user_id: "user-01",
    session_id: "session-01",
    ...(deviceId === null ? {} : { device_id: deviceId }),
    expires_at: "2099-01-01T00:00:00.000Z",
  });
}

function testD1PreparedStatement(
  allRows: () => unknown[],
  firstRows: unknown[],
  nextFirstIndex: () => number,
  binds: unknown[][],
  isAuthSessionQuery: boolean,
  sessionRow: unknown | null,
  nextRunChanges: () => number,
  runError: Error | undefined,
): ElyD1PreparedStatement {
  return {
    bind(...values: unknown[]) {
      binds.push(values);
      return this;
    },
    first<T>() {
      if (isAuthSessionQuery) {
        return Promise.resolve(sessionRow as T | null);
      }
      return Promise.resolve((firstRows[nextFirstIndex()] as T | undefined) ?? null);
    },
    all<T>() {
      return Promise.resolve({ results: allRows() as T[] });
    },
    run() {
      if (runError !== undefined) {
        return Promise.reject(runError);
      }
      return Promise.resolve({ results: [], meta: { changes: nextRunChanges() } });
    },
  };
}

function testR2Bucket(
  puts: RecordedR2Put[] = [],
  objects: [string, ArrayBuffer][] = [],
  gets: string[] = [],
  deletes: string[] = [],
): Env["ELY_STORAGE"] {
  const values = new Map(objects);
  return {
    get(key: string) {
      gets.push(key);
      const value = values.get(key);
      if (value === undefined) {
        return Promise.resolve(null);
      }
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(value);
        },
      });
    },
    put(key: string, value: ArrayBuffer, options: ElyR2PutOptions = {}) {
      puts.push({ key, payload: value, options });
      values.set(key, value);
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(value);
        },
      });
    },
    delete(key: string) {
      deletes.push(key);
      values.delete(key);
      return Promise.resolve();
    },
  };
}

function stringValue(value: unknown): string {
  if (typeof value !== "string") {
    throw new TypeError("registration fixture field must be a string");
  }
  return value;
}

function hexBytes(value: string): Uint8Array {
  const bytes = new Uint8Array(value.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function hexString(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
