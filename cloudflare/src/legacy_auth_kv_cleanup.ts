import { authSessionCacheKvKey, authTokenHash } from "./auth.js";
import type { Env } from "./bindings.js";

const LIST_LIMIT = 1000;
const MAX_LIST_PAGES = 10;

interface KvListResult {
  keys: { name: string }[];
  list_complete: boolean;
  cursor?: string;
}

interface ListableKv {
  list(options: { prefix: string; cursor?: string; limit: number }): Promise<KvListResult>;
}

export class LegacyAuthKvCleanupError extends Error {}

export async function deleteLegacySessionKeys(
  env: Env,
  tokens: string[],
  currentTokenHash: string,
): Promise<number> {
  const keys = new Set<string>([
    authSessionCacheKvKey(env.ELY_ENVIRONMENT, currentTokenHash),
  ]);
  for (const token of tokens) {
    keys.add(authSessionCacheKvKey(env.ELY_ENVIRONMENT, await authTokenHash(token)));
  }
  let deleted = 0;
  for (const key of keys) {
    if (await env.ELY_KV.get(key) === null) continue;
    await env.ELY_KV.delete(key);
    deleted += 1;
  }
  return deleted;
}

export async function purgeLegacySessionCache(
  env: Env,
  maxPages = MAX_LIST_PAGES,
): Promise<number> {
  const namespace = env.ELY_KV as Env["ELY_KV"] & Partial<ListableKv>;
  if (typeof namespace.list !== "function") {
    throw new LegacyAuthKvCleanupError("legacy_auth_kv_list_unavailable");
  }
  const prefix = authSessionCacheKvKey(env.ELY_ENVIRONMENT, "0".repeat(64)).slice(0, -64);
  let cursor: string | undefined;
  let deleted = 0;
  for (let page = 0; page < maxPages; page += 1) {
    const result = await namespace.list({
      prefix,
      ...(cursor === undefined ? {} : { cursor }),
      limit: LIST_LIMIT,
    });
    for (const key of result.keys) {
      if (!key.name.startsWith(prefix)) {
        throw new LegacyAuthKvCleanupError("legacy_auth_kv_key_invalid");
      }
      await namespace.delete(key.name);
      deleted += 1;
    }
    if (result.list_complete) break;
    if (typeof result.cursor !== "string" || result.cursor.length === 0) {
      throw new LegacyAuthKvCleanupError("legacy_auth_kv_cursor_invalid");
    }
    cursor = result.cursor;
  }
  return deleted;
}
