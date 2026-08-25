/**
 * quote-anchor tests — EMIT-02 Layer-2 / EMIT-03 surface.
 *
 * Pins the hard invariants of GET /api/quote/anchor + POST /api/admin/quote/preview:
 *   (a) GET serves EXACTLY the 8 flattened keys — the preview-only sidecars
 *       (pairs_by_symbol / pools_by_symbol) NEVER ride the response,
 *   (b) R8 fail-honest: 503 redis_unavailable / quote_anchor_not_published /
 *       quote_anchor_snapshot_corrupted — never a fabricated anchor,
 *   (c) preview is admin-gated and validates the weights contract (mirrors
 *       quote_score.rs QuoteWeights::validate at wire level),
 *   (d) preview deterministically re-ranks the SAME rows: anchor change ⇒
 *       revaluation true + affected = both anchors' footprints + version+1;
 *       no change ⇒ everything honest-zero/unchanged,
 *   (e) QB-TOPOLOGY-01 doctrine literals: graph_rebuild_required === false,
 *       topology_version_unchanged === true.
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it, vi } from "vitest";
import type Redis from "ioredis";

import { mountQuoteAnchor } from "./quote-anchor.js";

const logger = { warn: vi.fn() };
const ADMIN = "test-admin-token";

function buildApp(redis: unknown): Express {
  const app = express();
  app.use(express.json());
  mountQuoteAnchor(app, { redis: redis as Redis | null, logger, adminToken: ADMIN });
  return app;
}

// Fixture mirrors quote_anchor_runtime::quote_anchor_snapshot_to_wire EXACTLY
// (10 keys: 7 view + tokens + 2 preview sidecars). Published anchor = USDC.
const A41 = `0x${"a".repeat(40)}`;
const B41 = `0x${"b".repeat(40)}`;
const USDC_C = { prior: 100, liquidity: 95, venues: 90, stability: 100, cross_dex: 95 };
const WETH_C = { prior: 80, liquidity: 100, venues: 100, stability: 90, cross_dex: 100 };
const SNAPSHOT_OBJ = {
  chain_id: 1,
  quote_symbol: "USDC",
  quote_score: 96.0,
  quote_version: 3,
  graph_version: 12345,
  components: USDC_C,
  weights: { prior: 0.3, liquidity: 0.2, venues: 0.1, stability: 0.3, cross_dex: 0.1 },
  tokens: [
    { symbol: "USDC", address: A41, components: USDC_C, score: 96.0 },
    { symbol: "WETH", address: B41, components: WETH_C, score: 91.0 },
  ],
  pairs_by_symbol: { USDC: 42, WETH: 17 },
  pools_by_symbol: { USDC: 60, WETH: 25 },
};
const SNAPSHOT = JSON.stringify(SNAPSHOT_OBJ);

// Liquidity/venue-heavy weights ⇒ WETH (100.0) overtakes USDC (94.0).
const CHANGE_W = { prior: 0, liquidity: 0.5, venues: 0.2, stability: 0, cross_dex: 0.3 };
// Prior-only weights ⇒ USDC (100.0) keeps the head over WETH (80.0).
const KEEP_W = { prior: 1, liquidity: 0, venues: 0, stability: 0, cross_dex: 0 };

describe("GET /api/quote/anchor (EMIT-02 Layer-2)", () => {
  it("returns 503 when Redis is null (fail-honest)", async () => {
    const res = await request(buildApp(null)).get("/api/quote/anchor");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("serves EXACTLY the 8 flattened keys — sidecars stripped, view verbatim", async () => {
    const redis = { get: vi.fn(async () => SNAPSHOT) };
    const res = await request(buildApp(redis)).get("/api/quote/anchor?chain_id=1");
    expect(res.status).toBe(200);
    expect(Object.keys(res.body).sort()).toEqual(
      [
        "chain_id",
        "quote_symbol",
        "quote_score",
        "quote_version",
        "graph_version",
        "components",
        "weights",
        "tokens",
      ].sort(),
    );
    expect(res.body.pairs_by_symbol).toBeUndefined();
    expect(res.body.pools_by_symbol).toBeUndefined();
    expect(res.body.quote_symbol).toBe("USDC");
    expect(res.body.quote_version).toBe(3);
    expect(res.body.tokens).toHaveLength(2);
    expect(redis.get).toHaveBeenCalledWith("arbx:quote:anchor:1");
  });

  it("defaults chain_id to 1 (canonical pattern)", async () => {
    const redis = { get: vi.fn(async () => SNAPSHOT) };
    const res = await request(buildApp(redis)).get("/api/quote/anchor");
    expect(res.status).toBe(200);
    expect(redis.get).toHaveBeenCalledWith("arbx:quote:anchor:1");
  });

  it("503 quote_anchor_not_published when the key is absent — never fabricated", async () => {
    const redis = { get: vi.fn(async () => null) };
    const res = await request(buildApp(redis)).get("/api/quote/anchor");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("quote_anchor_not_published");
  });

  it("503 quote_anchor_snapshot_corrupted on unparseable payload", async () => {
    const redis = { get: vi.fn(async () => "not-json{{") };
    const res = await request(buildApp(redis)).get("/api/quote/anchor");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("quote_anchor_snapshot_corrupted");
    expect(logger.warn).toHaveBeenCalled();
  });

  it("a transient Redis error degrades to 503, no crash", async () => {
    const redis = { get: vi.fn(async () => { throw new Error("ECONNRESET"); }) };
    const res = await request(buildApp(redis)).get("/api/quote/anchor");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("400 invalid_chain_id on garbage", async () => {
    const res = await request(buildApp({ get: vi.fn() })).get("/api/quote/anchor?chain_id=abc");
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_chain_id");
  });
});

describe("POST /api/admin/quote/preview (EMIT-03)", () => {
  const post = (app: Express, body: unknown, token = ADMIN) =>
    request(app).post("/api/admin/quote/preview").set("x-arbx-admin-token", token).send(body);

  it("401 without the admin token", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => SNAPSHOT) }), { chain_id: 1, weights: KEEP_W }, "wrong");
    expect(res.status).toBe(401);
    expect(res.body.error).toBe("unauthorized");
  });

  it("400 invalid_weights when an axis is missing", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => SNAPSHOT) }), {
      chain_id: 1,
      weights: { prior: 1, liquidity: 0, venues: 0, stability: 0 },
    });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_weights");
  });

  it("400 invalid_weights_sum outside the 1e-9 epsilon", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => SNAPSHOT) }), {
      chain_id: 1,
      weights: { prior: 0.5, liquidity: 0.2, venues: 0.1, stability: 0.1, cross_dex: 0.05 },
    });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_weights_sum");
  });

  it("503 quote_anchor_not_published when the snapshot is absent", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => null) }), { chain_id: 1, weights: KEEP_W });
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("quote_anchor_not_published");
  });

  it("re-ranks under proposed weights: anchor change flips revaluation + footprints + version", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => SNAPSHOT) }), {
      chain_id: 1,
      weights: CHANGE_W,
    });
    expect(res.status).toBe(200);
    // Envelope: impact + the three §10 proposed fields — nothing else.
    expect(Object.keys(res.body).sort()).toEqual(
      ["impact", "proposed_quote_symbol", "proposed_quote_score", "proposed_tokens"].sort(),
    );
    const impact = res.body.impact;
    expect(Object.keys(impact).sort()).toEqual(
      [
        "graph_rebuild_required",
        "quote_revaluation_required",
        "quote_cache_invalidation_required",
        "affected_pairs",
        "affected_edges",
        "affected_cached_routes",
        "current_quote_version",
        "proposed_quote_version",
        "topology_version_unchanged",
      ].sort(),
    );
    // QB-TOPOLOGY-01 doctrine literals.
    expect(impact.graph_rebuild_required).toBe(false);
    expect(impact.topology_version_unchanged).toBe(true);
    // USDC → WETH: revaluation + both anchors' sidecar footprints + bump.
    expect(impact.quote_revaluation_required).toBe(true);
    expect(impact.quote_cache_invalidation_required).toBe(true);
    expect(impact.affected_pairs).toBe(42 + 17);
    expect(impact.affected_edges).toBe(60 + 25);
    expect(impact.affected_cached_routes).toBe(0);
    expect(impact.current_quote_version).toBe(3);
    expect(impact.proposed_quote_version).toBe(4);
    // §10 sketch fields: the new head + payload-ordered rows (§79 — FE never
    // recomputes; it renders this order).
    expect(res.body.proposed_quote_symbol).toBe("WETH");
    expect(res.body.proposed_quote_score).toBeCloseTo(100.0, 9);
    expect(res.body.proposed_tokens[0].symbol).toBe("WETH");
    expect(res.body.proposed_tokens[1].symbol).toBe("USDC");
    expect(res.body.proposed_tokens[1].score).toBeCloseTo(
      0.5 * 95 + 0.2 * 90 + 0.3 * 95,
      9,
    );
  });

  it("no anchor change: honest zeros, no fake churn, version unchanged", async () => {
    const res = await post(buildApp({ get: vi.fn(async () => SNAPSHOT) }), {
      chain_id: 1,
      weights: KEEP_W,
    });
    expect(res.status).toBe(200);
    const impact = res.body.impact;
    expect(impact.quote_revaluation_required).toBe(false);
    expect(impact.quote_cache_invalidation_required).toBe(false);
    expect(impact.affected_pairs).toBe(0);
    expect(impact.affected_edges).toBe(0);
    expect(impact.proposed_quote_version).toBe(3);
    expect(res.body.proposed_quote_symbol).toBe("USDC");
    expect(res.body.proposed_quote_score).toBeCloseTo(100.0, 9);
  });

  it("deterministic tie-break: equal scores fall through symbol asc → address asc", async () => {
    // Two rows with IDENTICAL components under prior-only weights tie at 100;
    // the sort must be stable-by-construction (AAA before ZZZ).
    const tied = JSON.stringify({
      ...SNAPSHOT_OBJ,
      tokens: [
        { symbol: "ZZZ", address: A41, components: USDC_C, score: 96.0 },
        { symbol: "AAA", address: B41, components: USDC_C, score: 96.0 },
      ],
    });
    const res = await post(buildApp({ get: vi.fn(async () => tied) }), {
      chain_id: 1,
      weights: KEEP_W,
    });
    expect(res.status).toBe(200);
    expect(res.body.proposed_tokens.map((t: { symbol: string }) => t.symbol)).toEqual(["AAA", "ZZZ"]);
  });
});
