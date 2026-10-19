// backend/api-server/src/routes/route-discovery-outcomes-api.test.ts
//
// FE-0038 (§47) — contract test for the outcomes summary groupings.
// The router takes a pg.Pool via DI, so the "database" here is a stub that
// answers per-SQL with verbatim rows; the assertions pin the ROUTE contract
// (shape, clamp, fail-honest 503s), never SQL semantics — those live in
// Postgres. RULE 00: rows pass through untouched (the '(null)' folding for
// empty cartridge_id is SQL-side NULLIF/COALESCE; the stub returns exactly
// what the DB would).
//
// RDO-SUMMARY-503: the summary now reads the 5-minute rollup for complete
// buckets + raw for the two window edges. The stub keys on the same stable
// SQL markers (dim = '<dim>' / sum(total) / AS missing / the maintenance
// INSERT), and rows come back keyed as the merge query returns them
// ({key, n, opportunities}) — the route's rename/split is part of the
// contract under test.
import { describe, expect, it, vi } from "vitest";
import express, { type Request, type Response } from "express";
import request from "supertest";
import type pg from "pg";

import {
  buildRouteDiscoveryOutcomes,
  buildRouteDiscoveryOutcomesRouter,
  ROLLUP_MAINTAIN_INTERVAL_MS,
  ROLLUP_MAINTAIN_WINDOW_HOURS,
} from "./route-discovery-outcomes-api.js";

/** One canned answer per query the summary runs, keyed by SQL fragment. */
function stubPool(overrides: Record<string, pg.QueryResult> = {}): pg.Pool {
  const answer = (rows: Record<string, unknown>[]): pg.QueryResult =>
    ({ rows, rowCount: rows.length }) as pg.QueryResult;
  const canned: Array<[string, pg.QueryResult]> = [
    // Rollup maintenance INSERT (idempotent top-up) — no rows back.
    ["INSERT INTO route_discovery_outcome_rollup_5m", answer([])],
    // Coverage check over the window's complete buckets.
    ["AS missing", answer([{ missing: 0, oldest_missing: "0" }])],
    // Totals merge (rollup ∪ raw head ∪ raw tail).
    ["sum(total)::bigint AS total", answer([{ total: "1200", opportunities: "3" }])],
    // Groupings — merge rows are {key, n, opportunities}; the route renames.
    ["dim = 'reason'", answer([{ key: "v3_sizing_pending", n: "700", opportunities: "2" }])],
    ["dim = 'chain'", answer([{ key: "1", n: "1200", opportunities: "3" }])],
    ["dim = 'cartridge'", answer([{ key: "MEV-01-015", n: "400", opportunities: "1" }])],
    [
      "dim = 'pair'",
      answer([
        {
          key: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2|0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
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
  it("serves totals + the five groupings verbatim (merge rows renamed, pairs split)", async () => {
    const res = await request(appFor(stubPool())).get("/api/v1/route-discovery-outcomes/summary?hours=24");
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    expect(res.body.source).toBe("postgres");
    expect(res.body.window_hours).toBe(24);
    expect(res.body.data.totals).toEqual({ total: "1200", opportunities: "3" });
    expect(res.body.data.by_reason).toEqual([{ reason: "v3_sizing_pending", n: "700", opportunities: "2" }]);
    expect(res.body.data.by_chain).toEqual([{ chain_id: "1", n: "1200", opportunities: "3" }]);
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
    // Rollup provenance metadata (diagnosable, never fabricated counts).
    expect(res.body.rollup.bucket_ms).toBe(300000);
    expect(res.body.rollup.served_buckets).toBeGreaterThan(0);
  });

  it("clamps the window to [1, 336] hours", async () => {
    const res = await request(appFor(stubPool())).get("/api/v1/route-discovery-outcomes/summary?hours=9999");
    expect(res.status).toBe(200);
    expect(res.body.window_hours).toBe(336);
  });

  it("empty groupings come back as empty arrays — an honest zero-row window, not a fabrication", async () => {
    const res = await request(
      appFor(stubPool({ "dim = 'cartridge'": { rows: [], rowCount: 0 } as pg.QueryResult })),
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

  // RDO-SUMMARY-503: a window the rollup does not yet cover must NOT be served
  // undercounted — same honest 503 family, with recovery detail (the top-up
  // converges across polls and the next request succeeds).
  it("rollup coverage gap → honest 503 rollup_backfilling with recovery detail", async () => {
    const res = await request(
      appFor(
        stubPool({
          "AS missing": { rows: [{ missing: 7, oldest_missing: "1756800000000" }], rowCount: 1 } as pg.QueryResult,
        }),
      ),
    ).get("/api/v1/route-discovery-outcomes/summary?hours=24");
    expect(res.status).toBe(503);
    expect(res.body).toMatchObject({
      ok: false,
      reason: "rollup_backfilling",
      source: "postgres",
      data: null,
    });
    expect(res.body.detail.missing_buckets).toBe(7);
    expect(res.body.detail.oldest_missing_ms).toBe("1756800000000");
    expect(res.body.detail.buckets_per_request).toBe(24);
    expect(res.body.detail.retry_after_s).toBe(30);
  });

  // The maintenance INSERT is idempotent by contract — concurrent pollers may
  // run it simultaneously; ON CONFLICT DO NOTHING keeps that a no-op instead
  // of a 23505 surfaced as query_failed.
  it("maintenance INSERT carries ON CONFLICT DO NOTHING (concurrent-poller idempotence)", async () => {
    const seen: string[] = [];
    const recording = {
      connect: async () => ({
        query: async (sql: string) => {
          if (sql === "BEGIN" || sql.startsWith("SET LOCAL") || sql === "COMMIT" || sql === "ROLLBACK")
            return { rows: [], rowCount: 0 };
          seen.push(sql);
          if (sql.includes("AS missing")) return { rows: [{ missing: 0, oldest_missing: "0" }], rowCount: 1 };
          if (sql.includes("sum(total)::bigint AS total"))
            return { rows: [{ total: "0", opportunities: "0" }], rowCount: 1 };
          if (sql.includes("dim = '")) return { rows: [], rowCount: 0 };
          return { rows: [], rowCount: 0 };
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    const res = await request(appFor(recording)).get("/api/v1/route-discovery-outcomes/summary");
    expect(res.status).toBe(200);
    const insert = seen.find((s) => s.includes("INSERT INTO route_discovery_outcome_rollup_5m"));
    expect(insert).toBeDefined();
    expect(insert).toMatch(/ON CONFLICT DO NOTHING/);
    // And the top-up only ever targets COMPLETE buckets (below the current one).
    expect(insert).toMatch(/floor\(extract\(epoch FROM now\(\)\) \* 1000\)::bigint\s*\/ 300000 \* 300000 - 300000/);
  });

  it("RDO-SUMMARY-503 runtime closure: chains/cartridges distinct reads ONLY the head/tail edges — never a full-window raw scan (count DISTINCT over ~68M rows blows the 15s statement timeout -> 503 query_failed)", async () => {
    // Production evidence 2026-09-02 (ddcacff7, 82.6M-row table): the two
    // count(DISTINCT) subqueries scanned `ts_ms >= $3` (= since = window
    // start) on the RAW table. The rollup serves the middle of the window;
    // raw must only ever read [since, firstAligned) and [tailFrom, now) —
    // the exact ranges rawh/rawt use. Pin every raw $3 reference to be
    // upper-bounded by $4 so this can never regress to a full-window scan.
    const seen: string[] = [];
    const recording = {
      connect: async () => ({
        query: async (sql: string) => {
          if (sql === "BEGIN" || sql.startsWith("SET LOCAL") || sql === "COMMIT" || sql === "ROLLBACK")
            return { rows: [], rowCount: 0 };
          seen.push(sql);
          if (sql.includes("AS missing")) return { rows: [{ missing: 0, oldest_missing: "0" }], rowCount: 1 };
          if (sql.includes("sum(total)::bigint AS total"))
            return { rows: [{ total: "0", opportunities: "0" }], rowCount: 1 };
          if (sql.includes("dim = '")) return { rows: [], rowCount: 0 };
          return { rows: [], rowCount: 0 };
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    const res = await request(appFor(recording)).get("/api/v1/route-discovery-outcomes/summary?hours=24");
    expect(res.status).toBe(200);
    const totals = seen.find((s) => s.includes("sum(total)::bigint AS total"));
    expect(totals).toBeDefined();
    // rawh + chains + cartridges: 3 edge reads [$3, $4), every one bounded.
    const occurrences = totals!.split("ts_ms >= $3").length - 1;
    expect(occurrences).toBeGreaterThanOrEqual(3);
    const unbounded = totals!.split("ts_ms >= $3").slice(1).filter((frag) => !frag.startsWith(" AND ts_ms < $4"));
    expect(unbounded, "raw scan of `ts_ms >= $3` without the $4 upper bound = full-window scan (the timeout)").toEqual([]);
    // Tail edges present for both dims ([$5, now) reads).
    expect(totals!.match(/route_discovery_outcomes WHERE ts_ms >= \$5/g)?.length).toBeGreaterThanOrEqual(3);
  });
});

// RDO-503-CONVERGE (2026-10-19): the operator panel showed "Outcomes series
// unavailable" while the backend was healthy. Root cause measured in this
// route: the top-up was ONE 24-bucket statement ordered strictly oldest-first,
// so (a) a batch crossing the 15s statement timeout was rolled back whole —
// zero progress, no convergence — and (b) every poll spent its buckets on the
// oldest gaps while the 24h window the panel reads stayed gapped (503). The
// fix: the requested window's buckets go first, and the work commits in small
// batches so a slow batch costs at most its own size.
describe("route-discovery-outcomes top-up — RDO-503-CONVERGE", () => {
  function convergeStub(insertRowCount: (call: number) => number | "throw") {
    const inserts: Array<{ sql: string; params: unknown[] }> = [];
    let call = 0;
    const pool = {
      connect: async () => ({
        query: async (sql: string, params: unknown[] = []) => {
          if (sql === "BEGIN" || sql.startsWith("SET LOCAL") || sql === "COMMIT" || sql === "ROLLBACK")
            return { rows: [], rowCount: 0 };
          if (sql.includes("INSERT INTO route_discovery_outcome_rollup_5m")) {
            call += 1;
            inserts.push({ sql, params });
            const n = insertRowCount(call);
            if (n === "throw") throw new Error("canceling statement due to statement timeout");
            return { rows: [], rowCount: n };
          }
          if (sql.includes("AS missing")) return { rows: [{ missing: 0, oldest_missing: "0" }], rowCount: 1 };
          if (sql.includes("sum(total)::bigint AS total"))
            return { rows: [{ total: "0", opportunities: "0" }], rowCount: 1 };
          if (sql.includes("dim = '")) return { rows: [], rowCount: 0 };
          return { rows: [], rowCount: 0 };
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    return { pool, inserts };
  }

  it("fills the REQUESTED window's buckets first, in committed batches (not one all-or-nothing statement)", async () => {
    // Every batch reports a full insert, so the top-up keeps going until the
    // per-request bucket budget is spent.
    const { pool, inserts } = convergeStub(() => 6);
    const res = await request(appFor(pool)).get("/api/v1/route-discovery-outcomes/summary?hours=24");

    expect(res.status).toBe(200);
    // Batched: more than one statement, so a later failure cannot erase the
    // earlier committed buckets.
    expect(inserts.length).toBeGreaterThan(1);
    // Each statement is bounded well under the 24-bucket request budget.
    expect(inserts[0].params[2]).toBe(6);
    // And it is told WHICH window to prioritise (old code passed []).
    expect(inserts[0].params[0]).toEqual(expect.any(Number));
    expect(inserts[0].params[1]).toEqual(expect.any(Number));
    expect(Number(inserts[0].params[1])).toBeGreaterThan(Number(inserts[0].params[0]));
    // The window-first ordering clause is the mechanism under test.
    expect(inserts[0].sql).toMatch(
      /ORDER BY \(g\.bucket >= \$1::bigint AND g\.bucket <= \$2::bigint\) DESC, g\.bucket ASC/,
    );
    // 24 buckets / 6 per statement = 4 statements.
    expect(inserts.length).toBe(4);
  });

  it("a batch that dies keeps the committed progress and lets the coverage check decide (never a blanket query_failed)", async () => {
    // Batch 1 dies (statement timeout). The old single-statement top-up turned
    // this into 503 query_failed even when the window was already covered.
    const { pool, inserts } = convergeStub(() => "throw");
    const res = await request(appFor(pool)).get("/api/v1/route-discovery-outcomes/summary?hours=24");
    expect(inserts.length).toBe(1); // it does not retry the same dead batch
    expect(res.status).toBe(200); // coverage check said the window is covered
  });
});

// RDO-503-CONVERGE hardening: the top-up only ran inside a read. With the
// operator panel closed (most of the time) no read happens, so the rollup fell
// behind and the next visitor paid the whole backlog inside one request — the
// "series unavailable" the owner saw on a healthy backend. The maintainer
// closes that gap without any reader.
describe("route-discovery-outcomes rollup maintainer — RDO-503-CONVERGE", () => {
  function convergeStub(insertRowCount: (call: number) => number | "throw") {
    const inserts: Array<{ sql: string; params: unknown[] }> = [];
    const pool = {
      connect: async () => ({
        query: async (sql: string, params: unknown[] = []) => {
          if (sql === "BEGIN" || sql.startsWith("SET LOCAL") || sql === "COMMIT" || sql === "ROLLBACK")
            return { rows: [], rowCount: 0 };
          if (sql.includes("INSERT INTO route_discovery_outcome_rollup_5m")) {
            inserts.push({ sql, params });
            return { rows: [], rowCount: insertRowCount(inserts.length) };
          }
          return { rows: [], rowCount: 0 };
        },
        release: () => {},
      }),
    } as unknown as pg.Pool;
    return { pool, inserts };
  }

  it("keeps the rollup current on its interval with no reader at all", async () => {
    vi.useFakeTimers();
    try {
      const { pool, inserts } = convergeStub(() => 6);
      const wiring = buildRouteDiscoveryOutcomes(pool, { maintain: true });
      expect(inserts.length).toBe(0); // interval-only: nothing at build time

      await vi.advanceTimersByTimeAsync(ROLLUP_MAINTAIN_INTERVAL_MS);
      expect(inserts.length).toBeGreaterThan(0);
      // It maintains the panels' default 24h window (minus the in-progress
      // bucket and the ceil() alignment of the head edge), not an arbitrary
      // range.
      const from = Number(inserts[0].params[0]);
      const to = Number(inserts[0].params[1]);
      const span = to - from;
      expect(span).toBeLessThan(ROLLUP_MAINTAIN_WINDOW_HOURS * 3600_000);
      expect(span).toBeGreaterThanOrEqual(ROLLUP_MAINTAIN_WINDOW_HOURS * 3600_000 - 2 * 300_000);

      wiring.maintainer.stop();
      const stopped = inserts.length;
      await vi.advanceTimersByTimeAsync(ROLLUP_MAINTAIN_INTERVAL_MS * 3);
      expect(inserts.length).toBe(stopped); // stopped means stopped
    } finally {
      vi.useRealTimers();
    }
  });

  it("no pool → no timer (and it must not throw)", async () => {
    vi.useFakeTimers();
    try {
      const wiring = buildRouteDiscoveryOutcomes(null, { maintain: true });
      expect(typeof wiring.maintainer.stop).toBe("function");
      await vi.advanceTimersByTimeAsync(ROLLUP_MAINTAIN_INTERVAL_MS * 3);
      wiring.maintainer.stop();
    } finally {
      vi.useRealTimers();
    }
  });

  it("the router-only entry point never starts a timer (back-compat)", async () => {
    vi.useFakeTimers();
    try {
      const { pool, inserts } = convergeStub(() => 6);
      buildRouteDiscoveryOutcomesRouter(pool);
      await vi.advanceTimersByTimeAsync(ROLLUP_MAINTAIN_INTERVAL_MS * 3);
      expect(inserts.length).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
