export interface ElyKvNamespace {
  get(key: string): Promise<string | null>;
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
  ELY_KV: ElyKvNamespace;
  ELY_RATE_LIMITER: ElyRateLimit;
  ELY_API_AUDIT: ElyAnalyticsDataset;
  ELY_ENVIRONMENT: string;
}
