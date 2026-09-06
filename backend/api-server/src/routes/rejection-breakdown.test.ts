/**
 * REJECT-BREAKDOWN-EXPORT-01 — unit tests for the grouped breakdown surface.
 *
 * Mirrors the archive-control.test.ts harness (vitest + express + supertest +
 * a fake pg pool). The fake pool answers by SQL shape, so the tests pin the
 * CONTRACT (response shape, casing merge, weighted averages, honest nulls,
 * flood symbol resolution, param validation) without a real PG.
 *
 * Cases:
 *   (a) invalid hours → 400 invalid_hours
 *   (b) hours out of range (>720) → 400
 *   (c) invalid chain_id → 400
 *   (d) pool null → 503 db_unavailable
 *   (e) happy path — casing-duplicate families MERGE (UnknownTokenPrice +
 *       unknown_token_price), share % computed vs rejected_rows, token flood
 *       ranked with resolved symbol, unknown addr → symbol null (R8)
 *   (f) avg accumulation is weighted by n across raw groups
 *   (g) query error → 503 query_failed
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express } from "express";
import request from "supertest";

const logger = { warn: vi.fn(), error: vi.fn() };

/** Fake pg pool answering by SQL shape: the raw-family GROUP BY, the totals
 * COUNT, and the tokens symbol lookup. The same vi.fn serves pool.query and
 * the connect() client (timedQuery runs BEGIN/SET LOCAL/COMMIT through it). */
function fakePool(opts: {
  raws?: Array<{
    family_raw: string;
    raw_reason: string;
    n: number;
    gross_n?: number;
    net_n?: number;
    avg_gross: string | null;
    avg_net: string | null;
  }>;
  totals?: { total: number; rejected: number };
  symbols?: Array<{ addr: string; symbol: string }>;
  fail?: boolean;
}) {
  const query = vi.fn(async (q: unknown) => {
    const text = typeof q === "string" ? q : ((q as { text?: string }).text ?? "");
    if (opts.fail) throw new Error("connection refused");
    if (text.includes("split_part(rejection_reason")) {
      // Real PG returns every selected column; COUNT(col) defaults to COUNT(*)
      // when the fixture does not model NULL-skipping.
      return { rows: (opts.raws ?? []).map((r) => ({ gross_n: r.n, net_n: r.n, ...r })) };
    }
    if (text.includes("COUNT(rejection_reason)")) {
      return { rows: [opts.totals ?? { total: 0, rejected: 0 }] };
    }
    if (text.includes("FROM tokens")) {
      return { rows: (opts.symbols ?? []).map((s) => ({ addr: s.addr, symbol: s.symbol })) };
    }
    return { rows: [] };
  });
  return {
    query,
    connect: async () => ({ query, release: () => {} }),
  };
}

async function buildApp(pool: unknown): Promise<Express> {
  const { buildRejectionBreakdownRouter } = await import("./rejection-breakdown.js");
  const app = express();
  app.use(
    buildRejectionBreakdownRouter({
      // biome-ignore lint/suspicious/noExplicitAny: test-only pool stand-in
      pool: (pool === null ? null : (pool as any)) as never,
      logger,
    }),
  );
  return app;
}

describe("normalizeRejectionFamily", () => {
  it("camelCase → snake_case (mirrors opportunity_emitter camel_to_snake)", async () => {
    const m = await import("./rejection-breakdown.js");
    expect(m.normalizeRejectionFamily("UnknownTokenPrice")).toBe("unknown_token_price");
    expect(m.normalizeRejectionFamily("NegativeNetProfit")).toBe("negative_net_profit");
    expect(m.normalizeRejectionFamily("TokenNotAllowed")).toBe("token_not_allowed");
  });

  it("already-snake families pass through unchanged", async () => {
    const m = await import("./rejection-breakdown.js");
    expect(m.normalizeRejectionFamily("gas_floor_breach")).toBe("gas_floor_breach");
  });
});

describe("GET /api/v1/rejections/breakdown", () => {
  it("(a) non-integer hours → 400 invalid_hours", async () => {
    const app = await buildApp(fakePool({}));
    const r = await request(app).get("/api/v1/rejections/breakdown?hours=abc");
    expect(r.status).toBe(400);
    expect(r.body.error).toBe("invalid_hours");
  });

  it("(b) hours above the 720 cap → 400", async () => {
    const app = await buildApp(fakePool({}));
    const r = await request(app).get("/api/v1/rejections/breakdown?hours=9999");
    expect(r.status).toBe(400);
    expect(r.body.error).toBe("invalid_hours");
  });

  it("(c) invalid chain_id → 400", async () => {
    const app = await buildApp(fakePool({}));
    const r = await request(app).get("/api/v1/rejections/breakdown?chain_id=x");
    expect(r.status).toBe(400);
    expect(r.body.error).toBe("invalid_chain_id");
  });

  it("(d) pool null → 503 db_unavailable (R6/R8 honest)", async () => {
    const app = await buildApp(null);
    const r = await request(app).get("/api/v1/rejections/breakdown");
    expect(r.status).toBe(503);
    expect(r.body.error).toBe("db_unavailable");
  });

  it("(e) happy path — casing merge, share, flood symbols, honest null", async () => {
    const app = await buildApp(
      fakePool({
        raws: [
          // The two live floods (XEN, AGLD) + a casing-duplicated family.
          { family_raw: "TokenNotAllowed", raw_reason: "TokenNotAllowed:0x32353a6c91143bfd6c7d363b546e62a9a2489a20", n: 20888, avg_gross: "12.5", avg_net: null },
          { family_raw: "TokenNotAllowed", raw_reason: "TokenNotAllowed:0x06450dee7fd2fb8e39061434babcfc05599a6fb8", n: 16968, avg_gross: "7.5", avg_net: null },
          { family_raw: "gas_floor_breach", raw_reason: "gas_floor_breach", n: 1904, avg_gross: "3", avg_net: "0.5" },
          { family_raw: "UnknownTokenPrice", raw_reason: "UnknownTokenPrice", n: 300, avg_gross: null, avg_net: null },
          { family_raw: "unknown_token_price", raw_reason: "unknown_token_price", n: 200, avg_gross: null, avg_net: null },
        ],
        totals: { total: 48397, rejected: 42260 },
        symbols: [{ addr: "0x32353a6c91143bfd6c7d363b546e62a9a2489a20", symbol: "AGLD" }],
      }),
    );
    const r = await request(app).get("/api/v1/rejections/breakdown?hours=24");
    expect(r.status).toBe(200);
    expect(r.body.ok).toBe(true);
    expect(r.body.kind).toBe("rejection_breakdown");
    expect(r.body.window_hours).toBe(24);
    expect(r.body.total_rows).toBe(48397);
    expect(r.body.rejected_rows).toBe(42260);
    expect(r.body.raw_groups_truncated).toBe(false); // 5 raw groups < 500 bound

    const fams: Array<{ family: string; count: number; share_pct_of_rejected: number; avg_gross_usd: number | null }> = r.body.families;
    expect(fams.map((f) => f.family)).toEqual([
      "token_not_allowed",
      "gas_floor_breach",
      "unknown_token_price", // casing duplicates MERGED (500 = 300+200)
    ]);
    expect(fams[0]!.count).toBe(37856);
    expect(fams[0]!.share_pct_of_rejected).toBeCloseTo(89.6, 1);
    expect(fams[2]!.count).toBe(500);
    // net avg honest: no row had net for the flood family → null, never 0.
    expect(fams[0]!.avg_net_usd).toBeNull();

    const flood: Array<{ address: string; symbol: string | null; count: number }> = r.body.token_flood;
    expect(flood).toHaveLength(2);
    expect(flood[0]).toMatchObject({ symbol: "AGLD", count: 20888 });
    expect(flood[1]).toMatchObject({ symbol: null, count: 16968 }); // unknown addr stays null (R8)
  });

  it("(f) avg merges weight by COUNT(col) — the rows that HAD the value — not COUNT(*)", async () => {
    const app = await buildApp(
      fakePool({
        raws: [
          // Group A: 100 rows, all with gross (avg 10). Group B: 300 rows but
          // only 100 carry gross (avg 2 — PG AVG is over the valued subset).
          // Combined over valued rows: (10×100 + 2×100)/200 = 6.
          // The OLD buggy weight (COUNT(*)) would give (10×100+2×300)/400 = 4.
          { family_raw: "TokenNotAllowed", raw_reason: "TokenNotAllowed:0x" + "a".repeat(40), n: 100, gross_n: 100, avg_gross: "10", avg_net: null },
          { family_raw: "TokenNotAllowed", raw_reason: "TokenNotAllowed:0x" + "b".repeat(40), n: 300, gross_n: 100, avg_gross: "2", avg_net: null },
        ],
        totals: { total: 400, rejected: 400 },
      }),
    );
    const r = await request(app).get("/api/v1/rejections/breakdown");
    expect(r.body.families[0]!.avg_gross_usd).toBe(6);
  });

  it("(i) exactly RAW_REASON_LIMIT raw groups → raw_groups_truncated=true (R8 honesty)", async () => {
    const raws = Array.from({ length: 500 }, (_, i) => ({
      family_raw: "gas_floor_breach",
      raw_reason: `r${i}`,
      n: 1,
      avg_gross: null,
      avg_net: null,
    }));
    const app = await buildApp(fakePool({ raws, totals: { total: 500, rejected: 500 } }));
    const r = await request(app).get("/api/v1/rejections/breakdown");
    expect(r.status).toBe(200);
    expect(r.body.raw_groups_truncated).toBe(true);
  });

  it("(j) chain_id above the int4 ceiling → 400 invalid_chain_id (not PG 503 noise)", async () => {
    const app = await buildApp(fakePool({}));
    const r = await request(app).get("/api/v1/rejections/breakdown?chain_id=99999999999");
    expect(r.status).toBe(400);
    expect(r.body.error).toBe("invalid_chain_id");
  });

  it("(g) PG error → 503 query_failed (never fabricated data)", async () => {
    const app = await buildApp(fakePool({ fail: true }));
    const r = await request(app).get("/api/v1/rejections/breakdown");
    expect(r.status).toBe(503);
    expect(r.body.error).toBe("query_failed");
  });

  it("(h) chain_id filter binds $2 — params match placeholders EXACTLY (no dangling $2)", async () => {
    const pool = fakePool({
      raws: [{ family_raw: "gas_floor_breach", raw_reason: "gas_floor_breach", n: 5, avg_gross: "1", avg_net: null }],
      totals: { total: 5, rejected: 5 },
    });
    const app = await buildApp(pool);
    const r = await request(app).get("/api/v1/rejections/breakdown?hours=24&chain_id=1");
    expect(r.status).toBe(200);
    expect(r.body.chain_id).toBe(1);
    // Both aggregate queries carry the chain clause AND the bound values
    // (query(text, params) two-arg form — params is the second call arg).
    const calls = pool.query.mock.calls as Array<[unknown, unknown[]?]>;
    expect(calls.length).toBeGreaterThanOrEqual(2);
    for (const [text, values] of calls) {
      if (typeof text === "string" && text.includes("FROM opportunities")) {
        expect(text).toContain(" AND chain_id = $2");
        expect(values).toEqual([24, 1]);
      }
    }
  });

  it("(h2) no chain_id → NO $2 in SQL, params = [hours] only", async () => {
    const pool = fakePool({
      raws: [],
      totals: { total: 0, rejected: 0 },
    });
    const app = await buildApp(pool);
    const r = await request(app).get("/api/v1/rejections/breakdown?hours=24");
    expect(r.status).toBe(200);
    expect(r.body.chain_id).toBeNull();
    const calls = pool.query.mock.calls as Array<[unknown, unknown[]?]>;
    for (const [text, values] of calls) {
      if (typeof text === "string" && text.includes("FROM opportunities")) {
        expect(text).not.toContain("$2");
        expect(values).toEqual([24]);
      }
    }
  });
});
