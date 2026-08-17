/**
 * RunFullSyncCycle FASE 3 — unit tests for the PG→Redis credential projection.
 * Fake Redis/Pool record calls; no live services.
 *
 * Guards the trading-config mirror contract:
 *   - mirrorCredential SETs arbx:svc_cred:<provider>:<scope> with the EXACT
 *     projected shape and publishes the reload channel.
 *   - unmirrorCredential DELs + publishes a tombstone.
 *   - rehydrateSvcCredMirror replays every row WITH a secret, skips empty
 *     ones, never throws, and logs the summary.
 *   - A Redis failure is logged + returns false — the admin write path never
 *     breaks (PG stays the source of truth).
 */
import { describe, it, expect } from "vitest";
import type { Pool } from "pg";

import {
  mirrorCredential,
  unmirrorCredential,
  rehydrateSvcCredMirror,
  svcCredKey,
  SVC_CRED_CHANNEL,
  SVC_CRED_KEY_PREFIX,
  type ProjectedCredential,
} from "./projection.js";

class FakeRedis {
  store = new Map<string, string>();
  publishes: Array<{ channel: string; message: string }> = [];
  failSet = false;
  async set(key: string, value: string): Promise<"OK"> {
    if (this.failSet) throw new Error("redis down");
    this.store.set(key, value);
    return "OK";
  }
  async del(key: string): Promise<number> {
    return this.store.delete(key) ? 1 : 0;
  }
  async publish(channel: string, message: string): Promise<number> {
    this.publishes.push({ channel, message });
    return 1;
  }
}

const logger = {
  warn: [] as Array<Record<string, unknown>>,
  info: [] as Array<Record<string, unknown>>,
};
const trackLogger = {
  warn: (o: object) => logger.warn.push(o as Record<string, unknown>),
  info: (o: object) => logger.info.push(o as Record<string, unknown>),
};

const row: ProjectedCredential = {
  provider: "coingecko_pro",
  scope: "global",
  secret_value: "CG-test-value",
  metadata: { _validation: { message: null, providers: [] } },
  status: "valid",
  updated_at: "2026-08-17T00:00:00.000Z",
  updated_by: "operator:macro",
};

describe("mirrorCredential", () => {
  it("SETs the exact projection shape and publishes the reload channel", async () => {
    const redis = new FakeRedis();
    await mirrorCredential({ redis: redis as never, logger: trackLogger }, row);
    const key = svcCredKey("coingecko_pro", "global");
    expect(key).toBe(`${SVC_CRED_KEY_PREFIX}coingecko_pro:global`);
    expect(redis.store.get(key)).toBe(JSON.stringify(row));
    expect(redis.publishes).toHaveLength(1);
    expect(redis.publishes[0]!.channel).toBe(SVC_CRED_CHANNEL);
    expect(redis.publishes[0]!.message).toBe(JSON.stringify(row));
  });

  it("returns false + warn on Redis failure — NEVER throws (write path safe)", async () => {
    const redis = new FakeRedis();
    redis.failSet = true;
    const before = logger.warn.length;
    const ok = await mirrorCredential({ redis: redis as never, logger: trackLogger }, row);
    expect(ok).toBe(false);
    expect(logger.warn.length).toBe(before + 1);
    expect(logger.warn.at(-1)!.event).toBe("credentials.projection_mirror_failed");
  });
});

describe("unmirrorCredential", () => {
  it("DELs the key and publishes a tombstone", async () => {
    const redis = new FakeRedis();
    redis.store.set(svcCredKey("coingecko_pro", "global"), "{}");
    await unmirrorCredential({ redis: redis as never, logger: trackLogger }, "coingecko_pro", "global");
    expect(redis.store.size).toBe(0);
    expect(redis.publishes[0]!.message).toContain('"deleted":true');
  });
});

describe("rehydrateSvcCredMirror (boot)", () => {
  function fakePool(rows: Array<Record<string, unknown>>) {
    return { query: async () => ({ rows }) } as unknown as Pool;
  }

  it("replays every row WITH a secret; empty-secret rows are filtered by the query contract", async () => {
    const redis = new FakeRedis();
    const pool = fakePool([
      { provider: "rpc_http", scope: "chain:1", secret_value: "a=b,c=d", metadata: {}, status: "valid", updated_at: new Date(), updated_by: "admin" },
      { provider: "github_token", scope: "global", secret_value: "ghp_x", metadata: {}, status: "valid", updated_at: new Date(), updated_by: "admin" },
    ]);
    await rehydrateSvcCredMirror({ pool, redis: redis as never, logger: trackLogger });
    expect(redis.store.size).toBe(2);
    expect(redis.publishes).toHaveLength(2);
    const summary = logger.info.at(-1)!;
    expect(summary.event).toBe("credentials.projection_rehydrated");
    expect(summary.mirrored).toBe(2);
  });

  it("skips (warn) when DB is unavailable and never throws", async () => {
    const redis = new FakeRedis();
    const before = logger.warn.length;
    await rehydrateSvcCredMirror({ pool: null, redis: redis as never, logger: trackLogger });
    expect(logger.warn.length).toBe(before + 1);
    expect(logger.warn.at(-1)!.event).toBe("credentials.projection_rehydrate_skipped");
    expect(redis.store.size).toBe(0);
  });
});
