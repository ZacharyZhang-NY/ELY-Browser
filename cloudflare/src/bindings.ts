export interface ElyKvNamespace {
  get(key: string): Promise<string | null>;
}

export interface ElyD1PreparedStatement {
  bind(...values: unknown[]): ElyD1PreparedStatement;
  first<T = unknown>(): Promise<T | null>;
  all<T = unknown>(): Promise<{ results: T[] }>;
  run(): Promise<unknown>;
}

export interface ElyD1Database {
  prepare(query: string): ElyD1PreparedStatement;
  batch<T = unknown>(statements: ElyD1PreparedStatement[]): Promise<T[]>;
  exec(query: string): Promise<unknown>;
}

export interface ElyRateLimit {
  limit(options: { key: string }): Promise<{ success: boolean }>;
}

export interface ElyAnalyticsDataPoint {
  indexes?: (ArrayBuffer | string | null)[];
  doubles?: number[];
  blobs?: (ArrayBuffer | string | null)[];
}

export interface ElyAnalyticsDataset {
  writeDataPoint(event?: ElyAnalyticsDataPoint): void;
}

export interface Env {
  ELY_DB: ElyD1Database;
  ELY_KV: ElyKvNamespace;
  ELY_RATE_LIMITER: ElyRateLimit;
  ELY_API_AUDIT: ElyAnalyticsDataset;
  ELY_ENVIRONMENT: string;
}
