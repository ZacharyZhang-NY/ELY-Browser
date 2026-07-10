import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Env } from "../src/bindings.js";
import { purgeLegacySessionCache } from "../src/legacy_auth_kv_cleanup.js";

describe("legacy auth KV cleanup", () => {
  it("purges KV-only historical session keys across list pages", async () => {
    const prefix = "ely:production:auth_session_cache:";
    const first = `${prefix}${"a".repeat(64)}`;
    const second = `${prefix}${"b".repeat(64)}`;
    const unrelated = "ely:production:public_cache:plugins";
    const kv = new PaginatedKv([first, second, unrelated]);
    const env = { ELY_KV: kv, ELY_ENVIRONMENT: "production" } as unknown as Env;

    assert.equal(await purgeLegacySessionCache(env), 2);

    assert.deepEqual(kv.deleted, [first, second]);
    assert.deepEqual([...kv.values], [unrelated]);
    assert.deepEqual(kv.cursors, [undefined, "page-2"]);
  });
});

class PaginatedKv {
  readonly values: Set<string>;
  readonly deleted: string[] = [];
  readonly cursors: (string | undefined)[] = [];

  constructor(keys: string[]) {
    this.values = new Set(keys);
  }

  get(key: string): Promise<string | null> {
    return Promise.resolve(this.values.has(key) ? "value" : null);
  }

  put(key: string): Promise<void> {
    this.values.add(key);
    return Promise.resolve();
  }

  delete(key: string): Promise<void> {
    this.deleted.push(key);
    this.values.delete(key);
    return Promise.resolve();
  }

  list(options: { prefix: string; cursor?: string; limit: number }) {
    this.cursors.push(options.cursor);
    const matching = [...this.values].filter((key) => key.startsWith(options.prefix)).sort();
    if (options.cursor === undefined) {
      return Promise.resolve({
        keys: matching.slice(0, 1).map((name) => ({ name })),
        list_complete: false as const,
        cursor: "page-2",
      });
    }
    return Promise.resolve({
      keys: matching.map((name) => ({ name })),
      list_complete: true as const,
    });
  }
}
