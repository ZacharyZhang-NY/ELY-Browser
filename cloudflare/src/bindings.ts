export interface ElyKvNamespace {
  get(key: string): Promise<string | null>;
}

export interface Env {
  ELY_KV: ElyKvNamespace;
  ELY_ENVIRONMENT: string;
}
