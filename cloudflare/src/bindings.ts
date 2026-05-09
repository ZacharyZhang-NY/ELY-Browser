export interface ElyKvNamespace {
  get(key: string): Promise<string | null>;
}

export interface ElyR2Object {
  arrayBuffer(): Promise<ArrayBuffer>;
}

export interface ElyR2PutOptions {
  httpMetadata?: { contentType?: string };
  customMetadata?: Record<string, string>;
  sha256?: ArrayBuffer;
}

export interface ElyR2Bucket {
  get(key: string): Promise<ElyR2Object | null>;
  put(key: string, value: ArrayBuffer, options?: ElyR2PutOptions): Promise<ElyR2Object>;
  delete(key: string): Promise<void>;
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
  ELY_STORAGE: ElyR2Bucket;
  ELY_RATE_LIMITER: ElyRateLimit;
  ELY_API_AUDIT: ElyAnalyticsDataset;
  ELY_ENVIRONMENT: string;
  ELY_AUTH_BASE_URL: string;
  ELY_AUTH_SECRET: string;
  ELY_AUTH_GOOGLE_CLIENT_ID?: string;
  ELY_AUTH_GOOGLE_CLIENT_SECRET?: string;
  ELY_AUTH_GITHUB_CLIENT_ID?: string;
  ELY_AUTH_GITHUB_CLIENT_SECRET?: string;
  ELY_AUTH_EMAIL_OTP_ENDPOINT?: string;
  ELY_AUTH_EMAIL_OTP_TOKEN?: string;
}
