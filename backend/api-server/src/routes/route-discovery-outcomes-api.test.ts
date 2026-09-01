// backend/api-server/src/routes/route-discovery-outcomes-api.test.ts
//
// FE-0038 (§47) — contract test for the outcomes summary groupings.
// The router takes a pg.Pool via DI, so the "database" here is a stub that
// answers per-SQL with verbatim rows; the assertions pin the ROUTE contract
// (shape, clamp, fail-honest 503s), never SQL semantics — those live in
// Postgres. RULE 00: rows pass through untouched (the '(null)' folding for
// empty cartridge_id is SQL-side NULLIF/COALESCE; the stub returns exactly
// what the DB would).
import { describe, expect, it } from "vitest";
import express, { type Request, type Response } from "express";
import request from "supertest";
import type pg from "pg";

import { buildRouteDiscoveryOutcomesRouter } from "./route-discovery-outcomes-api.js";

/** One canned answer per aggregate the summary runs, keyed by SQL fragment. */
function stubPool(overrides: Record<string, pg.QueryResult> = {}): pg.Pool {
  const answer = (rows: Record<string, unknown>[]): pg.QueryResult =>
    ({ rows, rowCount: rows.length }) as pg.QueryResult;
  const canned: Array<[string, pg.QueryResult]> = [
    ["count(*)::bigint AS total", answer([{ total: "1200", opportunities: "3" }])],
    ["AS reason, count(*)", answer([{ reason: "v3_sizing_pending", n: "700" }])],
    ["chain_id, count(*)", answer([{ chain_id: 1, n: "1200", opportunities: "3" }])],
    [
      "AS cartridge_id, count(*)",
      answer([{ cartridge_id: "MEV-01-015", n: "400", opportunities: "1" }]),
    ],
    [
      "token_in, token_out, count(*)",
      answer([
        {
          token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
          token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
          n: "250",
          opportunities: "1",
        },
      ]),
    ],
  ];
  return {
    // RDO-SUMMARY-HANG: the route checks out one client per aggregate
    // (BEGIN + SET LOCAL statement_timeout + query + COMMIT), so the stub
    // exposes connect() the same way pg.Pool does.
    connect: async () => ({
      query: async (sql: string) => {
        if (sql === "BEGIN" || sql.startsWith("SET LOCAL") || sql === "COMMIT") return answer([]);
        if (sql === "ROLLBACK") return answer([]);
        for (const [frag, res] of canned) if (sql.includes(frag)) return overrides[frag] ?? res;
        return overrides["*"] ?? answer([]);
      },
      release: () => {},
    }),
  } as unknown as pg.Pool;
}

function appFor(pool: pg.Pool | null) {
  const app = express();
  app.use(buildRouteDiscoveryOutcomesRouter(pool));
  return app;
}

describe("route-discovery-outcomes summary — FE-0038 §47 groupings", () => {
  it("serves totals + the five groupings verbatim (rows pass through untouched)", async () => {
    const res = await request(appFor(stubPool())).get("/api/v1/route-discovery-outcomes/summary?hours=24");
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    expect(res.body.source).toBe("postgres");
    expect(res.body.window_hours).toBe(24);
    expect(res.body.data.totals).toEqual({ total: "1200", opportunities: "3" });
    expect(res.body.data.by_reason).toEqual([{ reason: "v3_sizing_pending", n: "700" }]);
    expect(res.body.data.by_chain).toEqual([{ chain_id: 1, n: "1200", opportunities: "3" }]);
    expect(res.body.data.by_cartridge).toEqual([
      { cartridge_id: "MEV-01-015", n: "400", opportunities: "1" },
    ]);
    expect(res.body.data.by_pair).toEqual([
      {
        token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        n: "250",
        opportunities: "1",
      },
    ]);
  });

  it("clamps the window to [1, 336] hours", async () => {
    const res = await request(appFor(stubPool())).get("/api/v1/route-discovery-outcomes/summary?hours=9999");
    expect(res.status).toBe(200);
    expect(res.body.window_hours).toBe(336);
  });

  it("empty groupings come back as empty arrays — an honest zero-row window, not a fabrication", async () => {
    const res = await request(
      appFor(stubPool({ "AS cartridge_id, count(*)": { rows: [], rowCount: 0 } as pg.QueryResult })),
    ).get("/api/v1/route-discovery-outcomes/summary");
    expect(res.body.data.by_cartridge).toEqual([]);
    expect(res.body.data.by_pair.length).toBe(1); // the other grouping is intact
  });

  it("pool null → 503 with the verbatim reason (R8 fail-honest, never a fake empty)", async () => {
    const res = await request(appFor(null)).get("/api/v1/route-discovery-outcomes/summary");
    expect(res.status).toBe(503);
    expect(res.body).toMatchObject({ ok: false, reason: "db_unavailable", data: null });
  });

  it("query failure → 503 query_failed with the error message", async () => {
    const failing = {
      connect: async () => ({
        query: async (sql: string) => {
          if (sql === "ROLLBACK") return { rows: [], rowCount: 0 };
          throw new Error("relation does not exist");
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    const res = await request(appFor(failing)).get("/api/v1/route-discovery-outcomes/summary");
    expect(res.status).toBe(503);
    expect(res.body).toMatchObject({ ok: false, reason: "query_failed" });
    expect(res.body.error).toContain("relation does not exist");
  });

  // RDO-SUMMARY-HANG: a window too heavy to aggregate inside the per-statement
  // budget must surface as the SAME honest 503 — never a hang, never a stacked
  // aggregation, never a fabricated zero.
  it("statement timeout → honest 503 query_failed (Postgres cancel wording)", async () => {
    const slow = {
      connect: async () => ({
        query: async (sql: string) => {
          if (sql === "BEGIN" || sql.startsWith("SET LOCAL")) return { rows: [], rowCount: 0 };
          if (sql === "ROLLBACK") return { rows: [], rowCount: 0 };
          throw new Error("canceling statement due to statement timeout");
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    const res = await request(appFor(slow)).get("/api/v1/route-discovery-outcomes/summary?hours=336");
    expect(res.status).toBe(503);
    expect(res.body).toMatchObject({ ok: false, reason: "query_failed" });
    expect(res.body.error).toContain("statement timeout");
  });
});
