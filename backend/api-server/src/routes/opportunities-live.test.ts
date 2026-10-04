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

async function buildApp(pool: unknown, redis: unknown = null): Promise<Express> {
  const app = express();
  mountOpportunitiesLive(
    app,
    // biome-ignore lint/suspicious/noExplicitAny: test-only pool stand-in
    (pool === null ? null : (pool as any)) as never,
    // biome-ignore lint/suspicious/noExplicitAny: test-only redis stand-in
    redis as any,
    logger,
  );
  return app;
}

/**
 * CARDS-PRICES-01 gate (adversarial-review finding #5): the route's PriceBus
 * read had ZERO coverage — both existing suites mounted with redis = null, so a
 * bug that always emitted null (or read the wrong chain key) shipped green.
 * Minimal in-memory double, same shape as prices-live.test.ts.
 */
function fakePriceRedis(prices: Record<string, string>, chainId = 1): unknown {
  return {
    hgetall: async (key: string) =>
      key === `arbx:token_prices:${chainId}` ? { ...prices } : {},
    get: async (_key: string) => null,
    ttl: async (_key: string) => 42,
  };
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

  // CARDS-PRICES-01 (adversarial-review finding #5): the PriceBus enrichment is
  // exercised end-to-end with a Redis double — prices are attached per card
  // keyed by the row's OWN symbols (UPPER), and a null/missing price never
  // fabricates an entry.
  it("(c2) with a PriceBus hash → items carry token_prices_usd for their symbols only", async () => {
    const rows = [fixtureRow({ chain_id: 1 })]; // token_in_symbol WETH, token_out_symbol USDC
    const app = await buildApp(
      fakePool({ rows }),
      fakePriceRedis({ WETH: "2685.78", USDC: "0.9998", PEPE: "0.0000043" }),
    );
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    // Only the card's own symbols — the unrelated PEPE price is NOT attached.
    expect(r.body.items[0].token_prices_usd).toEqual({ WETH: 2685.78, USDC: 0.9998 });
  });

  it("(c3) redis null → token_prices_usd is null on every item (R8 fail-honest)", async () => {
    const app = await buildApp(fakePool({ rows: [fixtureRow()] }), null);
    const r = await request(app).get("/api/v1/opportunities/live?limit=50");
    expect(r.status).toBe(200);
    expect(r.body.items[0].token_prices_usd).toBeNull();
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
      "simulated_cost_breakdown", "simulated_at", "simulated_target",
      // CARDS-NOTIONAL-01: the ladder's own gross/Σcosts are null too when no
      // forward ran — never a 0 the card could pair with a canonical figure.
      "simulated_gross_usd", "simulated_costs_total_usd"]) expect(row[key], key).toBeNull();
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
  // LEGSYM-01 (2026-09-26): `leg_symbols` was emitted as `null` on 41/41 items
  // of the live feed. Root cause proven against production: the hydrator skipped
  // BOTH endpoint tokens (they were assumed covered by the LEFT JOIN, which in
  // fact only fills `token_in_symbol`/`token_out_symbol`), and every live route
  // is a 2-leg CLOSED cycle A→B→A whose `token_addresses` contains nothing but
  // those two endpoints — so `legMissing` stayed empty, the lookup map stayed
  // empty, and `any` stayed false. This test pins the live shape.
  it("LEGSYM-01: a 2-hop closed cycle (A→B→A) emits leg_symbols for BOTH endpoints", async () => {
    const A = "0x0000000000000000000000000000000000000001"; // fixture token_in  (WETH)
    const B = "0x0000000000000000000000000000000000000002"; // fixture token_out (USDC)
    const topology = {
      token_addresses: [A, B, A],
      pool_addresses: [
        "0x00000000000000000000000000000000000000c1",
        "0x00000000000000000000000000000000000000c2",
      ],
      dex_adapters: ["uniswap-v2", "sushiswap"],
    };
    const app = await buildApp(fakePool({ rows: [fixtureRow({ route_metadata: topology })] }));
    const response = await request(app).get("/api/v1/opportunities/live");
    expect(response.status).toBe(200);
    // Real resolved symbols from the row's own enrichment — never fabricated.
    expect(response.body.items[0].leg_symbols).toEqual({ [A]: "WETH", [B]: "USDC" });
  });

  it("LEGSYM-01 (R8): an unresolved endpoint stays ABSENT from leg_symbols — never a guess", async () => {
    const A = "0x0000000000000000000000000000000000000001";
    const B = "0x0000000000000000000000000000000000000002";
    const topology = {
      token_addresses: [A, B, A],
      pool_addresses: [
        "0x00000000000000000000000000000000000000c1",
        "0x00000000000000000000000000000000000000c2",
      ],
      dex_adapters: ["uniswap-v2", "sushiswap"],
    };
    const app = await buildApp(
      fakePool({
        rows: [
          fixtureRow({
            route_metadata: topology,
            // The endpoint join missed and the on-demand resolver found nothing.
            token_in_symbol: null,
            token_out_symbol: null,
          }),
        ],
      }),
    );
    const response = await request(app).get("/api/v1/opportunities/live");
    expect(response.status).toBe(200);
    expect(response.body.items[0].leg_symbols).toBeNull();
  });

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

  // CARDS-NOTIONAL-01 (2026-09-26): the ladder's OWN gross and Σcosts must ride
  // the wire next to its net. Without them every consumer had to pair the SIM
  // cost breakdown with `expected_profit_usd` / `net_expected_profit_usd` —
  // canonical figures the SEARCHER produced at the searcher's own size — which
  // is how the card printed `IN $0.00` beside `GROSS $1.47M` and
  // `Total cost $73.4k` beside `Net yield -$0.0000`.
  it("CARDS-NOTIONAL-01: publishes the ladder's own gross and Σcosts, closed against its net", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const cost = {gas_usd: 11.8, lp_fees_usd: 3, slippage_usd: 5, failure_buffer_usd: 0.4,
      copied_buffer_usd: 12.61391713, capital_cost_usd: 0, ops_overhead_usd: 0.01,
      flashloan_fee_usd: 0.9, relay_fee_usd: 1.261391713};
    const total = 34.985308843;
    const sim = {forward:{gross_usd: 25.22783426, net_usd: 25.22783426 - total,
        costs_total_usd: total, amount_in_usd: 1000, roi_pct: -0.97,
        cost_breakdown: cost, notes: ["basis=amount_in_wei"]},
      inverse:null, simulated_at:"2026-09-26T00:00:00Z"};
    const result = __forTesting.rowToOpportunity(
      fixtureRow() as never, sim as never, null, new Map(), new Map(),
    );
    expect(result.simulated_gross_usd).toBe(25.22783426);
    expect(result.simulated_costs_total_usd).toBe(total);
    // Closure inside ONE computation, on the wire itself.
    expect(
      Math.abs(result.simulated_net_profit_usd - (result.simulated_gross_usd - result.simulated_costs_total_usd)),
    ).toBeLessThanOrEqual(0.005);
    // The basis marker travels with the ladder.
    expect(result.simulated_notes).toContain("basis=amount_in_wei");
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
    // representative row every poll (economics stability, WARN-1 review WO-3).
    // ROUTE-REP-01 (2026-09-28): the primary key is `$6`-gated — with
    // route_representative=best_net the group is represented by its computed
    // best net, so a later re-detection cannot bury a real gain.
    expect(text).toContain(
      "(ARRAY_AGG(o.id ORDER BY\n       CASE WHEN $6::bool\n            THEN (o.economics->>'net_profit_usd')::numeric\n            ELSE NULL END DESC NULLS LAST,\n       o.detected_at DESC, o.id DESC))[1] AS latest_id",
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

  // ROUTE-REP-01 (2026-09-28): the representative selector is opt-in. The
  // default must stay `latest` (previous behaviour for every other consumer),
  // and an unknown value must never open a third silent mode.
  it("(h) ROUTE-REP-01: best_net rides the query as $6; latest is the default", async () => {
    const poolBest = fakePool({ rows: [] });
    const appBest = await buildApp(poolBest);
    const resBest = await request(appBest).get(
      "/api/v1/opportunities/live?limit=50&route_representative=best_net",
    );
    const paramsBest = (poolBest.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[1] as unknown[];
    expect(paramsBest?.[5]).toBe(true);
    expect(resBest.body?.route_representative).toBe("best_net");

    const poolDefault = fakePool({ rows: [] });
    const appDefault = await buildApp(poolDefault);
    const resDefault = await request(appDefault).get("/api/v1/opportunities/live?limit=50");
    const paramsDefault = (poolDefault.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[1] as unknown[];
    expect(paramsDefault?.[5]).toBe(false);
    expect(resDefault.body?.route_representative).toBe("latest");

    const poolBogus = fakePool({ rows: [] });
    const appBogus = await buildApp(poolBogus);
    const resBogus = await request(appBogus).get(
      "/api/v1/opportunities/live?limit=50&route_representative=cuac",
    );
    expect((poolBogus.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[1]?.[5]).toBe(false);
    expect(resBogus.body?.route_representative).toBe("latest");
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

// ── ECON-DECLARE-01 (2026-09-27) ─────────────────────────────────────────────
// The operator's complaint: "hay muchos valores que no se ven, no están
// declarados." The census is how the closure is MEASURED from outside the
// producer instead of asserted. Baseline it must reproduce, measured on the live
// feed (33 rows, 2026-09-27T02:06Z):
//
//     route_metadata present            33/33
//     economics_amount_in_wei declared   0/33   ← the entire gap
//     economics_basis declared           0/33
//     expected_profit_usd present       18/33
//     net_expected_profit_usd present   15/33
//     roi_pct present                    0/33
//     risk_score present                 0/33

describe("ECON-DECLARE-01 — economics-declaration census", () => {
  /** One live-shaped item: the fields the census reads, and nothing else. */
  const item = (over: Record<string, unknown> = {}) => ({
    amount_in_wei: "1000000000000000000",
    expected_profit_usd: 12.5,
    net_expected_profit_usd: -50,
    roi_pct: null,
    risk_score: null,
    route_metadata: { dex_adapters: ["uniswap-v2", "sushiswap"] },
    ...over,
  });

  it("reproduces the measured baseline: route present, NOTHING declared", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([
      item(),
      item({ expected_profit_usd: null }),
      item({ net_expected_profit_usd: null, amount_in_wei: "0" }),
    ]);
    expect(census.rows).toBe(3);
    expect(census.rows_with_route_metadata).toBe(3);
    // The gap, stated as a number: 0 of 3 rows declare what the figures are.
    expect(census.rows_with_declared_notional).toBe(0);
    expect(census.rows_with_declared_basis).toBe(0);
    expect(census.declaration_rate_pct).toBe(0);
    // Every figure that IS present counts against the declaration rate.
    expect(census.present_but_undeclared["expected_profit_usd"]).toBe(2);
    expect(census.present_but_undeclared["net_expected_profit_usd"]).toBe(2);
    // amount "0" is not a carried amount — it is the lost-decoder state (R8).
    expect(census.present_but_undeclared["amount_in_wei"]).toBe(2);
  });

  it("goes to 100% once a producer declares the notional and the basis", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([
      item({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          economics_amount_in_wei: "1000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
        },
      }),
      item({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          economics_amount_in_wei: "1000000000000000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "stamped" },
        },
      }),
    ]);
    expect(census.rows_with_declared_notional).toBe(2);
    expect(census.rows_with_declared_basis).toBe(2);
    expect(census.declaration_rate_pct).toBe(100);
    expect(census.declared_basis_words).toEqual({
      probe: 2, kernel: 2, intent: 1, stamped: 1,
    });
    // gross + net + amount are all declared ⇒ nothing counts against the rate.
    expect(census.present_but_undeclared).toEqual({});
  });

  it("counts roi_pct / risk_score as undeclared when present — no vocabulary exists", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([
      item({ roi_pct: 0.12, risk_score: 0.34 }),
    ]);
    // These two have no basis word on the wire at all (0/33 live) — the census
    // must report the gap rather than treat their presence as declared.
    expect(census.present_but_undeclared["roi_pct"]).toBe(1);
    expect(census.present_but_undeclared["risk_score"]).toBe(1);
    expect(census.economic_figures_present["roi_pct"]).toBe(1);
  });

  it("a malformed declaration is NOT counted as declared (no repair)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([
      item({
        route_metadata: {
          dex_adapters: ["uniswap-v2"],
          economics_amount_in_wei: "not-a-number",
          economics_basis: { gross: "  " },
        },
      }),
    ]);
    expect(census.rows_with_declared_notional).toBe(0);
    expect(census.rows_with_declared_basis).toBe(0);
    expect(census.declaration_rate_pct).toBe(0);
  });

  it("R8: an empty batch reports 0, never null and never a fabricated rate", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([]);
    expect(census.rows).toBe(0);
    expect(census.declaration_rate_pct).toBe(0);
    expect(census.declared_basis_words).toEqual({});
    expect(census.present_but_undeclared).toEqual({});
  });

  it("counts a row with no route_metadata without inventing one", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const census = __forTesting.censusEconomicsDeclaration([
      item({ route_metadata: null }),
    ]);
    expect(census.rows_with_route_metadata).toBe(0);
    expect(census.rows_with_declared_notional).toBe(0);
    // The figure is still present and still undeclared — stated, not hidden.
    expect(census.present_but_undeclared["expected_profit_usd"]).toBe(1);
  });
});

// ── ALWAYS-COMPUTE (operator mandate 2026-09-27) ─────────────────────────────
// Deliverables under test (unit level; the testcontainers file covers the SQL
// column): (2) required_amount_in_usd can no longer vanish via JSON, (3) the
// per-field missing_economics census, (4) numeric-string hardening at this
// JSON boundary, (1) the economics object passes through to the wire shape.

describe("ALWAYS-COMPUTE — hardenEconomics (boundary hardening)", () => {
  it("accepts native numbers verbatim and numeric strings for USD fields", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const out = __forTesting.hardenEconomics({
      computation_status: "computed",
      gross_profit_usd: 12.5,
      net_profit_usd: "-50",          // drifted producer → tolerated
      total_cost_usd: "62.5",
      amount_in_wei: "1000000000000000000",
      amount_out_wei: "990000000000000000",
    });
    expect(out).not.toBeNull();
    expect(out!.gross_profit_usd).toBe(12.5);
    expect(out!.net_profit_usd).toBe(-50);
    expect(out!.total_cost_usd).toBe(62.5);
    expect(out!.amount_in_wei).toBe("1000000000000000000");
  });

  it("degrades unparseable USD strings and non-digit wei strings to null (R8, never 0)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const out = __forTesting.hardenEconomics({
      computation_status: "partial",
      gross_profit_usd: "  ",          // empty-after-trim → null
      net_profit_usd: "abc",
      roi_pct: "1e999",                // Infinity → not finite → null
      amount_in_wei: "0xdeadbeef",     // not /^-?\d+$/ → null
      amount_out_wei: 12345,           // number where wei-string expected → null
    });
    expect(out!.gross_profit_usd).toBeNull();
    expect(out!.net_profit_usd).toBeNull();
    expect(out!.roi_pct).toBeNull();
    expect(out!.amount_in_wei).toBeNull();
    expect(out!.amount_out_wei).toBeNull();
  });

  it("returns null for absent/non-object columns (pre-migration rows)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    expect(__forTesting.hardenEconomics(null)).toBeNull();
    expect(__forTesting.hardenEconomics(undefined as never)).toBeNull();
    expect(__forTesting.hardenEconomics("nope" as never)).toBeNull();
  });
});

describe("ALWAYS-COMPUTE — missingEconomicsCensus (deliverable #3)", () => {
  it("counts non-zero on TODAY's production shape (nulls everywhere) — measures the gap", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    // The operator's audit shape: rejected row, gross/net/roi null, no
    // economics object, no sim context, no ledger.
    const row = fixtureRow({
      expected_profit_usd: null,
      net_expected_profit_usd: null,
      roi_pct: null,
      route_metadata: null,
      economics: null,
    });
    const census = __forTesting.missingEconomicsCensus([row as never], new Map());
    expect(census.rows).toBe(1);
    expect(census.fields.amount_in_usd).toBe(1);
    expect(census.fields.gross_usd).toBe(1);
    expect(census.fields.costs_usd).toBe(1);
    expect(census.fields.net_usd).toBe(1);
    expect(census.fields.roi_pct).toBe(1);
    expect(census.fields.target_usd).toBe(1);
    expect(census.fields.achieved_usd).toBe(1);
    expect(census.fields.required_amount_usd).toBe(1);
    expect(census.fields.ledger).toBe(1);
  });

  it("near-zero for a fully-computed rejected row (the post-patch expectation)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const econ = {
      computation_status: "computed",
      amount_in_usd: 2350,
      gross_profit_usd: 12.5,
      gas_usd: 0.18,
      flash_fee_usd: 2.12,
      bribe_usd: 0,
      other_costs_usd: 0.01,
      total_cost_usd: 2.31,
      net_profit_usd: -50,
      roi_pct: -2.13,
      target_net_usd: 25,
      target_delta_usd: -75,
      meets_target: false,
      legs: [{ token_in: "0xa", token_out: "0xb", amount_in_wei: "1", amount_out_wei: "1" }],
    };
    const row = fixtureRow({
      expected_profit_usd: 12.5,
      net_expected_profit_usd: -50,
      roi_pct: -2.13,
      route_metadata: { leg_amounts_in: ["1"] },
      economics: econ,
    });
    const sim = new Map([
      [row.id as string, {
        forward: null,
        inverse: {
          target_net_usd: 25, target_roi_pct: null, target_source: "simulation_tab",
          binding_floor: "net-per-usd-nonpositive",
          required_amount_in_usd: "Infinity", required_is_infinite: true,
          cap_amount_in_usd: 1000, suggested_amount_in_usd: 0, suggested_net_usd: -50,
          suggested_roi_pct: 0, meets_target_at_cap: false,
          estimation_basis: "observed-gross", notes: [],
        },
        simulated_at: new Date().toISOString(),
      }],
    ]);
    const census = __forTesting.missingEconomicsCensus([row as never], sim as never);
    expect(census.fields.amount_in_usd).toBe(0);
    expect(census.fields.gross_usd).toBe(0);
    expect(census.fields.costs_usd).toBe(0);
    expect(census.fields.net_usd).toBe(0);
    expect(census.fields.roi_pct).toBe(0);
    expect(census.fields.target_usd).toBe(0);
    expect(census.fields.achieved_usd).toBe(0);
    // The "Infinity" sentinel is a COMPUTED verdict, not a missing field.
    expect(census.fields.required_amount_usd).toBe(0);
    expect(census.fields.ledger).toBe(0);
  });
});

describe("ALWAYS-COMPUTE — rowToOpportunity economics passthrough (deliverable #1)", () => {
  it("carries the economics object into the wire shape, hardened", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({
        economics: {
          computation_status: "computed",
          gross_profit_usd: "12.5",
          net_profit_usd: -50,
          amount_in_wei: "1000",
          not_computed_reasons: { dex_fees_usd: "included_in_amount_out_post_fee" },
        },
      }) as never,
      undefined, null, new Map(), new Map(),
    ) as Record<string, unknown>;
    const econ = result.economics as Record<string, unknown>;
    expect(econ).not.toBeNull();
    expect(econ.computation_status).toBe("computed");
    expect(econ.gross_profit_usd).toBe(12.5); // numeric string hardened → number
    expect(econ.net_profit_usd).toBe(-50);
  });

  it("emits economics:null on legacy rows (absence is a state, R8)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ economics: null }) as never,
      undefined, null, new Map(), new Map(),
    ) as Record<string, unknown>;
    expect(result.economics).toBeNull();
  });
});

// A8-CONF-01 (2026-10-03). The home card's "conf" read
// `opp.confidence_score_bps` off a payload that never carried it: the card was
// permanently "— conf (unscored)" while the Gate-C scorer was in fact writing
// ~10M rows into scored_opportunities. These tests pin the three facts that
// keep that from silently regressing:
//   (i)   LIVE_QUERY actually joins the sink (dropping the join while the
//         fixture keeps injecting scored_* columns = silent dead wire again);
//   (ii)  the bps mapping + R8 None/Some(0.0) distinction;
//   (iii) the declared reason on the not-computed path (R10).
describe("A8-CONF-01 — confidence scoring wire end-to-end", () => {
  it("(i) LIVE_QUERY joins scored_opportunities with the indexed key and a single-row LATERAL", async () => {
    const pool = fakePool({ rows: [] });
    const app = await buildApp(pool);
    await request(app).get("/api/v1/opportunities/live?limit=50");
    const firstCall = (pool.query as ReturnType<typeof vi.fn>).mock.calls[0]?.[0];
    const text =
      typeof firstCall === "string" ? firstCall : ((firstCall as { text?: string })?.text ?? "");
    // The join key MUST cast the uuid: opportunities.id is uuid, the sink's
    // opportunity_id is TEXT (migration 097).
    expect(text).toContain("s.opportunity_id = o.id::text");
    expect(text).toContain("FROM scored_opportunities s");
    // LATERAL + LIMIT 1: the emitter scores on BOTH the accept and the reject
    // path, so a plain LEFT JOIN would duplicate the card.
    expect(text).toContain("LEFT JOIN LATERAL (");
    expect(text).toContain("ORDER BY s.created_at DESC, s.id DESC");
    expect(text).toContain("LIMIT 1");
    expect(text).toContain("sc.posterior_prob       AS scored_posterior_prob");
  });

  it("maps the scored posterior to integer basis points, keeping the un-rounded value", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({
        scored_posterior_prob: 0.87,
        scored_kelly_fraction: 0.125,
        scored_bayesian_accepted: true,
        scored_emission_outcome: "accepted",
        scored_rejection_reason: null,
      }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.confidence_score_bps).toBe(8700);
    expect(result.posterior_probability_bps).toBe(8700);
    expect(result.posterior_prob).toBe(0.87);
    expect(result.kelly_fraction_bps).toBe(1250);
    expect(result.scoring_decision).toBe("accepted");
    expect(result.scoring_reason).toBeNull();
    expect(result.confidence_state).toBe("computed");
    expect(result.confidence_source).toBe("scored_opportunities.posterior_prob");
    expect(result.confidence_reason).toBeNull();
  });

  it("a sub-basis-point posterior is COMPUTED 0 bps, not NOT COMPUTED — the raw float rides along", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    // Real prod value (scored_opportunities, 2026-10-03): 2.600356865160073e-05.
    const result = __forTesting.rowToOpportunity(
      fixtureRow({
        scored_posterior_prob: 2.600356865160073e-5,
        scored_kelly_fraction: 0,
        scored_bayesian_accepted: false,
        scored_emission_outcome: "rejected",
        scored_rejection_reason: "non_positive_profit",
      }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.confidence_score_bps).toBe(0);
    expect(result.confidence_state).toBe("computed");
    expect(result.confidence_reason).toBeNull();
    // The distinction the card needs to avoid dressing this as a hard zero.
    expect(result.posterior_prob).toBeCloseTo(2.600356865160073e-5, 12);
    expect(result.scoring_decision).toBe("rejected");
    expect(result.scoring_reason).toBe("non_positive_profit");
  });

  it("no scored row for the opportunity => NO COMPUTADO with an explicit reason, never 0 (R8/R10)", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ scored_posterior_prob: null }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.confidence_score_bps).toBeNull();
    expect(result.confidence_state).toBe("not_computed");
    expect(result.confidence_reason).toBe("no_scored_row_for_opportunity");
    expect(result.confidence_source).toBeNull();
  });

  it("a non-finite posterior degrades to NOT COMPUTADO instead of laundering NaN onto the wire", async () => {
    const { __forTesting } = await import("./opportunities-live.js");
    const result = __forTesting.rowToOpportunity(
      fixtureRow({ scored_posterior_prob: Number.NaN }) as never,
      undefined, null, new Map(), new Map(),
    );
    expect(result.confidence_score_bps).toBeNull();
    expect(result.posterior_prob).toBeNull();
    expect(result.confidence_state).toBe("not_computed");
  });
});
