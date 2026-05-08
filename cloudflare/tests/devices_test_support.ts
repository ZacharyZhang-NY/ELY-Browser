import type {
  ElyAnalyticsDataPoint,
  ElyD1Database,
  ElyD1PreparedStatement,
  ElyR2PutOptions,
  Env,
} from "../src/bindings.js";

export const ACCESS_TOKEN = "D".repeat(48);
export const PUBLIC_KEY = "a".repeat(64);

export interface TestEnvOptions {
  auditEvents?: ElyAnalyticsDataPoint[];
  d1?: RecordedD1Database;
  kvEntries?: [string, string][];
  kvReads?: string[];
}

export interface RecordedD1Database extends ElyD1Database {
  batches: number[];
  binds: unknown[][];
  queries: string[];
}

interface TestD1DatabaseOptions {
  allRows?: unknown[];
  firstRows?: unknown[];
}

export function testEnv(options: TestEnvOptions): Env {
  const values = new Map(options.kvEntries ?? []);
  return {
    ELY_ENVIRONMENT: "local",
    ELY_DB: options.d1 ?? testD1Database([]),
    ELY_KV: {
      get(key: string): Promise<string | null> {
        options.kvReads?.push(key);
        return Promise.resolve(values.get(key) ?? null);
      },
    },
    ELY_STORAGE: testR2Bucket(),
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
  };
}

export function testD1Database(rows: unknown[] | TestD1DatabaseOptions): RecordedD1Database {
  const binds: unknown[][] = [];
  const batches: number[] = [];
  const queries: string[] = [];
  const allRows = Array.isArray(rows) ? rows : rows.allRows ?? [];
  const firstRows = Array.isArray(rows) ? rows : rows.firstRows ?? [];
  let firstIndex = 0;
  return {
    batches,
    binds,
    queries,
    prepare(query: string) {
      queries.push(query);
      return testD1PreparedStatement(allRows, firstRows, () => firstIndex++, binds);
    },
    batch(statements: ElyD1PreparedStatement[]) {
      batches.push(statements.length);
      return Promise.resolve([]);
    },
    exec() {
      return Promise.resolve({});
    },
  };
}

export function sessionDocument(deviceId = "device-01"): string {
  return JSON.stringify({
    version: 1,
    user_id: "user-01",
    session_id: "session-01",
    device_id: deviceId,
    expires_at: "2099-01-01T00:00:00.000Z",
  });
}

function testD1PreparedStatement(
  allRows: unknown[],
  firstRows: unknown[],
  nextFirstIndex: () => number,
  binds: unknown[][],
): ElyD1PreparedStatement {
  return {
    bind(...values: unknown[]) {
      binds.push(values);
      return this;
    },
    first<T>() {
      return Promise.resolve((firstRows[nextFirstIndex()] as T | undefined) ?? null);
    },
    all<T>() {
      return Promise.resolve({ results: allRows as T[] });
    },
    run() {
      return Promise.resolve({});
    },
  };
}

function testR2Bucket(): Env["ELY_STORAGE"] {
  return {
    get() {
      return Promise.resolve(null);
    },
    put(_key: string, value: ArrayBuffer, _options?: ElyR2PutOptions) {
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(value);
        },
      });
    },
  };
}
