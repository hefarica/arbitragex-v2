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

/**
 * Ranking basis for the preset menu (operator 2026-10-21, D5).
 *   tvl        — Σ (token-side reserve × price), the PC-07 default.
 *   volume_24h — `token_validations.volume_24h_usd`, the only field in this
 *                system that measures *traded* volume over 24h.
 *
 * Why volume is a separate basis and not a window: `window=24h` re-values the
 * reserves snapshot from 24h ago at CURRENT prices — that is capital flow, not
 * volume. Ranking by it answers "where did liquidity move", not "where is the
 * trading". The operator asked for the latter.
 */
export const RANK_BASES = ["tvl", "volume_24h"] as const;
export type RankBy = (typeof RANK_BASES)[number];

/**
 * A pool-side valuation above $1T is a scale defect (decimals / price-tower
 * mismatch), not a market cap. Measured on chain 1 at 2026-10-21T05:20Z:
 * the #1 token was served with tvl_usd = 6.55e32. Such a row can never rank.
 */
export const IMPLAUSIBLE_TVL_USD = 1_000_000_000_000;

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
  s0: string | null; a0: string; r0: string | null; st0: boolean; d0: number | null;
  s1: string | null; a1: string; r1: string | null; st1: boolean; d1: number | null;
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
  tvl_current: number | null;
  tvl_24h: number | null;
  val: ValidationRow | null;
  safety: number | null;
}

export interface TopTokenRow {
  symbol: string | null;
  address: string;
  tvl_usd: string | null;
  tvl_24h_ago_usd: string | null;
  pool_count: number;
  cache_score: number | null;
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
  not_measured_rules: RugRuleId[];
}

/** Sort value for a row under the chosen basis; negative = cannot rank. */
export function rankValueOf(
  row: TopTokenRow,
  rankBy: RankBy,
  window: TopWindow = "current",
): number {
  if (rankBy === "volume_24h") {
    const v = row.volume_24h_usd === null ? null : Number(row.volume_24h_usd);
    return v !== null && Number.isFinite(v) && v > 0 ? v : -1;
  }
  if (window === "24h") {
    return row.tvl_24h_ago_usd === null ? -1 : Number(row.tvl_24h_ago_usd);
  }
  if (row.tvl_usd === null) return -1;
  const t = Number(row.tvl_usd);
  return Number.isFinite(t) ? t : -1;
}

/**
 * Final stage of the ranking, pure and testable: applies the two honesty
 * screens and sorts. Returns the excluded counters so the caller can merge
 * them with the rug-rule counters already accumulated.
 *
 * R8: a token with no measured 24h volume is NOT ranked as zero — it is
 * excluded and counted, because "no data" and "no volume" are different facts.
 */
export function screenAndRank(
  rows: readonly TopTokenRow[],
  rankBy: RankBy,
  limit: number,
  window: TopWindow = "current",
): { ranked: TopTokenRow[]; excluded: Record<string, number> } {
  const excluded: Record<string, number> = {};
  const kept: TopTokenRow[] = [];
  for (const row of rows) {
    const tvl = row.tvl_usd === null ? null : Number(row.tvl_usd);
    if (tvl !== null && Number.isFinite(tvl) && tvl > IMPLAUSIBLE_TVL_USD) {
      excluded["implausible_tvl"] = (excluded["implausible_tvl"] ?? 0) + 1;
      continue;
    }
    if (tvl === null) {
      // Null is not zero: a token with no reservable value is counted, not ranked.
      excluded["no_reserve"] = (excluded["no_reserve"] ?? 0) + 1;
      continue;
    }
    if (row.tvl_24h_ago_usd === null) {
      excluded["no_24h_snapshot"] = (excluded["no_24h_snapshot"] ?? 0) + 1;
    }
    if (rankBy === "volume_24h" && rankValueOf(row, rankBy) < 0) {
      excluded["no_volume_24h"] = (excluded["no_volume_24h"] ?? 0) + 1;
      continue;
    }
    kept.push(row);
  }
  kept.sort((x, y) => {
    const d = rankValueOf(y, rankBy, window) - rankValueOf(x, rankBy, window);
    // Deterministic tie-break: TVL, then address — no stable-sort luck.
    if (d !== 0) return d;
    const tvl = (y.tvl_usd === null ? 0 : Number(y.tvl_usd)) - (x.tvl_usd === null ? 0 : Number(x.tvl_usd));
    if (Number.isFinite(tvl) && tvl !== 0) return tvl;
    return x.address.localeCompare(y.address);
  });
  return { ranked: kept.slice(0, limit), excluded };
}

/** Human-readable basis for the served ranking (R8: the caller must see it). */
export function rankNote(rankBy: RankBy): string {
  return rankBy === "volume_24h"
    ? "orden = volumen 24h (token_validations.volume_24h_usd); TVL sólo como columna de contexto"
    : "orden = valor en pools × torre de precios (flujo de capital, no market cap)";
}

/** Served note for a NOT COMPUTED selection: names the basis and the cause. */
export function selectionNote(rankBy: RankBy, gate: VolumeGate): string {
  const cause =
    gate.selection_reason === "volume_as_of_absent"
      ? "token_validations.volume_as_of no existe: no hay edad de snapshot de volumen"
      : gate.selection_reason === "no_fresh_volume_snapshot"
        ? `0 filas con volumen y volume_as_of <= ${VOLUME_SNAPSHOT_MAX_AGE_HOURS} h`
        : gate.selection_reason === "volume_as_of_probe_failed"
          ? "no se pudo comprobar volume_as_of"
          : "no computable";
  return `selection = NOT COMPUTED (${cause}); no se sustituye por TVL (${rankNote(rankBy)})`;
}

/**
 * Freshness gate for the volume basis (contract §2(i)(ii), ratified 2026-10-21).
 *
 * `volume_24h_usd` lives in `token_validations` but has NO snapshot timestamp
 * there: `validated_at` is the validator's run, not the DEX Screener snapshot.
 * Ordering by a volume of unknown age is not a ranking, it is an appearance of
 * one. Until `volume_as_of` exists (PR #1004) and is populated, `rank_by=
 * volume_24h` answers NOT COMPUTED — and it NEVER falls back to TVL in
 * silence: substituting the definition would hide the gap.
 */
export const VOLUME_SNAPSHOT_MAX_AGE_HOURS = 6;

export type SelectionState = "MEASURED" | "NOT COMPUTED";

export interface VolumeGateInput {
  rankBy: RankBy;
  columnPresent: boolean;
  probeFailed: boolean;
  orderable: number;
}

export interface VolumeGate {
  selection_state: SelectionState;
  selection_reason: string | null;
  volume_orderable: number;
}

export function evaluateVolumeGate(i: VolumeGateInput): VolumeGate {
  if (i.rankBy !== "volume_24h") {
    return { selection_state: "MEASURED", selection_reason: null, volume_orderable: i.orderable };
  }
  if (i.probeFailed) {
    return { selection_state: "NOT COMPUTED", selection_reason: "volume_as_of_probe_failed", volume_orderable: 0 };
  }
  if (!i.columnPresent) {
    return { selection_state: "NOT COMPUTED", selection_reason: "volume_as_of_absent", volume_orderable: 0 };
  }
  if (i.orderable === 0) {
    return { selection_state: "NOT COMPUTED", selection_reason: "no_fresh_volume_snapshot", volume_orderable: 0 };
  }
  return { selection_state: "MEASURED", selection_reason: null, volume_orderable: i.orderable };
}

/**
 * Rules that measure something other than what they claim are diagnostics: they
 * never reject and never count as a verified rule (ratified §5, 2026-10-21).
 */
export const DIAGNOSTIC_RULES: ReadonlySet<RugRuleId> = new Set<RugRuleId>(["pool_age_24h"]);

export interface RulePopulation {
  evaluated: number;
  unverified: number;
  diagnostic: boolean;
}

export interface SaveGate {
  allowed: boolean;
  reasons: string[];
}

/**
 * Write lock (ratified decision 3, 2026-10-21). The ranking may still be served
 * with NOT COMPUTED rows, but the allowlist is a WRITE: it cannot be saved when
 * (a) no rule has a verifiable population, or (b) the selection is not computed.
 * The reason travels to the operator — a disabled button without a reason sends
 * them looking for a bypass.
 */
export function computeSaveGate(input: {
  selectionState: SelectionState;
  rulePopulation: Record<string, RulePopulation>;
  rulesApplied?: readonly RugRuleId[];
}): SaveGate {
  const reasons: string[] = [];
  if (input.selectionState !== "MEASURED") reasons.push("selection_not_computed");
  const counted = Object.entries(input.rulePopulation).filter(([, p]) => !p.diagnostic);
  const applied = (input.rulesApplied ?? []).filter((r) => !DIAGNOSTIC_RULES.has(r));
  if (applied.length === 0) {
    // No rule requested at all, or only rules that cannot be measured: the
    // second case is a population problem, not an empty selection.
    reasons.push(
      Object.keys(input.rulePopulation).length === 0 ? "no_rules_applied" : "no_verified_rule_population",
    );
  } else if (counted.every(([, p]) => p.evaluated === 0)) {
    reasons.push("no_verified_rule_population");
  }
  return { allowed: reasons.length === 0, reasons };
}

/**
 * Pure evaluator — exported for direct testing. A rule with a NULL metric is
 * `unverified`, never a pass/fail fabrication (R8).
 */
export function evaluateRugRules(
  agg: TokenAgg,
  rules: ReadonlySet<RugRuleId>,
): { failed: RugRuleId[]; unverified: RugRuleId[]; notMeasured: RugRuleId[] } {
  const failed: RugRuleId[] = [];
  const unverified: RugRuleId[] = [];
  const notMeasured: RugRuleId[] = [];
  const v = agg.val;

  // D-SEL-6 (decision 7, 2026-10-21): two instruments, two fields. The
  // threshold applies ONLY to validations.score; token_safety_cache.safety_score
  // is published as `cache_score` and never participates. Blending them under
  // one threshold made the rule mean neither instrument.
  if (rules.has("min_safety_score")) {
    const s = v?.score ?? null;
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
  // D-SEL-5 (decision 6, 2026-10-21): `pools.created_at` is the INSERTION
  // time, not the on-chain pool creation. A re-ingest resets it and would
  // evict old pools; a rule that measures something else must not reject
  // anyone. It is reported as NOT MEASURED and never counts as verified.
  if (rules.has("pool_age_24h")) notMeasured.push("pool_age_24h");
  if (rules.has("exclude_memecoins")) {
    if (isMemecoin(agg.symbol, null)) failed.push("exclude_memecoins");
  }
  return { failed, unverified, notMeasured };
}

/**
 * Detect `token_validations.volume_as_of` the way the SQL check does
 * (information_schema), never with a try/catch that reports green: a failed
 * probe is its own cause, distinct from "column absent".
 */
async function probeVolumeAsOf(
  pool: Pool,
  chainId: number,
): Promise<{ columnPresent: boolean; probeFailed: boolean; orderable: number }> {
  try {
    const col = await pool.query<{ present: number }>(
      `SELECT count(*)::int AS present
         FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'token_validations'
          AND column_name = 'volume_as_of'`,
    );
    const present = Number(col.rows[0]?.present ?? 0) > 0;
    if (!present) return { columnPresent: false, probeFailed: false, orderable: 0 };
    const q = await pool.query<{ orderable: number }>(
      `SELECT count(*)::int AS orderable
         FROM token_validations
        WHERE chain_id = $1
          AND volume_24h_usd IS NOT NULL AND volume_24h_usd > 0
          AND volume_as_of IS NOT NULL
          AND volume_as_of >= NOW() - INTERVAL '6 hours'`,
      [chainId],
    );
    return { columnPresent: true, probeFailed: false, orderable: Number(q.rows[0]?.orderable ?? 0) };
  } catch {
    return { columnPresent: false, probeFailed: true, orderable: 0 };
  }
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
    const rankByRaw = String(req.query["rank_by"] ?? "tvl");
    if (!(RANK_BASES as readonly string[]).includes(rankByRaw)) {
      res.status(400).json({ error: "invalid_rank_by", allowed: RANK_BASES });
      return;
    }
    const rankBy = rankByRaw as RankBy;
    const validRuleIds = new Set<string>(RUG_RULES.map((x) => x.id));
    const rules = new Set(
      String(req.query["rules"] ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter((s) => validRuleIds.has(s)),
    ) as Set<RugRuleId>;

    const volumeProbe = await probeVolumeAsOf(deps.pool, chainId);
    const gate = evaluateVolumeGate({ rankBy, ...volumeProbe });

    try {
      if (gate.selection_state === "NOT COMPUTED") {
        res.status(200).json({
          chain_id: chainId,
          window,
          limit,
          ranked: [],
          excluded: {},
          price_coverage: { symbols_priced: 0, symbols_total: 0 },
          selection_state: gate.selection_state,
          selection_reason: gate.selection_reason,
          volume_orderable: gate.volume_orderable,
          save_gate: { allowed: false, reasons: ["selection_not_computed"] },
          rule_population: {},
          window_note: selectionNote(rankBy, gate),
          rules_applied: [...rules],
          rule_catalog: RUG_RULES,
        });
        return;
      }

      // 1) Active pools + token identity for the chain.
      const poolsQ = await deps.pool.query<PoolRow>(
        `SELECT p.id, p.created_at,
                t0.symbol AS s0, t0.address AS a0, t0.risk_level AS r0, t0.is_stablecoin AS st0,
                t0.decimals AS d0,
                t1.symbol AS s1, t1.address AS a1, t1.risk_level AS r1, t1.is_stablecoin AS st1,
                t1.decimals AS d1
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
          selection_state: "NOT COMPUTED",
          selection_reason: "no_active_pools",
          volume_orderable: gate.volume_orderable,
          save_gate: { allowed: false, reasons: ["selection_not_computed"] },
          rule_population: {},
          window_note: `sin pools activos para chain ${chainId}: no hay universo que ordenar (${rankNote(rankBy)})`,
          rules_applied: [...rules],
        });
        return;
      }
      const poolIds = poolsQ.rows.map((p) => p.id);

      // 2) Latest reserves now, and latest ≤ 24h ago (same query, two cutoffs).
      //    ARBX-F13 (2026-10-18): `DISTINCT ON (pool_id) … ORDER BY pool_id,
      //    block_number DESC` se degradaba a Parallel Bitmap Heap Scan sobre
      //    pool_reserves (404.372 tids, 375.117 filas; 26 s medidos y 74–174 s
      //    en vivo), agotando /dev/shm con dos peticiones concurrentes y
      //    colgando el endpoint >30 s (500 en el edge). El LATERAL con LIMIT 1
      //    usa un index scan por pool (≤50 sondas) y devuelve EL MISMO
      //    registro: la fila de mayor block_number por pool, con el mismo
      //    filtro de timestamp en la variante de 24 h.
      const cutoff = new Date(Date.now() - 24 * 3_600_000);
      const [nowQ, agoQ] = await Promise.all([
        deps.pool.query<ReserveRow>(
          `SELECT p.pool_id, r.reserve0, r.reserve1
             FROM unnest($1::uuid[]) AS p(pool_id)
             CROSS JOIN LATERAL (
               SELECT rr.reserve0, rr.reserve1
                 FROM pool_reserves rr
                WHERE rr.pool_id = p.pool_id
                ORDER BY rr.block_number DESC
                LIMIT 1
             ) r`,
          [poolIds],
        ),
        deps.pool.query<ReserveRow>(
          `SELECT p.pool_id, r.reserve0, r.reserve1
             FROM unnest($1::uuid[]) AS p(pool_id)
             CROSS JOIN LATERAL (
               SELECT rr.reserve0, rr.reserve1
                 FROM pool_reserves rr
                WHERE rr.pool_id = p.pool_id AND rr."timestamp" <= $2
                ORDER BY rr.block_number DESC
                LIMIT 1
             ) r`,
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
            pool_count: 0, oldest_pool_at: poolAt, tvl_current: null, tvl_24h: null,
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
      // R8/scale: a reserve is a RAW integer. Valuing it without dividing by
      // 10^decimals inflated chain-1 rankings to 1e32 USD (measured
      // 2026-10-21T05:20Z: ADI at 6.55e32). No decimals = no value, counted —
      // never a guessed 18.
      // D-SEL-2 (decision 5, 2026-10-21): a pool with NO reserve row is not a
      // pool holding 0 — coalescing it to 0 fabricates a valuation and keeps the
      // token rankable at "0.00". No reserve row => no value for that side.
      const valueOf = (raw: string | undefined, decimals: number | null, price: number | null): number | null => {
        if (raw === undefined) return null;
        if (price === null || decimals === null || !Number.isFinite(decimals)) return null;
        const v = Number(raw) / 10 ** decimals;
        return Number.isFinite(v) ? v * price : null;
      };
      for (const p of poolsQ.rows) {
        const a0 = ensure(p.a0, p.s0, p.r0, p.st0, p.created_at);
        const a1 = ensure(p.a1, p.s1, p.r1, p.st1, p.created_at);
        const p0 = priceOf(p.s0);
        const p1 = priceOf(p.s1);
        const v0 = valueOf(nowRes.get(p.id)?.reserve0, p.d0, p0);
        const v0ago = valueOf(agoRes.get(p.id)?.reserve0, p.d0, p0);
        const v1 = valueOf(nowRes.get(p.id)?.reserve1, p.d1, p1);
        const v1ago = valueOf(agoRes.get(p.id)?.reserve1, p.d1, p1);
        if (p0 === null) excluded["no_price"] = (excluded["no_price"] ?? 0) + 1;
        else if (v0 === null) excluded["no_decimals"] = (excluded["no_decimals"] ?? 0) + 1;
        else {
          a0.tvl_current = (a0.tvl_current ?? 0) + v0;
          if (v0ago !== null) a0.tvl_24h = (a0.tvl_24h ?? 0) + v0ago;
        }
        if (p1 === null) excluded["no_price"] = (excluded["no_price"] ?? 0) + 1;
        else if (v1 === null) excluded["no_decimals"] = (excluded["no_decimals"] ?? 0) + 1;
        else {
          a1.tvl_current = (a1.tvl_current ?? 0) + v1;
          if (v1ago !== null) a1.tvl_24h = (a1.tvl_24h ?? 0) + v1ago;
        }
      }

      // 6) Rank by the chosen window's TVL; unpriced tokens cannot rank.
      // Unpriced tokens cannot rank (no fabricated value — RULE 00).
      const candidates = [...aggByAddr.values()].filter((a) => priceOf(a.symbol) !== null);
      const distinctPriced = candidates.length;

      const rankedAll: TopTokenRow[] = [];
      const rulePopulation: Record<string, RulePopulation> = {};
      for (const rule of rules) {
        rulePopulation[rule] = { evaluated: 0, unverified: 0, diagnostic: DIAGNOSTIC_RULES.has(rule) };
      }
      for (const a of candidates) {
        const { failed, unverified, notMeasured } = evaluateRugRules(a, rules);
        for (const rule of rules) {
          const rec = rulePopulation[rule]!;
          if (unverified.includes(rule) || notMeasured.includes(rule)) rec.unverified += 1;
          else rec.evaluated += 1;
        }
        if (failed.length > 0) {
          for (const f of failed) excluded[`rug:${f}`] = (excluded[`rug:${f}`] ?? 0) + 1;
          continue;
        }
        const liq = a.val?.liquidity_usd == null ? null : Number(a.val.liquidity_usd);
        const vol24 = a.val?.volume_24h_usd == null ? null : Number(a.val.volume_24h_usd);
        rankedAll.push({
          symbol: a.symbol,
          address: a.address,
          tvl_usd: a.tvl_current === null ? null : a.tvl_current.toFixed(2),
          tvl_24h_ago_usd: a.tvl_24h === null ? null : a.tvl_24h.toFixed(2),
          pool_count: a.pool_count,
          cache_score: a.safety,
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
          not_measured_rules: notMeasured,
        });
      }

      const screened = screenAndRank(rankedAll, rankBy, limit, window);
      for (const [k, v] of Object.entries(screened.excluded)) {
        excluded[k] = (excluded[k] ?? 0) + v;
      }
      const ranked = screened.ranked;

      deps.logger.info(
        {
          event: "tokens.top_served",
          chain_id: chainId, window, rank_by: rankBy, limit,
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
        selection_state: gate.selection_state,
        selection_reason: gate.selection_reason,
        volume_orderable: gate.volume_orderable,
        save_gate: computeSaveGate({ selectionState: gate.selection_state, rulePopulation, rulesApplied: [...rules] }),
        rule_population: rulePopulation,
        // R8 honesty: the served ordering must name its own basis. `window=24h`
        // values OLD reserves at CURRENT prices (capital flow, not historical
        // market cap); `rank_by=volume_24h` orders by measured traded volume.
        window_note: rankBy === "volume_24h"
          ? rankNote(rankBy)
          : window === "24h"
            ? "reservas de hace ≤24h valorizadas a precios ACTUALES (flujo de capital, no market cap histórico)"
            : rankNote(rankBy),
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
