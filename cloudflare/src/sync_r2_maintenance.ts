import type { Env } from "./bindings.js";
import { purgeLegacySessionCache } from "./legacy_auth_kv_cleanup.js";
import { collectSyncR2Garbage } from "./sync_r2_gc.js";
import { inventorySyncR2Objects } from "./sync_r2_inventory.js";
import { finalizeCleanedVaultRotations } from "./sync_vault_rotation_cleanup.js";

export async function maintainSyncR2Storage(env: Env, nowSeconds: number): Promise<void> {
  const errors: unknown[] = [];
  for (const task of [
    () => purgeLegacySessionCache(env),
    () => inventorySyncR2Objects(env, nowSeconds),
    () => collectSyncR2Garbage(env, nowSeconds, { limit: 100 }),
    () => finalizeCleanedVaultRotations(env, nowSeconds),
  ]) {
    try {
      await task();
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length > 0) throw new AggregateError(errors, "sync_storage_maintenance_failed");
}
