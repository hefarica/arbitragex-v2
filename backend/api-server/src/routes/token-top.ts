// Token Top-N ranking route — operator preset menu (PC-07, 2026-09-19).
//
//   GET /api/tokens/top?chain_id=1&limit=20|30|40|100&window=current|24h&rules=a,b,c
//
// Ranks tokens by ON-CHAIN pool value (RULE 00: no fabricated market cap —
// circulating supply data does not exist in this system). TVL per token =
// Σ (token-side reserve × sovereign price) over the chain's ACTIVE pools;
// prices come from the Redis price tower `arbx:token_prices:1` (symbol→USD).
//
// window=24h ranks by the latest pool_reserves snapshot ≤ now−24h valued at
// CURRENT prices (honest caveat served in `window_note`: capital flows only,
// no historical price series exists).
//
// The 10 rug/scam screening rules are evaluated SERVER-SIDE against real
// tables only (token_safety_cache, token_validations, pools, tokens). A rule
// whose metric is NULL for a token is reported in `unverified_rules` — it
// never fabricates a pass or a fail (R8).
//
// Discovery doctrine (operator 2026-09-19): this endpoint only feeds the
// REFLECTION allowlist presets; route discovery stays universe-wide.

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";

export const TOKEN_PRICES_KEY = "arbx:token_prices:1";

export const TOP_LIMITS = [20, 30, 40, 100] as const;
export type TopLimit = (typeof TOP_LIMITS)[number];
export type TopWindow = "current" | "24h";

/** Canonical rug-rule catalog — IDs are the wire contract for `&rules=`. */
export const RUG_RULES = [
  { id: "min_safety_score", label: "Safety score ≥ 40 (token_safety_cache / validaciones)" },
  { id: "validated_status", label: "final_status ∈ {VERIFIED, VIABLE} (excluye ILLIQUID/NO_DATA)" },
  { id: "min_liquidity_usd", label: "Liquidez ≥ $50k (token_validations.liquidity_usd)" },
  { id: "vol_liq_ratio_cap", label: "Volumen 24h / liquidez ≤ 10 (churn/wash-trade)" },
  { id: "pump_5m_spike", label: "Volumen 5m ≤ 30% del volumen 1h (pump repentino)" },
  { id: "registry_verified", label: "Contrato verificado en registry (explorador)" },
  { id: "risk_level_ok", label: "tokens.risk_level ∉ {HIGH, BLOCKED}" },
  { id: "multi_pool", label: "≥ 2 pools activos (concentración de liquidez)" },
  { id: "pool_age_24h", label: "Pool más antiguo del token ≥ 24h de edad" },
  { id: "exclude_memecoins", label: "Excluir memecoins (heurística de nombre/símbolo)" },
] as const;
export type RugRuleId = (typeof RUG_RULES)[number]["id"];

const MEMECOIN_PATTERNS = [
  "PEPE", "DOGE", "SHIB", "FLOKI", "BABY", "ELON", "MOON", "INU", "WOJAK",
  "BONK", "TURBO", "MEME", "PUMP", "CAT", "FROG", "DOGWIF", "NEIRO", "PEIPEI",
];

function isMemecoin(symbol: string | null, name: string | null): boolean {
  const hay = `${symbol ?? ""} ${name ?? ""}`.toUpperCase();
  if (hay.trim().length === 0) return false;
  return MEMECOIN_PATTERNS.some((p) => hay.includes(p));
}

interface PoolRow {
  id: string;
  created_at: Date;
  s0: string | null; a0: string; r0: string | null; st0: boolean;
  s1: string | null; a1: string; r1: string | null; st1: boolean;
}

interface ReserveRow {
  pool_id: string;
  reserve0: string;
  reserve1: string;
}

interface ValidationRow {
  address: string;
  liquidity_usd: string | null;
  volume_24h_usd: string | null;
  volume_1h_usd: string | null;
  volume_5m_usd: string | null;
  registry_verified: boolean | null;
  final_status: string | null;
  score: number | null;
}

interface SafetyRow {
  token_address: string;
  safety_score: number;
}

interface TokenAgg {
  symbol: string | null;
  address: string;
  risk_level: string | null;
  is_stablecoin: boolean;
  pool_count: number;
  oldest_pool_at: Date;
  tvl_current: number;
  tvl_24h: number | null;
  val: ValidationRow | null;
  safety: number | null;
}

export interface TopTokenRow {
  symbol: string | null;
  address: string;
  tvl_usd: string;
  tvl_24h_ago_usd: string | null;
  pool_count: number;
  safety_score: number | null;
  validation_score: number | null;
  final_status: string | null;
  liquidity_usd: string | null;
  volume_24h_usd: string | null;
  vol_liq_ratio: number | null;
  registry_verified: boolean | null;
  oldest_pool_age_hours: number | null;
  risk_level: string | null;
  is_stablecoin: boolean;
  memecoin_heuristic: boolean;
  failed_rules: RugRuleId[];
  unverified_rules: RugRuleId[];
}

/**
 * Pure evaluator — exported for direct testing. A rule with a NULL metric is
 * `unverified`, never a pass/fail fabrication (R8).
 */
export function evaluateRugRules(
  agg: TokenAgg,
  rules: ReadonlySet<RugRuleId>,
): { failed: RugRuleId[]; unverified: RugRuleId[] } {
  const failed: RugRuleId[] = [];
  const unverified: RugRuleId[] = [];
  const v = agg.val;

  if (rules.has("min_safety_score")) {
    const s = agg.safety ?? v?.score ?? null;
    if (s === null) unverified.push("min_safety_score");
    else if (s < 40) failed.push("min_safety_score");
  }
  if (rules.has("validated_status")) {
    if (!v || v.final_status === null) unverified.push("validated_status");
    else if (v.final_status !== "VERIFIED" && v.final_status !== "VIABLE")
      failed.push("validated_status");
  }
  if (rules.has("min_liquidity_usd")) {
    if (!v || v.liquidity_usd === null) unverified.push("min_liquidity_usd");
    else if (Number(v.liquidity_usd) < 50_000) failed.push("min_liquidity_usd");
  }
  if (rules.has("vol_liq_ratio_cap")) {
    if (!v || v.volume_24h_usd === null || v.liquidity_usd === null || Number(v.liquidity_usd) === 0)
      unverified.push("vol_liq_ratio_cap");
    else if (Number(v.volume_24h_usd) / Number(v.liquidity_usd) > 10)
      failed.push("vol_liq_ratio_cap");
  }
  if (rules.has("pump_5m_spike")) {
    if (!v || v.volume_5m_usd === null || v.volume_1h_usd === null || Number(v.volume_1h_usd) === 0)
      unverified.push("pump_5m_spike");
    else if (Number(v.volume_5m_usd) / Number(v.volume_1h_usd) > 0.3)
      failed.push("pump_5m_spike");
  }
  if (rules.has("registry_verified")) {
    if (!v || v.registry_verified === null) unverified.push("registry_verified");
    else if (!v.registry_verified) failed.push("registry_verified");
  }
  if (rules.has("risk_level_ok")) {
    if (agg.risk_level === null) unverified.push("risk_level_ok");
    else if (agg.risk_level === "HIGH" || agg.risk_level === "BLOCKED")
      failed.push("risk_level_ok");
  }
  if (rules.has("multi_pool")) {
    if (agg.pool_count < 2) failed.push("multi_pool");
  }
  if (rules.has("pool_age_24h")) {
    const ageH = (Date.now() - agg.oldest_pool_at.getTime()) / 3_600_000;
    if (ageH < 24) failed.push("pool_age_24h");
  }
  if (rules.has("exclude_memecoins")) {
    if (isMemecoin(agg.symbol, null)) failed.push("exclude_memecoins");
  }
  return { failed, unverified };
}

interface Deps {
  pool: Pool | null;
  redis: Redis;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
}

export function buildTokenTopRouter(deps: Deps): Router {
  const r = Router();

  r.get("/api/tokens/top", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    const limitRaw = Number(req.query["limit"] ?? 20);
    const limit = (TOP_LIMITS as readonly number[]).includes(limitRaw) ? (limitRaw as TopLimit) : null;
    if (limit === null) {
      res.status(400).json({ error: "invalid_limit", allowed: TOP_LIMITS });
      return;
    }
    const window = req.query["window"] === "24h" ? "24h" : "current";
    const validRuleIds = new Set<string>(RUG_RULES.map((x) => x.id));
    const rules = new Set(
      String(req.query["rules"] ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter((s) => validRuleIds.has(s)),
    ) as Set<RugRuleId>;

    try {
      // 1) Active pools + token identity for the chain.
      const poolsQ = await deps.pool.query<PoolRow>(
        `SELECT p.id, p.created_at,
                t0.symbol AS s0, t0.address AS a0, t0.risk_level AS r0, t0.is_stablecoin AS st0,
                t1.symbol AS s1, t1.address AS a1, t1.risk_level AS r1, t1.is_stablecoin AS st1
           FROM pools p
           JOIN tokens t0 ON t0.id = p.token0_id
           JOIN tokens t1 ON t1.id = p.token1_id
          WHERE p.chain_id = $1 AND p.is_active = TRUE`,
        [chainId],
      );
      if (poolsQ.rowCount === 0) {
        res.status(200).json({
          chain_id: chainId, window, limit, ranked: [], excluded: {},
          price_coverage: { symbols_priced: 0, symbols_total: 0 },
          window_note: null, rules_applied: [...rules],
        });
        return;
      }
      const poolIds = poolsQ.rows.map((p) => p.id);

      // 2) Latest reserves now, and latest ≤ 24h ago (same query, two cutoffs).
      const cutoff = new Date(Date.now() - 24 * 3_600_000);
      const [nowQ, agoQ] = await Promise.all([
        deps.pool.query<ReserveRow>(
          `SELECT DISTINCT ON (pool_id) pool_id, reserve0, reserve1
             FROM pool_reserves
            WHERE pool_id = ANY($1::uuid[])
            ORDER BY pool_id, block_number DESC`,
          [poolIds],
        ),
        deps.pool.query<ReserveRow>(
          `SELECT DISTINCT ON (pool_id) pool_id, reserve0, reserve1
             FROM pool_reserves
            WHERE pool_id = ANY($1::uuid[]) AND "timestamp" <= $2
            ORDER BY pool_id, block_number DESC`,
          [poolIds, cutoff],
        ),
      ]);
      const nowRes = new Map(nowQ.rows.map((x) => [x.pool_id, x]));
      const agoRes = new Map(agoQ.rows.map((x) => [x.pool_id, x]));

      // 3) Sovereign price tower (symbol → USD).
      let prices: Record<string, string> = {};
      try {
        prices = await deps.redis.hgetall(TOKEN_PRICES_KEY);
      } catch {
        // no tower = no ranking possible; honest empty with reason
      }
      const priceOf = (sym: string | null): number | null => {
        if (!sym) return null;
        const p = prices[sym.toUpperCase()];
        if (p === undefined) return null;
        const n = Number(p);
        return Number.isFinite(n) && n > 0 ? n : null;
      };

      // 4) Rug-screening tables.
      const [valQ, safeQ] = await Promise.all([
        deps.pool.query<ValidationRow>(
          `SELECT address, liquidity_usd, volume_24h_usd, volume_1h_usd, volume_5m_usd,
                  registry_verified, final_status, score
             FROM token_validations
            WHERE chain_id = $1 AND stale_after > NOW()`,
          [chainId],
        ),
        deps.pool.query<SafetyRow>(
          `SELECT token_address, safety_score
             FROM token_safety_cache
            WHERE chain_id = $1 AND ttl_expires_at > NOW()`,
          [chainId],
        ),
      ]);
      const valByAddr = new Map(valQ.rows.map((x) => [x.address.toLowerCase(), x]));
      const safeByAddr = new Map(safeQ.rows.map((x) => [x.token_address.toLowerCase(), x]));

      // 5) Aggregate per token.
      const aggByAddr = new Map<string, TokenAgg>();
      const ensure = (
        addr: string, sym: string | null, risk: string | null, stable: boolean, poolAt: Date,
      ): TokenAgg => {
        let a = aggByAddr.get(addr);
        if (!a) {
          a = {
            symbol: sym, address: addr, risk_level: risk, is_stablecoin: stable,
            pool_count: 0, oldest_pool_at: poolAt, tvl_current: 0, tvl_24h: null,
            val: valByAddr.get(addr.toLowerCase()) ?? null,
            safety: safeByAddr.get(addr.toLowerCase())?.safety_score ?? null,
          };
          aggByAddr.set(addr, a);
        }
        a.pool_count += 1;
        if (poolAt < a.oldest_pool_at) a.oldest_pool_at = poolAt;
        return a;
      };

      const excluded: Record<string, number> = {};
      for (const p of poolsQ.rows) {
        const a0 = ensure(p.a0, p.s0, p.r0, p.st0, p.created_at);
        const a1 = ensure(p.a1, p.s1, p.r1, p.st1, p.created_at);
        const p0 = priceOf(p.s0);
        const p1 = priceOf(p.s1);
        if (p0 === null) excluded["no_price"] = (excluded["no_price"] ?? 0) + 1;
        else { a0.tvl_current += Number(nowRes.get(p.id)?.reserve0 ?? 0) * p0; a0.tvl_24h = (a0.tvl_24h ?? 0) + Number(agoRes.get(p.id)?.reserve0 ?? 0) * p0; }
        if (p1 === null) excluded["no_price"] = (excluded["no_price"] ?? 0) + 1;
        else { a1.tvl_current += Number(nowRes.get(p.id)?.reserve1 ?? 0) * p1; a1.tvl_24h = (a1.tvl_24h ?? 0) + Number(agoRes.get(p.id)?.reserve1 ?? 0) * p1; }
      }

      // 6) Rank by the chosen window's TVL; unpriced tokens cannot rank.
      // Unpriced tokens cannot rank (no fabricated value — RULE 00).
      const candidates = [...aggByAddr.values()].filter((a) => priceOf(a.symbol) !== null);
      const distinctPriced = candidates.length;

      const rankedAll: TopTokenRow[] = [];
      for (const a of candidates) {
        const { failed, unverified } = evaluateRugRules(a, rules);
        if (failed.length > 0) {
          for (const f of failed) excluded[`rug:${f}`] = (excluded[`rug:${f}`] ?? 0) + 1;
          continue;
        }
        const liq = a.val?.liquidity_usd == null ? null : Number(a.val.liquidity_usd);
        const vol24 = a.val?.volume_24h_usd == null ? null : Number(a.val.volume_24h_usd);
        rankedAll.push({
          symbol: a.symbol,
          address: a.address,
          tvl_usd: a.tvl_current.toFixed(2),
          tvl_24h_ago_usd: a.tvl_24h === null ? null : a.tvl_24h.toFixed(2),
          pool_count: a.pool_count,
          safety_score: a.safety ?? a.val?.score ?? null,
          validation_score: a.val?.score ?? null,
          final_status: a.val?.final_status ?? null,
          liquidity_usd: liq === null ? null : liq.toFixed(2),
          volume_24h_usd: vol24 === null ? null : vol24.toFixed(2),
          vol_liq_ratio: liq && liq > 0 && vol24 !== null ? vol24 / liq : null,
          registry_verified: a.val?.registry_verified ?? null,
          oldest_pool_age_hours: Number(((Date.now() - a.oldest_pool_at.getTime()) / 3_600_000).toFixed(1)),
          risk_level: a.risk_level,
          is_stablecoin: a.is_stablecoin,
          memecoin_heuristic: isMemecoin(a.symbol, null),
          failed_rules: failed,
          unverified_rules: unverified,
        });
      }

      const rankValue = (row: TopTokenRow) =>
        window === "24h"
          ? row.tvl_24h_ago_usd === null
            ? -1
            : Number(row.tvl_24h_ago_usd)
          : Number(row.tvl_usd);
      rankedAll.sort((x, y) => rankValue(y) - rankValue(x));
      const ranked = rankedAll.slice(0, limit);

      deps.logger.info(
        {
          event: "tokens.top_served",
          chain_id: chainId, window, limit,
          pools: poolsQ.rowCount,
          tokens_considered: candidates.length,
          tokens_ranked: rankedAll.length,
          rules_applied: [...rules],
        },
        "token top-N ranking served",
      );

      res.status(200).json({
        chain_id: chainId,
        window,
        limit,
        ranked,
        excluded,
        price_coverage: {
          symbols_priced: distinctPriced,
          symbols_total: aggByAddr.size,
        },
        // R8 honesty: the 24h window values OLD reserves at CURRENT prices —
        // it measures capital flow, not historical market cap.
        window_note: window === "24h"
          ? "reservas de hace ≤24h valorizadas a precios ACTUALES (flujo de capital, no market cap histórico)"
          : null,
        rules_applied: [...rules],
        rule_catalog: RUG_RULES,
      });
    } catch (e) {
      deps.logger.warn({ event: "tokens.top_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  return r;
}
