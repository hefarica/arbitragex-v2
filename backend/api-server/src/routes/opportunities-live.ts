/**
 * Sprint 7 / Task 9 — /api/v1/opportunities/live route module.
 *
 * Extracted from index.ts inline handler (commit 97ffc52 baseline) and
 * enriched with LEFT JOIN tokens for nested token_in_info / token_out_info.
 *
 * Contract (CARDS-MIRROR-01, 2026-08-22):
 *   - viable_only=false (default): returns all recent rows including rejected ones
 *     (rejection_reason visible per-item). R8: rows are real PG rows, never fabricated.
 *   - viable_only=true: opt-in filter that excludes rows where rejection_reason IS NOT NULL.
 *   - rejection_reason field always present in each item (null or string).
 *   - HTTP 503 { error: "db_unavailable" } when pool is null.
 *   - HTTP 503 { error: "query_failed" } on PG error.
 *   - NEVER synthesize data (R8 fail-honest). NULL = no data. 0 = data is zero.
 *
 * Cross-chain token lookup:
 *   token_in  → tokens where chain_id = o.chain_id      AND address = LOWER(o.token_in)
 *   token_out → tokens where chain_id = COALESCE(o.chain_id_out, o.chain_id)
 *                                    AND address = LOWER(o.token_out)
 *   tokens.address is stored lowercase (CHECK constraint chk_address_format).
 *   LOWER() on o.token_{in,out} is belt-and-suspenders for uppercase inputs.
 */

import type { Application, Request, Response } from "express";
import type { Pool, QueryResultRow } from "pg";
import type { Redis } from "ioredis";
import { resolveTokensOnDemand, tokenCacheKey } from "./tokenResolver.js";

// CARDS-MIRROR-01 — the set of `opportunities.status` values that count as
// VIABLE (cards-visible when viable_only=true). Sourced from the schema CHECK
// constraint (migration 003_opportunities.sql:20) — the forward lifecycle states
// before a gate rejects. This is the single source of truth for the api-server;
// it MUST mirror `VIABLE_STATUSES` in backend/searcher-rs/src/persistence.rs so
// a row the searcher persists as viable is surfaced by this endpoint. A new
// schema state only needs adding in BOTH places (kept in sync by the
// cards-mirror regression test). No literals inline in the LIVE_QUERY SQL.
export const VIABLE_STATUSES = ["detected", "validated", "simulated", "scored"] as const;

// ── Token Symbol Cache ───────────────────────────────────────────────────────
// Cache en memoria para símbolos de tokens (reduce queries repetidas a DB)
// TTL: 60 segundos - balance entre frescura de datos y rendimiento

interface TokenSymbolCacheEntry {
  symbol: string;
  decimals: number;
  logo_url: string | null;
  timestamp: number;
}

const TOKEN_CACHE_TTL_MS = 60 * 1000; // 60 segundos
const tokenSymbolCache = new Map<string, TokenSymbolCacheEntry>();

/**
 * Resuelve símbolos de tokens con caching agresivo.
 * Evita queries repetidas a PostgreSQL para tokens ya resueltos recientemente.
 */
async function resolveTokenSymbolsWithCache(
  pool: Pool,
  tokenAddresses: Array<{ chain_id: number; address: string }>,
): Promise<Map<string, TokenSymbolCacheEntry>> {
  const results = new Map<string, TokenSymbolCacheEntry>();
  const toFetch: Array<{ chain_id: number; address: string }> = [];
  const now = Date.now();

  // Check cache primero
  for (const { chain_id, address } of tokenAddresses) {
    const cacheKey = tokenCacheKey(chain_id, address);
    const cached = tokenSymbolCache.get(cacheKey);

    if (cached && now - cached.timestamp < TOKEN_CACHE_TTL_MS) {
      results.set(cacheKey, cached);
    } else {
      toFetch.push({ chain_id, address });
    }
  }

  // Fetch solo los que no están en cache o expiraron
  if (toFetch.length > 0) {
    const query = `
      SELECT chain_id, address, symbol, decimals, logo_url
      FROM tokens
      WHERE (chain_id, address) IN (${toFetch
        .map((_, i) => `($${i * 2 + 1}, $${i * 2 + 2})`)
        .join(", ")})
    `;
    const params = toFetch.flatMap((t) => [t.chain_id, t.address.toLowerCase()]);

    try {
      const { rows } = await pool.query(query, params);

      for (const row of rows) {
        const entry: TokenSymbolCacheEntry = {
          symbol: row.symbol,
          decimals: row.decimals,
          logo_url: row.logo_url,
          timestamp: now,
        };
        const cacheKey = tokenCacheKey(row.chain_id, row.address);
        tokenSymbolCache.set(cacheKey, entry);
        results.set(cacheKey, entry);
      }
    } catch (e) {
      // Fail-honest: si falla, continuamos con lo que tengamos en cache
      console.warn("[TokenCache] DB query failed, using cached data only:", e);
    }
  }

  return results;
}

/**
 * Obtiene entrada de cache para un token específico
 */
function getCachedToken(
  chain_id: number,
  address: string,
): TokenSymbolCacheEntry | undefined {
  const cacheKey = tokenCacheKey(chain_id, address);
  const cached = tokenSymbolCache.get(cacheKey);

  if (cached && Date.now() - cached.timestamp < TOKEN_CACHE_TTL_MS) {
    return cached;
  }
  return undefined;
}
import { verifyToken } from "../services/tokenRegistry.js";
import {
  readTokenValidationsBatch,
  type TokenValidationRow,
} from "../services/tokenValidation/index.js";
import {
  getTradingConfigForChain,
  type TradingConfigSnapshot,
} from "../simulation/tradingConfigSnapshot.js";
import {
  forwardSimulate,
  inverseSize,
  resolveTarget,
  type InverseSizingResult,
  type SimulatedCostBreakdown,
  type SimulationResult,
  type SimulatorRow,
} from "../simulation/computeSimulatedNet.js";

// ── Types ────────────────────────────────────────────────────────────────────

interface TokenValidationBlock {
  status: TokenValidationRow["final_status"];
  score: number;
  liquidity_usd: number | null;
  volume_24h_usd: number | null;
  pair_count: number | null;
  primary_dex: string | null;
  registry_source: string | null;
  validated_at: string;
  reasons: Array<{ key: string; delta: number; note: string }> | null;
}

interface TokenInfoResult {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: string | null;
  /**
   * Curated-registry verification (Uniswap Labs Default Token List).
   * True only when address is in the curated list AND on-chain symbol
   * matches the registry's. The frontend renders "UNVERIFIED" badge for
   * any token where verified=false, defending the operator against
   * memecoins (`69420`), emoji-named impersonators (`🦈EMS`), and
   * contracts that lie about their symbol.
   */
  verified: boolean;
  registry_symbol: string | null;
  registry_name: string | null;
  verified_notes: string[];
  /**
   * Token Validation Engine Phase 1 — composite multi-validator result
   * (OnChainTruth + LiquidityReality + Registry). Replaces the binary
   * verified/unverified pill with a real safety score and status:
   *   VERIFIED | VIABLE | LOW_LIQUIDITY | ILLIQUID | NO_DATA | INVALID | PENDING.
   * The frontend renders one of seven coloured badges based on `status`
   * + `score`. Null when validation hasn't yet run (cold start).
   */
  validation: TokenValidationBlock | null;
}

interface OpportunityLiveRow extends QueryResultRow {
  id: string;
  chain_id: number;
  strategy_kind: string;
  cartridge_id: string | null;
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_out: string;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  // C5 fix (audit 2026-05-10): the dashboard had been displaying
  // expected_profit_usd as "Net Profit" — that's GROSS, before gas/slippage/
  // relay fees. Migration 049 added net_expected_profit_usd; route now
  // surfaces it so the UI can label honestly.
  net_expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;
  rejection_reason: string | null;
  // WO-G2-PARITY (2026-09-17): opportunities.block_number is BIGINT (migration
  // 003) and node-postgres returns int8 as a STRING at runtime — the declared
  // `number` below was a lie that shipped `"25995384"` on the wire and made the
  // frontend Zod z.number() drop the whole live feed (G2 §37 parity hole,
  // FEED-SCHEMA-01). Runtime type is string|number|null; the emitted wire value
  // is normalized to number|null by normalizeBlockNumber() below.
  block_number: number | string | null;
  status: string;
  detected_at: Date | string;
  trace_id: string;
  // WO-CARDS-COMPLETE-01 (2026-09-17, migration 121): which detector produced
  // the row (core engine name or cartridge stem). NULL on legacy rows.
  detector_id: string | null;
  // WO-CARDS-COMPLETE-01: BIGINT → node-postgres returns int8 as a STRING at
  // runtime (same parity trap as block_number, WO-G2-PARITY); normalized to
  // number|null by normalizeBlockNumber() below. NULL = not stamped at emit
  // (pre-field row or dry-run) — R8.
  pipeline_latency_ms: number | string | null;
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
  // G-SIM-1 B2b / migration 099: complete multi-hop route topology stored as
  // JSONB. Shape: { pool_addresses[], token_addresses[], dex_adapters[],
  // decimals{} }. NOT NULL DEFAULT '{}' — empty object for legacy rows or
  // detection-time failures. Surfaced so the exchange dashboard can render the
  // full A→B cycle (2..N legs) per opportunity (R8: empty {} = no topology,
  // caller falls back to dex_a/dex_b).
  route_metadata: Record<string, unknown> | null;
  // CARDS-DEDUP-HOPS (2026-09-20): route-group aggregates from the `grouped`
  // CTE — exact GROUP BY outputs (R8, never synthesised). first/last_seen_at
  // are TIMESTAMPTZ (node-postgres → Date); confirmations = COUNT(*) >= 1 by
  // construction. route_group_key is the SQL twin of the frontend's
  // routeGroupKeyOf() (frontend/lib/store/route-key.ts) — bit-for-bit
  // identical or client and server disagree on route identity (-61 condition,
  // pinned by route-key.test.ts).
  first_seen_at: Date | string;
  last_seen_at: Date | string;
  confirmations: number;
  route_group_key: string;
  // WO-H4 (2026-09-07): COUNT(*) OVER () — total rows matching the window
  // WHERE (pre-LIMIT). Same value on every row; absent when 0 rows match.
  // CARDS-DEDUP-HOPS: counts route GROUPS, not raw detections.
  window_total: number;
  // LEFT JOIN tokens ti (token_in side)
  token_in_symbol: string | null;
  token_in_decimals: number | null;
  token_in_logo_url: string | null;
  token_in_resolved_via: string | null;
  // LEFT JOIN tokens to_ (token_out side)
  token_out_symbol: string | null;
  token_out_decimals: number | null;
  token_out_logo_url: string | null;
  token_out_resolved_via: string | null;
}

// ── Query ────────────────────────────────────────────────────────────────────

// CARDS-DEDUP-HOPS (2026-09-20, operador): the exchange dashboard used to
// render one card per re-detection of the SAME route — identical economics,
// only detected_at moved. The `grouped` CTE collapses every re-detection of
// the same route identity into ONE row: MIN/MAX detected_at become
// first_seen_at/last_seen_at ("time since first detection" + "time since last
// vigency ratification") and COUNT(*) becomes `confirmations`. The wire row
// carries the LATEST detection's economics (ARRAY_AGG ... ORDER BY
// detected_at DESC)[1] — trade values refresh in place on the existing card,
// the card never duplicates per re-detection.
//
// route_group_key is the SQL twin of the frontend's routeGroupKeyOf()
// (frontend/lib/store/route-key.ts) — BIT-FOR-BIT identical or client and
// server disagree on identity (condition imposed by -61, pinned by
// route-key.test.ts). The key is DIRECTIONAL by design (debate 2026-09-20,
// -89): dex_a/dex_b are NOT normalised because token_in/token_out define the
// economics — A→B and B→A are different trades. concat_ws skips NULLs, hence
// the COALESCE-to-'' on every nullable segment.
//
// R8 fail-honest: aggregates are exact GROUP BY outputs — nothing is
// synthesised; a group of one still reports first_seen=last_seen=detected_at
// and confirmations=1 (COUNT(*) >= 1 by construction).
const LIVE_QUERY = `
WITH grouped AS (
  SELECT
    concat_ws('|',
      o.chain_id::text,
      COALESCE(o.chain_id_out::text, ''),
      COALESCE(o.strategy_kind, ''),
      o.token_in,
      o.token_out,
      o.dex_a,
      COALESCE(o.dex_b, '')
    ) AS route_group_key,
    MIN(o.detected_at) AS first_seen_at,
    MAX(o.detected_at) AS last_seen_at,
    COUNT(*)::int      AS confirmations,
    -- Latest detection per group: its economics become the card's values.
    -- id DESC tiebreaker: bursts sharing one detected_at must pick the same
    -- latest row every poll, or the card's economics flicker between polls.
    (ARRAY_AGG(o.id ORDER BY o.detected_at DESC, o.id DESC))[1] AS latest_id
  FROM opportunities o
  -- Same window + viability filter as the pre-grouping query, applied INSIDE
  -- the CTE so both the aggregates and the outer row set share one boundary.
  WHERE o.detected_at >= NOW() - ($3::int * INTERVAL '1 second')
    AND ($2::bool = false
         OR (o.status = ANY($4::text[])
             AND o.rejection_reason IS NULL))
  GROUP BY 1
)
SELECT
  g.first_seen_at,
  g.last_seen_at,
  g.confirmations,
  g.route_group_key,
  o.id,
  o.chain_id,
  o.strategy_kind,
  o.dex_a,
  o.dex_b,
  o.pair_symbol,
  o.token_in,
    ti.symbol      AS token_in_symbol,
    ti.decimals    AS token_in_decimals,
    ti.logo_url    AS token_in_logo_url,
    ti.resolved_via AS token_in_resolved_via,
  o.token_out,
    to_.symbol      AS token_out_symbol,
    to_.decimals    AS token_out_decimals,
    to_.logo_url    AS token_out_logo_url,
    to_.resolved_via AS token_out_resolved_via,
  o.amount_in_wei::text                 AS amount_in_wei,
  o.expected_profit_usd::float          AS expected_profit_usd,
  o.net_expected_profit_usd::float      AS net_expected_profit_usd,
  o.roi_pct::float                      AS roi_pct,
  o.risk_score::float                   AS risk_score,
  o.rejection_reason,
  o.block_number,
  o.status,
  o.detected_at,
  o.trace_id,
  o.cartridge_id,
  -- WO-CARDS-COMPLETE-01 (2026-09-17, migration 121): detector identity +
  -- emit-boundary latency. NULL on pre-migration rows (R8: never fabricated).
  o.detector_id,
  o.pipeline_latency_ms,
  o.chain_id_out,
  o.bridge,
  o.bridge_fee_usd::float               AS bridge_fee_usd,
  o.route_metadata                       AS route_metadata,
  -- WO-H4 (2026-09-17): real total of the live window, UNBOUNDED by LIMIT.
  -- Window functions evaluate before LIMIT, so COUNT(*) OVER () counts every
  -- row matching the WHERE (time window + viable_only filter) even when only
  -- the top-N are returned — the dashboard can show the true detection count
  -- instead of the fetch-window length. ::int because COUNT is bigint and
  -- node-postgres returns int8 as string. Tokens PK (chain_id, address)
  -- cannot fan out the LEFT JOINs, so the count equals opportunities rows.
  -- CARDS-DEDUP-HOPS: the row set is now the GROUPED set (one row per route
  -- identity), so window_total counts DISTINCT ROUTES in the window, not raw
  -- detections — the honest "how many different opportunities are live" number.
  (COUNT(*) OVER ())::int                  AS window_total
-- One row per route group: the latest detection carries the economics, the
-- group's aggregates ride alongside. The time-window + viable_only filter
-- (2026-05-10 hotfix, operator-tunable via ?max_age_seconds=N default 300s;
-- fail-honest empty response when nothing fresh) now lives INSIDE the CTE so
-- the aggregates and the row set share one boundary.
FROM grouped g
JOIN opportunities o
  ON  o.id = g.latest_id
LEFT JOIN tokens ti
  ON  ti.chain_id = o.chain_id
  AND ti.address  = LOWER(o.token_in)
LEFT JOIN tokens to_
  ON  to_.chain_id = COALESCE(o.chain_id_out, o.chain_id)
  AND to_.address  = LOWER(o.token_out)
ORDER BY
  -- PC-08 (2026-09-19, doctrina operador): ordenar por Topological Yield USD
  -- de MAYOR a MENOR cuando order=profit_usd. Default intacto detected_at DESC
  -- (compatibilidad). net_expected_profit_usd es el yield neto post-gas;
  -- COALESCE con expected para filas pre-net (R8: NULL no fabricado, ordena al final).
  CASE WHEN $5::text = 'profit_usd' THEN
    -(COALESCE(o.net_expected_profit_usd, o.expected_profit_usd))
  ELSE NULL END NULLS LAST,
  o.detected_at DESC
LIMIT $1
`.trim();

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Assembles a TokenInfoResult from prefixed columns in a query row.
 *
 * 2026-05-10 fix: the prior sentinel was `resolved_via` — assuming it was
 * a NOT NULL column. Reality: rows seeded via migrations (e.g. USDT, WETH,
 * core stables) carry symbol/decimals but `resolved_via` is NULL because
 * they never went through the enricher pipeline. The old gate dropped
 * those rows to `token_info: null` and the UI rendered "—" for the most
 * recognisable tokens on the chain.
 *
 * New sentinel: ANY of symbol/decimals/logo_url/resolved_via being
 * non-null → emit a TokenInfoResult with whatever fields we have. Only
 * when ALL FOUR are null do we return null (LEFT JOIN miss). When
 * resolved_via is null but symbol exists, surface "onchain_partial" as
 * the fallback so the frontend's "failed" branch doesn't mis-trigger.
 */
/**
 * Build the TokenInfo wire object for a single token (token_in or token_out).
 *
 * 2026-05-11 fix: previously returned null when neither the DB JOIN nor the
 * on-demand resolver had any metadata AND the address wasn't in the curated
 * registry. That left the row visually unmarked — the operator saw a
 * blank "—" cell with no warning, indistinguishable from a verified token.
 *
 * Behavior now: ALWAYS returns a TokenInfoResult. When the symbol is
 * unknown AND the address is not in the registry, surface
 * `verified=false` with `verified_notes=["address-not-in-registry"]` so
 * the frontend renders the "⚠ UNVERIFIED" badge regardless of metadata
 * completeness. The only way a token escapes the badge is being in the
 * curated registry (Uniswap default token list).
 *
 * R8 fail-honest: we still don't fabricate a symbol when we don't have one
 * — `symbol` stays null — but we DO assert the verification status
 * truthfully (unknown address = unverified).
 */
function tokenInfoFromRow(
  row: OpportunityLiveRow,
  prefix: "token_in" | "token_out",
  validation: TokenValidationRow | null,
): TokenInfoResult {
  const resolvedVia = row[`${prefix}_resolved_via`];
  const symbol      = row[`${prefix}_symbol`];
  const decimals    = row[`${prefix}_decimals`];
  const logoUrl     = row[`${prefix}_logo_url`];

  const chainId = prefix === "token_in"
    ? row.chain_id
    : (row.chain_id_out ?? row.chain_id);
  const address = prefix === "token_in" ? row.token_in : row.token_out;

  // Curated-registry verification — always runs against the bare address
  // even when DB and resolver both miss. Output is canonical: in-registry
  // tokens → verified=true; anything else → verified=false with a note.
  const reg = verifyToken(chainId, address ?? "", symbol ?? null);

  // Resolution provenance for the frontend's symbol-rendering branches:
  //   - When DB has any field         → use whatever resolved_via says (or
  //                                      "onchain_partial" for seeded rows).
  //   - When DB is empty but registry → "trustwallet_only" — we know the
  //                                      token's identity from the curated
  //                                      list even without an on-chain call.
  //   - When DB is empty and registry → "failed" — no metadata anywhere,
  //                                      shown as "—" symbol but still
  //                                      flagged UNVERIFIED.
  const allDbNull =
    resolvedVia == null && symbol == null && decimals == null && logoUrl == null;
  const resolvedViaOut: string = !allDbNull
    ? (resolvedVia ?? "onchain_partial")
    : reg.registry_symbol != null
      ? "trustwallet_only"
      : "failed";

  // ── Token Validation Engine block ──
  // The composite multi-validator output (OnChainTruth + LiquidityReality
  // + Registry) — when present, this replaces the binary
  // verified/unverified pill with a real safety score + status. Null when
  // the validation hasn't completed yet on first sighting (the worker
  // populates it within seconds, picked up on the next refresh).
  const validationBlock: TokenValidationBlock | null = validation == null
    ? null
    : {
        status:          validation.final_status,
        score:           validation.score,
        liquidity_usd:   validation.liquidity_usd,
        volume_24h_usd:  validation.volume_24h_usd,
        pair_count:      validation.pair_count,
        primary_dex:     validation.primary_dex,
        registry_source: validation.registry_source,
        validated_at:    validation.validated_at,
        reasons:         validation.score_reasons,
      };

  return {
    symbol:       symbol     ?? null,
    decimals:     decimals   ?? null,
    logo_url:     logoUrl    ?? null,
    resolved_via: resolvedViaOut,
    verified: reg.verified,
    registry_symbol: reg.registry_symbol,
    registry_name: reg.registry_name,
    verified_notes: reg.notes,
    validation: validationBlock,
  };
}

/**
 * Maps a raw PG result row to the wire-format item shape.
 * detected_at: node-postgres returns TIMESTAMPTZ as a Date object.
 * JSON.stringify auto-converts Date → ISO string, but explicit conversion
 * is clearer and avoids surprises if serialization path changes.
 */
/**
 * WO-G2-PARITY (2026-09-17): normalizes a raw PG block_number to the wire
 * contract's `number | null`. node-postgres returns BIGINT (int8) as a string,
 * which violated this route's own declared type and made the frontend Zod
 * schema reject the entire /api/opportunities/live payload (FEED-SCHEMA-01:
 * "items.0.block_number: Expected number, received string" — OpportunityTicker
 * stuck on "Opportunity feed unavailable"). Null passes through unchanged
 * (R8: block not detected ≠ block 0); values that are not safe integers
 * degrade to null rather than fabricating 0 or NaN on the wire.
 */
function normalizeBlockNumber(v: number | string | null): number | null {
  if (v == null) return null;
  const n = typeof v === "number" ? v : Number(v);
  return Number.isSafeInteger(n) ? n : null;
}

/**
 * Derives the paper-mode visibility status from rejection state. Paper mode
 * is the default operational mode (`ARBX_PAPER_TRADE=true`), so every
 * opportunity is either viable for the paper P&L or rejected by some gate.
 *
 *   no rejection reason and no terminal rejection/failure → paper_viable
 *   rejection reason OR rejected/failed lifecycle → paper_rejected
 * This is pipeline classification, not simulation or execution certification.
 *
 * The status field exists so the dashboard can filter / count without
 * re-doing the rejection_reason null-check inline. R8 fail-honest: derivation
 * is exact, not synthesised.
 */
function paperStatusFromRow(row: OpportunityLiveRow): "paper_viable" | "paper_rejected" {
  return row.rejection_reason != null || row.status === "rejected" || row.status === "failed"
    ? "paper_rejected" : "paper_viable";
}

/**
 * Derives the unique set of chain ids this opportunity touches (typically
 * one for atomic same-chain arb; two when chain_id_out is set for
 * cross-chain bridge legs). Lowercase-stable, sorted ascending.
 */
function chainsUsedFromRow(row: OpportunityLiveRow): number[] {
  const set = new Set<number>([row.chain_id]);
  if (row.chain_id_out != null && row.chain_id_out !== row.chain_id) {
    set.add(row.chain_id_out);
  }
  return Array.from(set).sort((a, b) => a - b);
}

/**
 * Derives all DEX adapter names from endpoints AND the full route topology. Empty
 * when both are blank. Lowercase-stable for case-insensitive joins.
 */
function dexesUsedFromRow(row: OpportunityLiveRow): string[] {
  const set = new Set<string>();
  if (row.dex_a) set.add(row.dex_a.toLowerCase());
  if (row.dex_b) set.add(row.dex_b.toLowerCase());
  // A 3/4/5-hop route can use intermediate venues not represented by dex_a/b.
  // Read only names actually carried by the persisted route, without guessing.
  const adapters = row.route_metadata?.dex_adapters;
  if (Array.isArray(adapters)) {
    for (const adapter of adapters) {
      if (typeof adapter === "string" && adapter.trim()) set.add(adapter.toLowerCase());
    }
  }
  return Array.from(set).sort();
}

/**
 * Per-row simulation context computed once per request.
 *
 * `forward` is the forward-simulated net at the worker's recorded amount_in;
 * `inverse` is the target-driven sizing suggestion when the operator has
 * configured a target via /strategies card or the Simulación tab. Both are
 * null when inputs are insufficient (R8: never invent).
 */
interface SimContext {
  forward: SimulationResult | null;
  inverse: InverseSizingResult | null;
  simulated_at: string;
}

function rowToOpportunity(
  row: OpportunityLiveRow,
  sim: SimContext | undefined,
  chainBaseTokenSymbol: string | null,
  validations: Map<string, TokenValidationRow>,
  legSymbols: Map<string, string>,
) {
  // Look up the per-token validation snapshots. Key format mirrors
  // tokenValidation/index.ts: `${chain_id}:${address.toLowerCase()}`.
  // Null when validation hasn't run yet (first sighting) — the
  // dashboard renders "validating…" until the async worker writes back.
  const tokenInChain = row.chain_id;
  const tokenOutChain = row.chain_id_out ?? row.chain_id;
  const validationIn = validations.get(
    `${tokenInChain}:${row.token_in.toLowerCase()}`,
  ) ?? null;
  const validationOut = validations.get(
    `${tokenOutChain}:${row.token_out.toLowerCase()}`,
  ) ?? null;
  // Observation provenance: a canonical estimate is not a forward simulation.
  // Keep gross/net/ROI in their canonical fields below. Without a computed
  // forward result every simulated field is null, never an invented zero-cost
  // breakdown or a copy of the estimate dressed as simulation evidence.
  const simulated_net_profit_usd = sim?.forward?.net_usd ?? null;
  const simulated_amount_in_usd = sim?.forward?.amount_in_usd ?? null;
  const simulated_roi_pct = sim?.forward?.roi_pct ?? null;
  const simulated_cost_breakdown: SimulatedCostBreakdown | null =
    sim?.forward?.cost_breakdown ?? null;
  const simulated_target: InverseSizingResult | null =
    sim?.inverse != null ? sim.inverse : null;
  // simulated_at is the timestamp of any sim activity (forward OR Path-B
  // inverse). Path B rows still emit a simulated_at so the dashboard can
  // age-stamp the suggestion.
  const simulated_at: string | null =
    (sim?.forward != null || sim?.inverse != null) ? sim!.simulated_at : null;
  const simulated_notes: string[] = [
    ...(sim?.forward?.notes ?? []),
    ...(sim?.inverse?.notes ?? []),
  ];

  return {
    id:                       row.id,
    chain_id:                 row.chain_id,
    // 2026-05-10 operator request: every row carries the chain's base token
    // symbol (WETH on Ethereum, USDC on Base, etc.) so the dashboard can
    // discreetly badge each opportunity with "starts/ends in WETH" without
    // the operator inferring it from the chain id.
    chain_base_token_symbol:  chainBaseTokenSymbol,
    strategy_kind:            row.strategy_kind,
    cartridge_id:             row.cartridge_id,
    dex_a:                    row.dex_a,
    dex_b:                    row.dex_b,
    pair_symbol:              row.pair_symbol,
    token_in:                 row.token_in,
    token_in_info:            tokenInfoFromRow(row, "token_in", validationIn),
    token_out:                row.token_out,
    token_out_info:           tokenInfoFromRow(row, "token_out", validationOut),
    // F2 (audit §11 RC1): symbols for intermediate route legs (multi-hop).
    // Only resolved addresses are included; an absent entry = unresolved →
    // the client falls back to its honest shortAddr (R8: no fabrication).
    leg_symbols: (() => {
      const rm = row.route_metadata;
      if (!rm || typeof rm !== "object") return null;
      const addrs = (rm as { token_addresses?: unknown }).token_addresses;
      if (!Array.isArray(addrs) || addrs.length === 0) return null;
      const out: Record<string, string> = {};
      let any = false;
      for (const a of addrs) {
        if (typeof a !== "string") continue;
        const s = legSymbols.get(tokenCacheKey(row.chain_id, a));
        if (s) {
          out[a.toLowerCase()] = s;
          any = true;
        }
      }
      return any ? out : null;
    })(),
    amount_in_wei:            row.amount_in_wei,
    // C5 fix (audit 2026-05-10): both gross and net surfaced separately so
    // the UI labels honestly. R8: both can be null (data not yet computed).
    expected_profit_usd:      row.expected_profit_usd,        // GROSS (pre-cost)
    net_expected_profit_usd:  row.net_expected_profit_usd,    // NET (gross - costs)
    roi_pct:                  row.roi_pct,
    risk_score:               row.risk_score,
    rejection_reason:         row.rejection_reason,
    // Derivations: zero added storage, single source of truth in DB.
    paper_status:             paperStatusFromRow(row),
    chains_used:              chainsUsedFromRow(row),
    dexes_used:               dexesUsedFromRow(row),
    block_number:             normalizeBlockNumber(row.block_number), // WO-G2-PARITY (2026-09-17)
    status:                   row.status,
    detected_at:              row.detected_at instanceof Date
                                ? row.detected_at.toISOString()
                                : row.detected_at,
    trace_id:                 row.trace_id,
    // CARDS-DEDUP-HOPS (2026-09-20): route-group aggregates forwarded verbatim
    // from the `grouped` CTE. first/last_seen normalized like detected_at
    // (TIMESTAMPTZ Date → ISO); confirmations is an exact int; the key is the
    // SQL twin of routeGroupKeyOf(). The frontend Zod schema/mapper treats
    // these as optional — the WS single-row path never carries them (R8).
    first_seen_at:            row.first_seen_at instanceof Date
                                ? row.first_seen_at.toISOString()
                                : row.first_seen_at,
    last_seen_at:             row.last_seen_at instanceof Date
                                ? row.last_seen_at.toISOString()
                                : row.last_seen_at,
    confirmations:            row.confirmations,
    route_group_key:          row.route_group_key,
    // WO-CARDS-COMPLETE-01 (2026-09-17): passed through verbatim (null on
    // legacy rows); latency normalized from the int8 string like block_number.
    detector_id:              row.detector_id,
    pipeline_latency_ms:      normalizeBlockNumber(row.pipeline_latency_ms),
    chain_id_out:             row.chain_id_out,
    bridge:                   row.bridge,
    bridge_fee_usd:           row.bridge_fee_usd,
    // Multi-hop route topology (migration 099). Empty object → null so the
    // frontend treats absence uniformly (R8). Passed through verbatim; never
    // fabricated.
    route_metadata:
      row.route_metadata &&
      typeof row.route_metadata === "object" &&
      Object.keys(row.route_metadata).length > 0
        ? row.route_metadata
        : null,
    // Target-driven simulation (R8 fail-honest: all nullable, source-labeled).
    // Computed only when net_expected_profit_usd is null (the canonical Rust
    // spine output wins when present).
    simulated_net_profit_usd,
    simulated_amount_in_usd,
    simulated_roi_pct,
    simulated_cost_breakdown,
    simulated_target,
    simulated_at,
    simulated_notes:           simulated_notes.length ? simulated_notes : null,
  };
}

// ── Route mount ───────────────────────────────────────────────────────────────

/**
 * Mounts GET /api/v1/opportunities/live on the given Express app.
 *
 * @param app   - Express Application instance (passed by index.ts)
 * @param pool  - pg.Pool | null (null when DATABASE_URL not configured)
 * @param redis - ioredis client | null. When non-null, the route loads each
 *                row's chain `trading_config` snapshot from
 *                `arbx:trading_config:<chain_id>` and computes a forward
 *                net-profit simulation + (optional) target-driven inverse
 *                sizing suggestion. When null, simulation is skipped and the
 *                wire shape's simulated_* fields stay null (R8 fail-honest).
 * @param log   - Structured logger (pino-compatible { warn(obj, msg?) })
 */
export function mountOpportunitiesLive(
  app: Application,
  pool: Pool | null,
  redis: Redis | null,
  log: { warn: (obj: object, msg?: string) => void },
): void {
  app.get("/api/v1/opportunities/live", async (req: Request, res: Response) => {
    if (!pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    const limit = Math.max(1, Math.min(200, Number(req.query["limit"] ?? 50)));

    // viable_only filters out rows persisted as gate rejections (rejection_reason
    // populated by spine when an opportunity is rejected before profit eval).
    // CARDS-MIRROR-01: viable_only is an opt-in filter, NOT the default. Default
    // FALSE — cards must reflect real pipeline activity (including gate
    // rejections with their honest reason visible per-item), not hide behind a
    // viable-only default that returned 0 when every recent opp was
    // gate-rejected (the operator's empty-cards symptom, 2026-08-22). The
    // interactive client still sends the flag explicitly; SSR/default callers
    // now get the full recent view (viable + rejected with reasons), mirroring
    // what paper history shows. viable_only=true remains available for callers
    // who want only non-rejected rows.
    const viableOnly =
      String(req.query["viable_only"] ?? "false").toLowerCase() === "true";

    // 2026-05-10 hotfix: bound the live window so the SSR snapshot can never
    // surface opportunities from days ago when no fresh viable rows exist.
    // User-observed regression: viable_only=true + no recent viables → ORDER BY
    // detected_at DESC LIMIT N returned the most recent VIABLE which was 4
    // days old. With the window, the response is honestly empty when nothing
    // qualifies, and the dashboard renders the empty state instead of stale
    // historical rows. Default 300s (5 min); operator can override via query
    // param. Clamped to [10s, 24h] to refuse silly values.
    const maxAgeSeconds = Math.max(
      10,
      Math.min(86_400, Number(req.query["max_age_seconds"] ?? 300)),
    );

    // PC-08 (2026-09-19): order=profit_usd sorts by Topological Yield USD
    // highest→lowest (matches the $5 CASE in LIVE_QUERY). Default detected_at.
    const order =
      String(req.query["order"] ?? "detected_at") === "profit_usd"
        ? "profit_usd"
        : "detected_at";

    try {
      const q = await pool.query<OpportunityLiveRow>(LIVE_QUERY, [
        limit,
        viableOnly,
        maxAgeSeconds,
        [...VIABLE_STATUSES],
        order,
      ]);

      // 2026-05-10 operator request: every token row must surface a symbol
      // and contract — no "—" placeholders. When the LEFT JOIN missed
      // (token row absent from `tokens` because the enricher hasn't picked
      // up this brand-new altcoin yet), fall back to an on-demand eth_call
      // resolver that fetches symbol() + decimals() from the chain in
      // parallel for every missing token in this batch, persists the result
      // back to `tokens` (so the next request hits the JOIN cache), and
      // injects the metadata into the response. Hard timeout 1.5s per RPC,
      // all-or-nothing per token — never blocks the route past a few
      // hundred ms total.
      const missing: Array<{ chain_id: number; address: string }> = [];
      for (const r of q.rows) {
        if (r.token_in_symbol == null) {
          missing.push({ chain_id: r.chain_id, address: r.token_in });
        }
        if (r.token_out_symbol == null) {
          missing.push({
            chain_id: r.chain_id_out ?? r.chain_id,
            address: r.token_out,
          });
        }
      }
      const resolved = await resolveTokensOnDemand(pool, missing);

      // Apply resolved metadata into the row before serialising.
      for (const r of q.rows) {
        if (r.token_in_symbol == null) {
          const m = resolved.get(tokenCacheKey(r.chain_id, r.token_in));
          if (m) {
            r.token_in_symbol = m.symbol;
            r.token_in_decimals = m.decimals;
            r.token_in_resolved_via = "onchain_partial";
          }
        }
        if (r.token_out_symbol == null) {
          const chainOut = r.chain_id_out ?? r.chain_id;
          const m = resolved.get(tokenCacheKey(chainOut, r.token_out));
          if (m) {
            r.token_out_symbol = m.symbol;
            r.token_out_decimals = m.decimals;
            r.token_out_resolved_via = "onchain_partial";
          }
        }
      }

      // ── F2 (audit §11 RC1): intermediate route-leg token symbols ─────────
      //
      // Multi-hop routes (triangular A→B→C→A) carry the intermediate token
      // only inside route_metadata.token_addresses — the LIVE_QUERY LEFT JOIN
      // enriches the PAIR's token_in/token_out, so intermediate legs rendered
      // as truncated addresses on the exchange cards. Hydrate the batch's
      // leg symbols here: first from the tokens table (ONE unnest join),
      // then via the on-demand eth_call resolver for the misses (which also
      // persists them back). Emitted per-item as `leg_symbols`
      // (lowercased address → symbol, resolved-only): unresolved addresses
      // are omitted so the client's honest shortAddr fallback still applies
      // (R8: no fabrication).
      const legSymbols = new Map<string, string>();
      const legMissing: Array<{ chain_id: number; address: string }> = [];
      const legSeen = new Set<string>();
      const LEG_ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
      for (const r of q.rows) {
        const rm = r.route_metadata;
        if (!rm || typeof rm !== "object") continue;
        const addrs = (rm as { token_addresses?: unknown }).token_addresses;
        if (!Array.isArray(addrs)) continue;
        for (const a of addrs) {
          if (typeof a !== "string" || !LEG_ADDR_RE.test(a)) continue;
          const key = tokenCacheKey(r.chain_id, a);
          if (legSeen.has(key)) continue;
          legSeen.add(key);
          // Pair tokens are already enriched via the LEFT JOIN above.
          if (a.toLowerCase() === r.token_in.toLowerCase()) continue;
          if (a.toLowerCase() === r.token_out.toLowerCase()) continue;
          legMissing.push({ chain_id: r.chain_id, address: a });
        }
      }
      if (legMissing.length > 0) {
        try {
          const legQ = await pool.query<{
            chain_id: number;
            address: string;
            symbol: string;
          }>(
            `SELECT t.chain_id, t.address, t.symbol
               FROM tokens t
               JOIN unnest($1::int[], $2::text[]) AS u(chain_id, address)
                 ON t.chain_id = u.chain_id AND t.address = u.address
              WHERE t.symbol IS NOT NULL`,
            [
              legMissing.map((p) => p.chain_id),
              legMissing.map((p) => p.address.toLowerCase()),
            ],
          );
          for (const lrow of legQ.rows) {
            legSymbols.set(tokenCacheKey(lrow.chain_id, lrow.address), lrow.symbol);
          }
        } catch {
          // Best-effort hydrate — the on-demand resolver below fills misses.
        }
        const stillMissing = legMissing.filter(
          (p) => !legSymbols.has(tokenCacheKey(p.chain_id, p.address)),
        );
        const legResolved = await resolveTokensOnDemand(pool, stillMissing);
        for (const [k, m] of legResolved) legSymbols.set(k, m.symbol);
      }

      // ── Target-driven simulation (per chain, per row) ──────────────────────
      //
      // For every row whose canonical Rust spine net (`net_expected_profit_usd`)
      // is null, run the TS forward simulator against the operator's per-chain
      // `trading_config` snapshot. When the row's strategy has a target
      // configured (priority: strategy_configs.min_profit_usd → simulation_tab),
      // also run the linear inverse sizer so the dashboard can render
      // "→ borrow $X to hit $Y" alongside the forward number.
      //
      // R8 fail-honest:
      //   - Redis miss / malformed JSON → snapshot null → simulation skipped.
      //   - row.expected_profit_usd null OR token can't be priced → forward null.
      //   - No target configured → inverse null but forward still rendered.
      //
      // Snapshot loading is parallel per distinct chain_id; the snapshot loader
      // has a 5s in-process cache so the typical 50-row burst on a single chain
      // hits Redis exactly once.
      const distinctChains = Array.from(
        new Set<number>(q.rows.map((r) => r.chain_id)),
      );
      const snapshotEntries = await Promise.all(
        distinctChains.map(
          async (chain_id) =>
            [chain_id, await getTradingConfigForChain(redis, chain_id)] as const,
        ),
      );
      const snapshots = new Map<number, TradingConfigSnapshot | null>(
        snapshotEntries,
      );

      const simByRowId = new Map<string, SimContext>();
      const simulatedAt = new Date().toISOString();
      for (const r of q.rows) {
        // HARDENING (2026-08-22): Remover el skip cuando net_expected_profit_usd
        // ya viene poblado. El searcher-rs calcula gross y net, pero NO calcula
        // el cost breakdown (gas, slippage, LP fees, TLS fee), ni el capital
        // amount, ni el target, ni el ROI. Esos vienen de forwardSimulate() e
        // inverseSize() que estaban siendo saltados. El SimContext siempre debe
        // correr para poblar TODOS los campos de la tarjeta.
        const snapshot = snapshots.get(r.chain_id);
        if (!snapshot) continue;
        const simRow: SimulatorRow = {
          chain_id: r.chain_id,
          strategy_kind: r.strategy_kind,
          amount_in_wei: r.amount_in_wei,
          expected_profit_usd: r.expected_profit_usd,
          token_in: r.token_in,
          token_in_symbol: r.token_in_symbol,
          token_in_decimals: r.token_in_decimals,
        };
        const forward = forwardSimulate(simRow, snapshot);
        const target = resolveTarget(snapshot, r.strategy_kind);
        // Inverse sizing has TWO paths:
        //   Path A (observed-gross): forward exists → linear extrap.
        //   Path B (roi-assumed):    forward null but target.roi_pct set →
        //                            use operator's min_roi_pct as assumed
        //                            gross_per_usd to size against USD floor.
        //                            Unblocks the dashboard when 100% of rows
        //                            arrive with expected_profit_usd=null
        //                            (workers reject before profit math).
        // When neither path is available (no forward AND no roi target),
        // inverse stays null and the dashboard renders "—".
        const inverse = target
          ? inverseSize(simRow, snapshot, target, forward)
          : null;
        // Record a SimContext when EITHER forward or inverse produced output,
        // so Path-B rows still get a target hint even without a forward block.
        if (forward || inverse) {
          simByRowId.set(r.id, { forward, inverse, simulated_at: simulatedAt });
        }
      }

      // ── Token Validation Engine batch lookup ─────────────────────────────
      //
      // For every (chain_id, address) pair the response will reference,
      // ask the validation engine for the latest composite score. Hot path
      // reads from PG (1h TTL) + in-process cache (5min TTL); first-sighted
      // tokens get a "PENDING" placeholder while the async worker validates
      // in the background — the next request picks up the real result.
      //
      // Deduplicated inside readTokenValidationsBatch: 50 rows × 2 tokens =
      // typically 5-10 unique (chain, address) pairs.
      const validationPairs: Array<{ chain_id: number; address: string }> = [];
      for (const r of q.rows) {
        validationPairs.push({ chain_id: r.chain_id, address: r.token_in });
        validationPairs.push({
          chain_id: r.chain_id_out ?? r.chain_id,
          address: r.token_out,
        });
      }
      const validations = await readTokenValidationsBatch(pool, validationPairs);

      // CARDS-PRICES-01 (2026-09-26): live PriceBus snapshot (real-time Binance
      // WS + Chainlink; Redis hash `arbx:token_prices:<chain>`, TTL ~42s) so
      // EVERY card — accepted or rejected — can render real token prices.
      // Fail-honest: unknown symbol → absent from the per-row map (R8: never a
      // fabricated price). One HGETALL per distinct chain per request.
      const priceMaps = new Map<number, Map<string, number>>();
      await Promise.all(
        [...new Set(q.rows.map((r) => r.chain_id))].map(async (cid) => {
          try {
            const raw = (await redis?.hgetall(`arbx:token_prices:${cid}`)) as unknown as
              | Record<string, string>
              | undefined;
            if (raw && Object.keys(raw).length > 0) {
              const m = new Map<string, number>();
              for (const [sym, v] of Object.entries(raw)) {
                const n = Number(v);
                // trim() guards a padded producer key (" WETH") from silently
                // missing every lookup (adversarial-review robustness note).
                if (Number.isFinite(n) && n > 0) m.set(sym.trim().toUpperCase(), n);
              }
              if (m.size > 0) priceMaps.set(cid, m);
            }
          } catch (e) {
            // fail-honest: this request ships without prices — but NEVER
            // silently: a persistent Redis failure must leave a trace.
            log.warn(
              {
                event: "opportunities.live.price_read_failed",
                chain_id: cid,
                err: (e as Error).message,
              },
              "PriceBus read failed — cards ship without live prices this request",
            );
          }
        }),
      );

      res.status(200).json({
        count:           q.rows.length,
        // WO-H4 (2026-09-07): ALL detections in the live window (COUNT over
        // the WHERE, unbounded by `limit`) so consumers can show the real
        // total instead of the fetch-window length. 0 rows → 0
        // (computed-and-exactly-zero, R8 — never null, never items.length).
        window_total:    q.rows[0]?.window_total ?? 0,
        window:          "latest",
        viable_only:     viableOnly,
        max_age_seconds: maxAgeSeconds,
        items:           q.rows.map((r) => {
                           const item = rowToOpportunity(
                             r,
                             simByRowId.get(r.id),
                             snapshots.get(r.chain_id)?.base_token_symbol ?? null,
                             validations,
                             legSymbols,
                           );
                           // CARDS-PRICES-01: attach live PriceBus prices for
                           // every token symbol on the card (endpoints + legs).
                           // Absent key = no live price (R8 fail-honest).
                           const pm = priceMaps.get(r.chain_id);
                           if (!pm) return { ...item, token_prices_usd: null };
                           const syms = new Set<string>();
                           const push = (
                             info: { symbol?: string | null; registry_symbol?: string | null } | null | undefined,
                           ): void => {
                             const s = info?.symbol ?? info?.registry_symbol;
                             if (s) syms.add(s.trim().toUpperCase());
                           };
                           push(item.token_in_info);
                           push(item.token_out_info);
                           if (item.leg_symbols) {
                             for (const s of Object.values(item.leg_symbols)) syms.add(s.toUpperCase());
                           }
                           const prices: Record<string, number> = {};
                           for (const s of syms) {
                             const p = pm.get(s);
                             if (p != null) prices[s] = p;
                           }
                           return {
                             ...item,
                             token_prices_usd: Object.keys(prices).length > 0 ? prices : null,
                           };
                         }),
        ts:              new Date().toISOString(),
      });
    } catch (e) {
      log.warn(
        { event: "opportunities.live.query_failed", err: (e as Error).message },
        "opportunities live query failed",
      );
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });
}

// Pure mapper exposed for regression inputs, never mounted as an endpoint.
export const __forTesting = { rowToOpportunity };
