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


describe("opportunity evidence provenance — constructed unit inputs, not production observations", () => {
  it("canonical estimates are preserved but never fabricate simulation or individual costs", async () => {
    const app = await buildApp(fakePool({ rows: [fixtureRow({
      status: "rejected", rejection_reason: "TokenNotAllowed:unit-test",
      expected_profit_usd: 7.83771991, net_expected_profit_usd: 7.240022, roi_pct: null,
    })] }));
    const response = await request(app).get("/api/v1/opportunities/live");
    expect(response.status).toBe(200);
    const row = response.body.items[0];
    expect(row.expected_profit_usd).toBe(7.83771991);
    expect(row.net_expected_profit_usd).toBe(7.240022);
    expect(row.paper_status).toBe("paper_rejected");
    for (const key of ["simulated_net_profit_usd", "simulated_amount_in_usd", "simulated_roi_pct",
      "simulated_cost_breakdown", "simulated_at", "simulated_target"]) expect(row[key], key).toBeNull();
  });
  for (const hops of [2, 3, 4, 5]) {
    it(`${hops} hops preserve every pool/token and ALL intermediate DEX names`, async () => {
      const addresses = Array.from({length: hops}, (_, i) => `0x${(i + 1).toString(16).padStart(40, "0")}`);
      const adapters = Array.from({length: hops}, (_, i) => `unit-dex-${i}`);
      const topology = {
        token_addresses: [...addresses, addresses[0]],
        pool_addresses: addresses.map((_, i) => `0x${(i + 101).toString(16).padStart(40, "0")}`),
        dex_adapters: adapters,
      };
      const app = await buildApp(fakePool({ rows: [fixtureRow({
        token_in: addresses[0], token_out: addresses[0], dex_a: adapters[0], dex_b: adapters[hops - 1],
        route_metadata: topology,
      })] }));
      const response = await request(app).get("/api/v1/opportunities/live");
      expect(response.status).toBe(200);
      expect(response.body.items[0].route_metadata).toEqual(topology);
      expect(response.body.items[0].dexes_used).toEqual(adapters);
    });
  }
  for (const status of ["rejected", "failed"]) {
    it(`${status} without a reason does not become paper_viable`, async () => {
      const app = await buildApp(fakePool({ rows: [fixtureRow({status, rejection_reason: null})] }));
      const response = await request(app).get("/api/v1/opportunities/live");
      expect(response.status).toBe(200);
      expect(response.body.items[0].paper_status).toBe("paper_rejected");
    });
  }
  it("preserves a computed forward zero, complete cost vector and timestamp without fallback", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const cost = {gas_usd: 1, lp_fees_usd: 2, slippage_usd: 0, failure_buffer_usd: 0,
      copied_buffer_usd: 0, capital_cost_usd: 0, ops_overhead_usd: 0, flashloan_fee_usd: 0, relay_fee_usd: 0};
    const sim = {forward:{net_usd:0,amount_in_usd:100,roi_pct:0,cost_breakdown:cost,notes:[]},
      inverse:null, simulated_at:"2026-09-14T00:00:00Z"};
    const result = __forTesting.rowToOpportunity(fixtureRow() as never, sim as never, null, new Map(), new Map());
    expect(result.simulated_net_profit_usd).toBe(0);
    expect(result.simulated_roi_pct).toBe(0);
    expect(result.simulated_amount_in_usd).toBe(100);
    expect(result.simulated_cost_breakdown).toEqual(cost);
    expect(result.simulated_at).toBe(sim.simulated_at);
    expect(result.net_expected_profit_usd).toBe(0.5);
  });
});

// CARDS-DEDUP-HOPS (2026-09-20, WO-3): LIVE_QUERY now collapses re-detections
// of the same route identity into ONE row via the `grouped` CTE —
// MIN/MAX(detected_at) → first_seen_at/last_seen_at, COUNT(*) → confirmations,
// latest detection's economics on the wire. These tests pin (1) the CTE shape
// (dropping it while fixtures keep injecting aggregates = silent identity
// regression) and (2) the wire forwarding incl. TIMESTAMPTZ Date → ISO.
describe("CARDS-DEDUP-HOPS — grouped CTE + route-group aggregates on the wire", () => {
  it("(f) LIVE_QUERY pins the grouped CTE and the route_group_key twin expression", async () => {
    const pool = fakePool({ rows: [] });
    const app = await buildApp(pool);
    await request(app).get("/api/v1/opportunities/live?limit=50");
    const firstCall = (pool.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[0];
    const text =
      typeof firstCall === "string" ? firstCall : ((firstCall as { text?: string })?.text ?? "");
    expect(text).toContain("WITH grouped AS");
    expect(text).toContain("MIN(o.detected_at) AS first_seen_at");
    expect(text).toContain("MAX(o.detected_at) AS last_seen_at");
    expect(text).toContain("COUNT(*)::int      AS confirmations");
    // id DESC tiebreaker: same-burst detections must resolve to the same
    // latest row every poll (economics stability, WARN-1 review WO-3).
    expect(text).toContain(
      "(ARRAY_AGG(o.id ORDER BY o.detected_at DESC, o.id DESC))[1] AS latest_id",
    );
    expect(text).toContain("GROUP BY 1");
    expect(text).toContain("JOIN opportunities o");
    expect(text).toContain("o.id = g.latest_id");
    // The bit-for-bit twin of routeGroupKeyOf() (frontend route-key.test.ts).
    // concat_ws skips NULLs → every nullable segment must be COALESCE'd to ''.
    expect(text).toContain(
      "concat_ws('|',\n      o.chain_id::text,\n      COALESCE(o.chain_id_out::text, ''),\n      COALESCE(o.strategy_kind, ''),\n      o.token_in,\n      o.token_out,\n      o.dex_a,\n      COALESCE(o.dex_b, '')\n    ) AS route_group_key",
    );
  });

  it("(g) route-group aggregates forward verbatim; TIMESTAMPTZ Date → ISO string", async () => {
    const rows = [fixtureRow({
      first_seen_at: new Date("2026-09-20T09:00:00Z"),
      last_seen_at: new Date("2026-09-20T12:00:01Z"),
      confirmations: 7,
      route_group_key: "1||dex_arb|0x…1|0x…2|uniswap_v2|sushiswap",
    })];
    const app = await buildApp(fakePool({ rows }));
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    const item = r.body.items[0];
    expect(item.first_seen_at).toBe("2026-09-20T09:00:00.000Z");
    expect(item.last_seen_at).toBe("2026-09-20T12:00:01.000Z");
    expect(item.confirmations).toBe(7);
    expect(item.route_group_key).toBe("1||dex_arb|0x…1|0x…2|uniswap_v2|sushiswap");
  });

  it("(h) rows without aggregates (WS-shaped) emit undefined, never fabricated 1s (R8)", async () => {
    const app = await buildApp(fakePool({ rows: [fixtureRow()] }));
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    expect(r.body.items[0].first_seen_at).toBeUndefined();
    expect(r.body.items[0].confirmations).toBeUndefined();
  });
});


// WO-G2-PARITY (2026-09-17): regression for FEED-SCHEMA-01 — opportunities
// .block_number is BIGINT (migration 003) and node-postgres returns int8 as a
// STRING, so rowToOpportunity shipped "25995384" on the wire while its own
// interface declared number|null; the frontend Zod z.number() then rejected the
// WHOLE /api/v1/opportunities/live payload and OpportunityTicker (root layout,
// every page incl. /operations and /status) stayed on "Opportunity feed
// unavailable (retrying every 30s)". The mapper must normalize to a real JSON
// number, keep null as null (R8), and never emit NaN.
describe("WO-G2-PARITY — block_number wire normalization (int8-as-string)", () => {
  it("maps a node-postgres int8 string to a JSON number", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ block_number: "25995384" }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.block_number).toBe(25995384);
    expect(typeof result.block_number).toBe("number");
  });

  it("keeps a numeric block_number numeric (no double coercion drift)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ block_number: 1000 }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.block_number).toBe(1000);
  });

  it("keeps null as null — R8: block not detected is not block 0", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ block_number: null }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.block_number).toBeNull();
  });

  it("degrades a non-numeric value to null instead of emitting NaN on the wire", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ block_number: "garbage" }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.block_number).toBeNull();
  });
});
