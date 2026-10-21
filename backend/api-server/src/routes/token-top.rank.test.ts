/**
 * D5 (2026-10-21) — endpoint-level regression for GET /api/tokens/top with
 * `rank_by=volume_24h`.
 *
 * Why an endpoint test and not only the pure-function test: the operator's
 * census is filled from THIS route, so the wire contract that matters is
 * "what does the route answer for a given table state". The fake pool answers
 * by SQL shape (same harness as rejection-breakdown.test.ts) and every value
 * is a real-shaped row: raw reserves + decimals + validation volume.
 *
 * Cases:
 *   (a) rank_by=volume_24h orders by measured volume and counts (never ranks)
 *       a token whose volume is NULL;
 *   (b) rank_by default (tvl) keeps prior behaviour and normalizes raw
 *       reserves by token decimals (the ADI 6.55e32 scale defect);
 *   (c) an implausible valuation (> $1T) is excluded from every basis;
 *   (d) a bogus rank_by is a 400, not a silent fallback.
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express } from "express";
import request from "supertest";

import { buildTokenTopRouter } from "./token-top.js";

const WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const TOKA = "0x00000000000000000000000000000000000000aa";
const WEIRD = "0x00000000000000000000000000000000000000bb";
const NOLIQ = "0x00000000000000000000000000000000000000cc";

const POOLS = [
  {
    id: "aaaaaaaa-0000-4000-8000-000000000001", created_at: new Date("2026-09-01T00:00:00Z"),
    s0: "WETH", a0: WETH, r0: "LOW", st0: false, d0: 18,
    s1: "USDC", a1: USDC, r1: "LOW", st1: true, d1: 6,
  },
  {
    id: "aaaaaaaa-0000-4000-8000-000000000002", created_at: new Date("2026-08-01T00:00:00Z"),
    s0: "TOKA", a0: TOKA, r0: "UNKNOWN", st0: false, d0: 18,
    s1: "WETH", a1: WETH, r1: "LOW", st1: false, d1: 18,
  },
  {
    id: "aaaaaaaa-0000-4000-8000-000000000003", created_at: new Date("2026-07-01T00:00:00Z"),
    s0: "WEIRD", a0: WEIRD, r0: "UNKNOWN", st0: false, d0: 0,
    s1: "USDC", a1: USDC, r1: "LOW", st1: true, d1: 6,
  },
  // No row in RESERVES for this pool: a priced token with no reservable value.
  {
    id: "aaaaaaaa-0000-4000-8000-000000000004", created_at: new Date("2026-06-01T00:00:00Z"),
    s0: "NOLIQ", a0: NOLIQ, r0: "UNKNOWN", st0: false, d0: 18,
    s1: "WETH", a1: WETH, r1: "LOW", st1: false, d1: 18,
  },
];

const RESERVES = new Map<string, { reserve0: string; reserve1: string }>([
  // 100 WETH + 244,500 USDC (the decoder-correct shape)
  ["aaaaaaaa-0000-4000-8000-000000000001", { reserve0: "100000000000000000000", reserve1: "244500000000" }],
  // 1,000,000 TOKA + 0.2 WETH
  ["aaaaaaaa-0000-4000-8000-000000000002", { reserve0: "1000000000000000000000000", reserve1: "200000000000000000" }],
  // 5e15 whole units at decimals=0 → $5e15 → implausible
  ["aaaaaaaa-0000-4000-8000-000000000003", { reserve0: "5000000000000000", reserve1: "1000000000" }],
]);

const VALIDATIONS = [
  { address: WETH, liquidity_usd: "100000000", volume_24h_usd: "50000000", volume_1h_usd: "2000000", volume_5m_usd: "100000", registry_verified: true, final_status: "VERIFIED", score: 80 },
  { address: USDC, liquidity_usd: "90000000", volume_24h_usd: "80000000", volume_1h_usd: "3000000", volume_5m_usd: "200000", registry_verified: true, final_status: "VERIFIED", score: 90 },
  { address: TOKA, liquidity_usd: "400000", volume_24h_usd: null, volume_1h_usd: null, volume_5m_usd: null, registry_verified: true, final_status: "VERIFIED", score: 70 },
  { address: WEIRD, liquidity_usd: "1000", volume_24h_usd: "123", volume_1h_usd: "1", volume_5m_usd: "0", registry_verified: true, final_status: "VERIFIED", score: 60 },
];

const SAFETY = [
  { token_address: WETH, safety_score: 90 },
  { token_address: USDC, safety_score: 95 },
  { token_address: TOKA, safety_score: 70 },
  { token_address: WEIRD, safety_score: 55 },
];

function fakePool(opts: { volumeAsOf?: "present" | "absent" | "probe_fails" } = {}) {
  const mode = opts.volumeAsOf ?? "present";
  return {
    query: vi.fn(async (sql: string) => {
      // Contract §2(i)(ii): the gate probes information_schema, like S09 does.
      if (sql.includes("information_schema.columns")) {
        if (mode === "probe_fails") throw new Error("relation pg_catalog is on fire");
        return { rows: [{ present: mode === "present" ? 1 : 0 }], rowCount: 1 };
      }
      if (sql.includes("AS orderable")) {
        // 3 validation rows carry a volume with volume_as_of <= 6 h; the oldest
        // snapshot is 1 day old (outside the 6 h window, so it does not order).
        return { rows: [{ orderable: 3, p50_age_s: "900", max_age_s: "86400" }], rowCount: 1 };
      }
      if (sql.includes("FROM pools p")) return { rows: POOLS, rowCount: POOLS.length };
      if (sql.includes("unnest($1::uuid[])")) {
        const rows = [...RESERVES.entries()].map(([pool_id, r]) => ({ pool_id, ...r }));
        return { rows, rowCount: rows.length };
      }
      if (sql.includes("FROM token_validations")) return { rows: VALIDATIONS, rowCount: VALIDATIONS.length };
      if (sql.includes("FROM token_safety_cache")) return { rows: SAFETY, rowCount: SAFETY.length };
      return { rows: [], rowCount: 0 };
    }),
  };
}

function app(opts: {
  volumeAsOf?: "present" | "absent" | "probe_fails";
  prices?: Record<string, string>;
} = {}): Express {
  const a = express();
  const hgetall = vi.fn(async (_key: string) =>
    opts.prices ?? { WETH: "2500", USDC: "1", TOKA: "2", WEIRD: "300", NOLIQ: "7" });
  a.use(
    buildTokenTopRouter({
      pool: fakePool(opts) as never,
      redis: { hgetall } as never,
      logger: { warn: vi.fn(), info: vi.fn() },
    }),
  );
  return a;
}

const get = (
  qs: string,
  opts: { volumeAsOf?: "present" | "absent" | "probe_fails"; prices?: Record<string, string> } = {},
) => request(app(opts)).get(`/api/tokens/top?${qs}`);

describe("GET /api/tokens/top — rank_by (D5)", () => {
  it("orders by measured 24h volume and counts the NULL-volume token", async () => {
    const res = await get("chain_id=1&limit=100&window=24h&rank_by=volume_24h");
    expect(res.status).toBe(200);
    expect(res.body.ranked.map((r: { symbol: string }) => r.symbol)).toEqual(["USDC", "WETH"]);
    expect(res.body.excluded["no_volume_24h"]).toBe(1);
    expect(res.body.window_note).toContain("volumen 24h");
    expect(res.body.window_note).toContain("par primario por token");
    expect(res.body.window_note).toContain("frescura de volumen: measured");
    expect(res.body.volume_freshness).toEqual({
      state: "measured", p50_age_s: 900, max_age_s: 86400, reason: null,
    });
  });

  it("defaults to tvl, normalizes raw reserves by decimals, and drops the implausible row", async () => {
    const res = await get("chain_id=1&limit=100");
    expect(res.status).toBe(200);
    const wethUsdc = res.body.ranked.find((r: { symbol: string }) => r.symbol === "WETH");
    // 100 WETH × $2,500 in pool 1 + 0.2 WETH × $2,500 in pool 2 = 250,500
    // — NOT 1e20 × 2,500. The sum across pools is the point: the fixture has
    // WETH on both sides of two pools, so a per-pool-only fix would still miss.
    expect(Number(wethUsdc.tvl_usd)).toBeCloseTo(250_500, 0);
    expect(res.body.ranked.some((r: { symbol: string }) => r.symbol === "WEIRD")).toBe(false);
    expect(res.body.excluded["implausible_tvl"]).toBe(1);
  });

  it("contract §2: volume basis without a fresh snapshot is NOT COMPUTED, never a TVL fallback", async () => {
    const absent = await get("chain_id=1&limit=100&rank_by=volume_24h", { volumeAsOf: "absent" });
    expect(absent.status).toBe(200);
    expect(absent.body.selection_state).toBe("NOT COMPUTED");
    expect(absent.body.selection_reason).toBe("volume_as_of_absent");
    expect(absent.body.ranked).toEqual([]);
    expect(absent.body.window_note).toContain("NOT COMPUTED");
    expect(absent.body.window_note).toContain("no se sustituye por TVL");
    expect(absent.body.volume_freshness.state).toBe("NOT COMPUTED");
    expect(absent.body.volume_freshness.reason).toContain("volume_as_of");

    const failed = await get("chain_id=1&limit=100&rank_by=volume_24h", { volumeAsOf: "probe_fails" });
    expect(failed.body.selection_state).toBe("NOT COMPUTED");
    expect(failed.body.selection_reason).toBe("volume_as_of_probe_failed");
    expect(failed.body.ranked).toEqual([]);
  });

  it("a tvl-based request is not blocked by the volume gate and names selection_state", async () => {
    const res = await get("chain_id=1&limit=100", { volumeAsOf: "absent" });
    expect(res.status).toBe(200);
    expect(res.body.selection_state).toBe("MEASURED");
    expect(res.body.ranked.length).toBeGreaterThan(0);
  });

  it("decision 3: the write lock blocks the save whenever the selection is NOT COMPUTED", async () => {
    const blocked = await get("chain_id=1&limit=100&rank_by=volume_24h", { volumeAsOf: "absent" });
    expect(blocked.body.save_gate.allowed).toBe(false);
    expect(blocked.body.save_gate.reasons).toEqual(["selection_not_computed"]);

    const open = await get("chain_id=1&limit=100&rules=min_liquidity_usd,validated_status");
    expect(open.body.save_gate.allowed).toBe(true);
    expect(open.body.rule_population["min_liquidity_usd"].evaluated).toBeGreaterThan(0);
  });

  it("decision 6: pool_age_24h rides as NOT MEASURED and never drops a row", async () => {
    const res = await get("chain_id=1&limit=100&rules=pool_age_24h");
    expect(res.status).toBe(200);
    expect(res.body.ranked.length).toBeGreaterThan(0);
    for (const r of res.body.ranked) {
      expect(r.failed_rules).not.toContain("pool_age_24h");
      expect(r.not_measured_rules).toContain("pool_age_24h");
    }
    expect(res.body.rule_population["pool_age_24h"].diagnostic).toBe(true);
    // A diagnostic-only rule set cannot unlock the write.
    expect(res.body.save_gate.reasons).toEqual(["no_verified_rule_population"]);
  });

  it("D-SEL-2: a priced token with no reserve row is counted as no_reserve, never ranked 0.00", async () => {
    const res = await get("chain_id=1&limit=100");
    expect(res.body.ranked.some((r: { symbol: string }) => r.symbol === "NOLIQ")).toBe(false);
    expect(res.body.excluded["no_reserve"]).toBeGreaterThanOrEqual(1);
    for (const r of res.body.ranked) expect(r.tvl_usd).not.toBe("0.00");
  });

  it("decision 5: chain != 1 is NOT COMPUTED with price_tower_not_parameterized (no chain-1 prices)", async () => {
    const res = await get("chain_id=42161&limit=100");
    expect(res.status).toBe(200);
    expect(res.body.selection_state).toBe("NOT COMPUTED");
    expect(res.body.selection_reason).toBe("price_tower_not_parameterized");
    expect(res.body.ranked).toEqual([]);
    expect(res.body.save_gate.allowed).toBe(false);
    expect(res.body.window_note).toContain("no se valora con precios de chain 1");
  });

  it("decision 5: an empty tower is a missing measurement, not an empty ranking", async () => {
    const res = await get("chain_id=1&limit=100", { prices: {} });
    expect(res.status).toBe(200);
    expect(res.body.selection_state).toBe("NOT COMPUTED");
    expect(res.body.selection_reason).toBe("price_tower_empty");
    expect(res.body.ranked).toEqual([]);
  });

  it("rejects a bogus rank_by instead of silently falling back", async () => {
    const res = await get("chain_id=1&limit=20&rank_by=marketcap");
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_rank_by");
    expect(res.body.allowed).toEqual(["tvl", "volume_24h"]);
  });
});
