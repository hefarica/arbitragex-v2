/**
 * paper-shadow-audit — ARBX-RDY-03 (A.5 daily ledger audit) tests.
 *
 * Mocks the pg pool (SQL/params recorded, canned rows per query — the ILIKE
 * classification itself is Postgres behavior; these tests pin the ROUTE's
 * aggregation math, the SQL contract of the documented classification rules,
 * and the R8 fail-honest semantics). Mirrors the go-no-go.test.ts harness
 * (vitest + express + supertest); never touches a real Postgres.
 *
 * Cases:
 *   (a) multi-day aggregation math: pass-through counts, green/red split,
 *       pnl per day, rate = runs/total per day
 *   (b) percentile null-safety: all-NULL latency → null p50/p95 (NOT 0)
 *   (c) reason histogram fidelity: RAW reason strings verbatim (incl. null),
 *       mapped to the right day
 *   (d) two_signal: accumulation streak over ALL days (breaks on first
 *       non-green) + scored calibration from scored_opportunities
 *   (e) empty scored table → zeros + null latest (never fabricated)
 *   (f) empty paper_trade_runs → 200 status INACTIVE with zeros
 *   (g) pool null → 503 db_unavailable
 *   (h) query error → 503 query_failed with verbatim detail
 *   (i) days clamp: 999→90, 0/abc/negative→14 default, 7.9→7
 *   (j) classification SQL contract: the documented ILIKE rules are present,
 *       window + chain parameterized ($1/$2), no value interpolated
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express } from "express";
import request from "supertest";
import type pg from "pg";
import { mountPaperShadowAudit } from "./paper-shadow-audit.js";

const logger = { warn: vi.fn() };

const DAY_A = new Date("2026-08-27T00:00:00.000Z");
const DAY_B = new Date("2026-08-26T00:00:00.000Z");

// ---------------------------------------------------------------------------
// Mock pool — canned rows keyed off SQL shape (recorded for contract asserts).
// ---------------------------------------------------------------------------

interface RecordedQuery {
  sql: string;
  params: unknown[];
}

interface MockWorld {
  daily?: Array<{
    day: Date;
    total_runs: string;
    latency_ms_p50: string | null;
    latency_ms_p95: string | null;
    sim_error_runs: string;
    sim_error_unclassified_runs: string;
    reverted_runs: string;
    green_runs: string;
    red_runs: string;
    pnl_day_usd: string;
  }>;
  reasons?: Array<{ day: Date; reason: string | null; runs: string }>;
  totals?: { total_trades: string; last_trade_at: Date | null };
  streak?: Array<{ day_pnl: string }>;
  scored?: { scored_total: string; scored_last_7d: string; scored_latest_at: Date | null };
  failOn?: "daily" | "totals" | "scored";
}

function makeMockPool(world: MockWorld): { pool: pg.Pool; queries: RecordedQuery[] } {
  const queries: RecordedQuery[] = [];
  const pool = {
    query: async (sql: string, params?: unknown[]): Promise<{ rows: Record<string, unknown>[] }> => {
      const s = sql.trim();
      queries.push({ sql: s, params: params ?? [] });

      // (1) Per-day audit aggregates.
      if (s.includes("FROM paper_trade_runs") && s.includes("percentile_cont")) {
        if (world.failOn === "daily") throw new Error("daily exploded");
        return { rows: (world.daily ?? []) as unknown as Record<string, unknown>[] };
      }

      // (2) Per-day RAW reason histogram.
      if (s.includes("FROM paper_trade_runs") && s.includes("GROUP BY date_trunc('day', created_at), reason")) {
        return { rows: (world.reasons ?? []) as unknown as Record<string, unknown>[] };
      }

      // (3) Accumulation totals.
      if (s.includes("FROM paper_trade_runs") && s.includes("total_trades")) {
        if (world.failOn === "totals") throw new Error("totals exploded");
        return {
          rows: [world.totals ?? { total_trades: "0", last_trade_at: null }] as unknown as Record<string, unknown>[],
        };
      }

      // (4) Consecutive-green-day streak (per-day PnL, most recent first).
      if (s.includes("FROM paper_trade_runs") && s.includes("day_pnl")) {
        return { rows: (world.streak ?? []) as unknown as Record<string, unknown>[] };
      }

      // (5) Calibration signal on scored_opportunities.
      if (s.includes("FROM scored_opportunities")) {
        if (world.failOn === "scored") throw new Error("scored exploded");
        return {
          rows: [
            world.scored ?? { scored_total: "0", scored_last_7d: "0", scored_latest_at: null },
          ] as unknown as Record<string, unknown>[],
        };
      }

      throw new Error("unmocked query: " + s.slice(0, 80));
    },
  };
  return { pool: pool as unknown as pg.Pool, queries };
}

function buildApp(pool: pg.Pool | null): Express {
  const app = express();
  app.use(express.json());
  mountPaperShadowAudit(app, { pool, logger });
  return app;
}

// A two-day world with distinct math per day:
//   DAY_A: 100 runs, 2 sim-error (1 simulation_failed:* + 1 revm_reverted:* in
//          the histogram), 1 other rejection reason, latency recorded.
//   DAY_B: 4 runs, all latency NULL (pre-LATLED-01 rows), red day.
function twoDayWorld(): MockWorld {
  return {
    daily: [
      {
        day: DAY_A,
        total_runs: "100",
        latency_ms_p50: "12.5",
        latency_ms_p95: "80",
        sim_error_runs: "2",
        sim_error_unclassified_runs: "1",
        reverted_runs: "1",
        green_runs: "60",
        red_runs: "40",
        pnl_day_usd: "15.5",
      },
      {
        day: DAY_B,
        total_runs: "4",
        latency_ms_p50: null,
        latency_ms_p95: null,
        sim_error_runs: "0",
        sim_error_unclassified_runs: "0",
        reverted_runs: "0",
        green_runs: "0",
        red_runs: "4",
        pnl_day_usd: "-2.25",
      },
    ],
    reasons: [
      { day: DAY_A, reason: "simulation_failed:SimulationFailed", runs: "1" },
      { day: DAY_A, reason: "revm_reverted:InsufficientProfit", runs: "1" },
      { day: DAY_A, reason: "gas_floor_breach", runs: "1" },
      { day: DAY_A, reason: null, runs: "97" },
      { day: DAY_B, reason: null, runs: "4" },
    ],
    totals: { total_trades: "104", last_trade_at: new Date("2026-08-27T18:00:00.000Z") },
    streak: [{ day_pnl: "15.5" }, { day_pnl: "-2.25" }],
    scored: { scored_total: "12", scored_last_7d: "5", scored_latest_at: new Date("2026-08-26T09:30:00.000Z") },
  };
}

// ---------------------------------------------------------------------------

describe("paper-shadow-audit — GET /api/v1/metrics/paper-shadow/daily-audit", () => {
  it("(a) multi-day aggregation math + rates per day", async () => {
    const { pool } = makeMockPool(twoDayWorld());
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);
    expect(res.body.audit.chain_id).toBe(1);
    expect(res.body.audit.days).toBe(14);
    expect(res.body.audit.status).toBe("ACTIVE");

    const daily = res.body.audit.daily;
    expect(daily).toHaveLength(2);

    const a = daily[0];
    expect(a.day).toBe(DAY_A.toISOString());
    expect(a.total_runs).toBe(100);
    expect(a.green_runs).toBe(60);
    expect(a.red_runs).toBe(40);
    expect(a.pnl_day_usd).toBe(15.5);
    expect(a.sim_error_runs).toBe(2);
    expect(a.sim_error_rate).toBeCloseTo(0.02, 12);
    expect(a.reverted_runs).toBe(1);
    expect(a.reverted_rate).toBeCloseTo(0.01, 12);
    expect(a.sim_error_unclassified_runs).toBe(1);

    const b = daily[1];
    expect(b.day).toBe(DAY_B.toISOString());
    expect(b.total_runs).toBe(4);
    expect(b.sim_error_rate).toBe(0);
    expect(b.reverted_rate).toBe(0);
    expect(b.pnl_day_usd).toBe(-2.25);
  });

  it("(b) all-NULL latency day → null p50/p95 (not 0, not fabricated)", async () => {
    const { pool } = makeMockPool(twoDayWorld());
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);
    const b = res.body.audit.daily[1];
    expect(b.latency_ms_p50).toBeNull();
    expect(b.latency_ms_p95).toBeNull();
    // Mixed day keeps its numeric percentiles.
    expect(res.body.audit.daily[0].latency_ms_p50).toBe(12.5);
    expect(res.body.audit.daily[0].latency_ms_p95).toBe(80);
  });

  it("(c) reason histogram: RAW strings verbatim (incl. null), mapped to the right day", async () => {
    const { pool } = makeMockPool(twoDayWorld());
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);

    const histA = res.body.audit.daily[0].reason_histogram;
    expect(histA).toEqual([
      { reason: "simulation_failed:SimulationFailed", runs: 1 },
      { reason: "revm_reverted:InsufficientProfit", runs: 1 },
      { reason: "gas_floor_breach", runs: 1 },
      { reason: null, runs: 97 },
    ]);
    const histB = res.body.audit.daily[1].reason_histogram;
    expect(histB).toEqual([{ reason: null, runs: 4 }]);
  });

  it("(d) two_signal: streak over ALL days breaks on first non-green + calibration surfaced", async () => {
    const { pool } = makeMockPool(twoDayWorld());
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);

    const ts = res.body.two_signal;
    expect(ts.accumulation_signal).toEqual({
      consecutive_green_days: 1, // 15.5 green, then -2.25 breaks the streak
      total_trades: 104,
      last_trade_at: "2026-08-27T18:00:00.000Z",
    });
    expect(ts.calibration_signal).toEqual({
      scored_opportunities_total: 12,
      scored_last_7d: 5,
      scored_latest_at: "2026-08-26T09:30:00.000Z",
    });
    expect(ts.note).toContain("BOTH signals");
    expect(ts.note).toContain("A.8");
  });

  it("(d2) streak counts consecutive green days beyond the window query", async () => {
    const world = twoDayWorld();
    world.streak = [{ day_pnl: "10" }, { day_pnl: "20.5" }, { day_pnl: "-1" }];
    const { pool } = makeMockPool(world);
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);
    expect(res.body.two_signal.accumulation_signal.consecutive_green_days).toBe(2);
  });

  it("(e) empty scored_opportunities → zeros + null latest (honest, never fabricated)", async () => {
    const world = twoDayWorld();
    world.scored = { scored_total: "0", scored_last_7d: "0", scored_latest_at: null };
    const { pool } = makeMockPool(world);
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);
    expect(res.body.two_signal.calibration_signal).toEqual({
      scored_opportunities_total: 0,
      scored_last_7d: 0,
      scored_latest_at: null,
    });
  });

  it("(f) empty paper_trade_runs → 200, status INACTIVE, zeros, null last_trade_at", async () => {
    const { pool } = makeMockPool({});
    const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200);
    expect(res.body.audit.status).toBe("INACTIVE");
    expect(res.body.audit.daily).toEqual([]);
    expect(res.body.two_signal.accumulation_signal).toEqual({
      consecutive_green_days: 0,
      total_trades: 0,
      last_trade_at: null,
    });
  });

  it("(g) pool null → 503 db_unavailable", async () => {
    const res = await request(buildApp(null)).get("/api/v1/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
  });

  it("(h) query error → 503 query_failed with verbatim detail", async () => {
    for (const failOn of ["daily", "totals", "scored"] as const) {
      const { pool } = makeMockPool({ ...twoDayWorld(), failOn });
      const res = await request(buildApp(pool)).get("/api/v1/metrics/paper-shadow/daily-audit");
      expect(res.status).toBe(503);
      expect(res.body.error).toBe("query_failed");
      expect(res.body.detail).toContain(`${failOn} exploded`);
    }
  });

  it("(i) days clamp: 999→90, 0/negative/abc→default 14, 7.9→7 (echoed + parameterized)", async () => {
    const { pool, queries } = makeMockPool(twoDayWorld());
    const app = buildApp(pool);

    const over = await request(app).get("/api/v1/metrics/paper-shadow/daily-audit?days=999");
    expect(over.status).toBe(200);
    expect(over.body.audit.days).toBe(90);

    for (const bad of ["0", "-5", "abc"]) {
      const res = await request(app).get(`/api/v1/metrics/paper-shadow/daily-audit?days=${bad}`);
      expect(res.status).toBe(200);
      expect(res.body.audit.days).toBe(14);
    }

    const frac = await request(app).get("/api/v1/metrics/paper-shadow/daily-audit?days=7.9");
    expect(frac.status).toBe(200);
    expect(frac.body.audit.days).toBe(7);

    // The effective window is parameterized into the daily queries ($2), never
    // interpolated: every paper_trade_runs window query used [chain, days].
    const windowQueries = queries.filter(
      (q) => q.sql.includes("FROM paper_trade_runs") && q.sql.includes("make_interval"),
    );
    expect(windowQueries.length).toBeGreaterThanOrEqual(6); // 7 requests × 2 window queries
    for (const q of windowQueries) {
      expect(q.params).toHaveLength(2);
      expect(q.params[0]).toBe(1);
      expect([90, 14, 7]).toContain(q.params[1]);
      expect(q.sql).not.toContain("make_interval(days => 999");
    }
  });

  it("(j) classification SQL contract: documented ILIKE rules present + parameterized", async () => {
    const { pool, queries } = makeMockPool(twoDayWorld());
    const res = await request(buildApp(pool)).get("/api/metrics/paper-shadow/daily-audit");
    expect(res.status).toBe(200); // /api/ dual path resolves to the same handler

    const daily = queries.find((q) => q.sql.includes("percentile_cont"))!;
    expect(daily.sql).toContain("reason ILIKE '%simulation_failed%'");
    expect(daily.sql).toContain("reason ILIKE 'revm_reverted%'");
    expect(daily.sql).toContain("reason ILIKE '%revert%'");
    expect(daily.sql).toContain("reason IS NOT NULL");
    expect(daily.sql).toContain("percentile_cont(0.50) WITHIN GROUP (ORDER BY execution_time_ms)");
    expect(daily.sql).toContain("percentile_cont(0.95) WITHIN GROUP (ORDER BY execution_time_ms)");
    // Same honest PnL expression as paper-shadow-metrics.ts.
    expect(daily.sql).toContain("COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)");
    // Parameterized: chain + days only.
    expect(daily.params).toEqual([1, 14]);

    const hist = queries.find((q) => q.sql.includes("GROUP BY date_trunc('day', created_at), reason"))!;
    // RAW reason strings only — no classification in the histogram SQL.
    expect(hist.sql).not.toContain("ILIKE");
    expect(hist.params).toEqual([1, 14]);

    const scored = queries.find((q) => q.sql.includes("FROM scored_opportunities"))!;
    expect(scored.sql).toContain("INTERVAL '7 days'");
    expect(scored.params).toEqual([]);
  });
});
