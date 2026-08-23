/**
 * viable-kpis tests — XLS-DASH-01 (workbook 29_SUPER_DASHBOARD KPI set).
 *
 * Pins the hard invariants of GET /api/v1/analytics/viable-kpis:
 *   (a) 503 db_unavailable when the PG pool is null (R8 fail-honest),
 *   (b) by_hops groups over jsonb_array_length(route_metadata->'pool_addresses')
 *       for the VIABLE statuses only — verbatim from the shared TS mirror,
 *   (c) viability_pct is null when total = 0 (R8: not computed, never 0%),
 *   (d) a transient query error degrades to 503 query_failed, never a crash,
 *   (e) hours is clamped server-side (1..336).
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it } from "vitest";
import type pg from "pg";

import { buildViableKpisRouter } from "./viable-kpis.js";

type QueryResult = { rows: Record<string, unknown>[]; rowCount?: number };

function fakePool(behavior: (sql: string) => QueryResult): pg.Pool {
  return { query: async (sql: string) => behavior(sql) } as unknown as pg.Pool;
}

function buildApp(pool: pg.Pool | null): Express {
  const app = express();
  app.use(buildViableKpisRouter(pool));
  return app;
}

const TOTALS = { viable: 7, routed: 5, total: 40 };

function cannedPool(): pg.Pool {
  return fakePool((sql) => {
    if (sql.includes("FILTER")) return { rows: [TOTALS] };
    if (sql.includes("jsonb_array_length")) return { rows: [{ hops: 2, n: 3 }, { hops: 3, n: 2 }] };
    if (sql.includes("strategy_kind")) {
      return { rows: [{ strategy_kind: "mev_01_001_dex_dex_arbitrage", n: 4 }] };
    }
    throw new Error("unexpected SQL in test: " + sql.slice(0, 60));
  });
}

describe("GET /api/v1/analytics/viable-kpis (XLS-DASH-01)", () => {
  it("returns 503 db_unavailable when the pool is null (fail-honest)", async () => {
    const res = await request(buildApp(null)).get("/api/v1/analytics/viable-kpis");
    expect(res.status).toBe(503);
    expect(res.body.reason).toBe("db_unavailable");
  });

  it("serves by_hops from route_metadata pool_addresses and by_kind grouped", async () => {
    const res = await request(buildApp(cannedPool())).get("/api/v1/analytics/viable-kpis?hours=12");
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    expect(res.body.window_hours).toBe(12);
    // viability 7/40 = 17.5% — computed from the real counters.
    expect(res.body.data.totals).toEqual({ viable: 7, routed: 5, total: 40, viability_pct: 17.5 });
    expect(res.body.data.by_hops).toEqual([
      { hops: 2, n: 3 },
      { hops: 3, n: 2 },
    ]);
    expect(res.body.data.by_kind[0].strategy_kind).toBe("mev_01_001_dex_dex_arbitrage");
  });

  it("viability_pct is null when nothing was observed (R8 — never 0%)", async () => {
    const pool = fakePool((sql) => {
      if (sql.includes("FILTER")) return { rows: [{ viable: 0, routed: 0, total: 0 }] };
      return { rows: [] };
    });
    const res = await request(buildApp(pool)).get("/api/v1/analytics/viable-kpis");
    expect(res.status).toBe(200);
    expect(res.body.data.totals.viability_pct).toBeNull();
    expect(res.body.data.by_hops).toEqual([]);
  });

  it("a transient query error degrades to 503 query_failed, no crash", async () => {
    const pool = fakePool(() => {
      throw new Error("ECONNRESET");
    });
    const res = await request(buildApp(pool)).get("/api/v1/analytics/viable-kpis");
    expect(res.status).toBe(503);
    expect(res.body.reason).toBe("query_failed");
    expect(res.body.error).toContain("ECONNRESET");
  });

  it("clamps hours into 1..336", async () => {
    const res = await request(buildApp(cannedPool())).get("/api/v1/analytics/viable-kpis?hours=9999");
    expect(res.status).toBe(200);
    expect(res.body.window_hours).toBe(336);
  });
});
