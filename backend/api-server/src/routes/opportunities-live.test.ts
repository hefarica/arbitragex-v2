/**
 * WO-H4 (2026-09-07) — regression tests for the window_total contract of
 * GET /api/v1/opportunities/live.
 *
 * BROWSE-Auditor-R8 H-4: the home statcard "Asimetrías detectadas" showed the
 * fetch-window length (limit=50) under a subtext implying a count of the
 * arbx:opps:detected stream. The fix exposes the REAL window total —
 * window_total = COUNT(*) OVER () on the LIVE_QUERY (window functions evaluate
 * before LIMIT, so it counts every row matching the time-window + viable_only
 * WHERE even when only the top-N rows are returned).
 *
 * Harness mirrors rejection-breakdown.test.ts (vitest + express + supertest +
 * a fake pg pool answering by SQL shape). Fixture rows carry chain_id=0 — NOT
 * in CHAIN_ID_TO_DEXSCREENER_SLUG (liquidityReality.ts:95) — so the background
 * token-validation kicked off by readTokenValidationsBatch short-circuits
 * WITHOUT any network I/O (validateLiquidityReality returns ran:false early).
 *
 * Cases:
 *   (a) pool null → 503 db_unavailable (pre-existing contract, unchanged)
 *   (b) window_total flows from the row's COUNT-over-window value — NOT from
 *       items.length — and does not leak into the per-item wire shape
 *   (c) empty window → window_total 0 (computed-and-exactly-zero, R8)
 *   (d) query fails → 503 query_failed (pre-existing contract, unchanged)
 *   (e) LIVE_QUERY pins the pre-LIMIT count expression (guards against the
 *       column being dropped from the SQL while the fixture keeps injecting it)
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express } from "express";
import request from "supertest";

const logger = { warn: vi.fn() };

function fixtureRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "00000000-0000-4000-8000-000000000001",
    chain_id: 0,
    strategy_kind: "dex_arb",
    dex_a: "uniswap_v2",
    dex_b: "sushiswap",
    pair_symbol: "WETH/USDC",
    token_in: "0x0000000000000000000000000000000000000001",
    token_out: "0x0000000000000000000000000000000000000002",
    amount_in_wei: "1000000000000000000",
    expected_profit_usd: 1.5,
    net_expected_profit_usd: 0.5,
    roi_pct: 0.05,
    risk_score: null,
    rejection_reason: null,
    block_number: 1000,
    status: "detected",
    detected_at: new Date("2026-09-07T00:00:00Z"),
    trace_id: "trace-1",
    cartridge_id: null,
    chain_id_out: null,
    bridge: null,
    bridge_fee_usd: null,
    route_metadata: null,
    token_in_symbol: "WETH",
    token_in_decimals: 18,
    token_in_logo_url: null,
    token_in_resolved_via: "db",
    token_out_symbol: "USDC",
    token_out_decimals: 6,
    token_out_logo_url: null,
    token_out_resolved_via: "db",
    // What real PG computes via (COUNT(*) OVER ())::int — e.g. 137 detections
    // in the window while only the top-2 are returned.
    window_total: 137,
    ...overrides,
  };
}

function fakePool(opts: { rows?: Array<Record<string, unknown>>; fail?: boolean }) {
  const query = vi.fn(async (q: unknown) => {
    if (opts.fail) throw new Error("connection refused");
    const text = typeof q === "string" ? q : ((q as { text?: string }).text ?? "");
    if (text.includes("FROM opportunities o")) {
      return { rows: opts.rows ?? [] };
    }
    return { rows: [] };
  });
  return {
    query,
    connect: async () => ({ query, release: () => {} }),
  };
}

// Top-level import: the route module pulls the simulation + tokenValidation
// dependency tree (transform ~6s cold) — loading it here keeps that cost OUT
// of any single test's 5s timeout (the in-test dynamic import made the first
// test absorb the transform and flake out).
const { mountOpportunitiesLive } = await import("./opportunities-live.js");

async function buildApp(pool: unknown): Promise<Express> {
  const app = express();
  mountOpportunitiesLive(
    app,
    // biome-ignore lint/suspicious/noExplicitAny: test-only pool stand-in
    (pool === null ? null : (pool as any)) as never,
    null,
    logger,
  );
  return app;
}

describe("GET /api/v1/opportunities/live — window_total contract (WO-H4)", () => {
  it("(a) pool null → 503 db_unavailable", async () => {
    const app = await buildApp(null);
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(503);
    expect(r.body.error).toBe("db_unavailable");
  });

  it("(b) window_total is the row's COUNT-over-window value, NOT items.length, and never leaks per-item", async () => {
    const rows = [
      fixtureRow(),
      fixtureRow({ id: "00000000-0000-4000-8000-000000000002" }),
    ];
    const app = await buildApp(fakePool({ rows }));
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    expect(r.body.count).toBe(2);
    expect(r.body.window_total).toBe(137);
    expect(r.body.items).toHaveLength(2);
    // Envelope-only field: rowToOpportunity must NOT copy it into items.
    expect(r.body.items[0].window_total).toBeUndefined();
  });

  it("(c) empty window → window_total 0 (computed-and-zero, R8 — not null)", async () => {
    const app = await buildApp(fakePool({ rows: [] }));
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    expect(r.body.count).toBe(0);
    expect(r.body.window_total).toBe(0);
    expect(r.body.items).toEqual([]);
  });

  it("(d) query fails → 503 query_failed", async () => {
    const app = await buildApp(fakePool({ fail: true }));
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(503);
    expect(r.body.error).toBe("query_failed");
  });

  it("(e) LIVE_QUERY pins the pre-LIMIT count expression", async () => {
    const pool = fakePool({ rows: [] });
    const app = await buildApp(pool);
    await request(app).get("/api/v1/opportunities/live?limit=50");
    const firstCall = (pool.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[0];
    const text =
      typeof firstCall === "string" ? firstCall : ((firstCall as { text?: string })?.text ?? "");
    expect(text).toContain("COUNT(*) OVER ()");
    expect(text).toContain("AS window_total");
  });
});
