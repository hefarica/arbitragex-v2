/**
 * Sprint 7 / Task 9 — /api/v1/opportunities/live route module.
 *
 * Extracted from index.ts inline handler (commit 97ffc52 baseline) and
 * enriched with LEFT JOIN tokens for nested token_in_info / token_out_info.
 *
 * Contract (unchanged from 97ffc52):
 *   - viable_only=true (default): excludes rows where rejection_reason IS NOT NULL.
 *   - viable_only=false: returns all rows in active statuses.
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
  block_number: number | null;
  status: string;
  detected_at: Date | string;
  trace_id: string;
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
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

const LIVE_QUERY = `
SELECT
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
  o.chain_id_out,
  o.bridge,
  o.bridge_fee_usd::float               AS bridge_fee_usd
FROM opportunities o
LEFT JOIN tokens ti
  ON  ti.chain_id = o.chain_id
  AND ti.address  = LOWER(o.token_in)
LEFT JOIN tokens to_
  ON  to_.chain_id = COALESCE(o.chain_id_out, o.chain_id)
  AND to_.address  = LOWER(o.token_out)
-- 2026-05-10 hotfix: bound the "live" window so the SSR snapshot does not
-- surface opportunities from days ago when no fresh viable rows exist.
-- Without this, the page rendered May-6 rows on May-10 because the query
-- only ordered by detected_at DESC LIMIT N — historical rows could fill the
-- frame. The window is operator-tunable via ?max_age_seconds=N (default
-- 300s = 5 minutes); the empty result is rendered as "No live opportunities
-- right now" by the dashboard, which is fail-honest.
--
-- viable_only=true: only show non-rejected opps in a viable lifecycle state.
-- viable_only=false: show ALL recent opps including rejected ones so the
--                    operator sees real-time pipeline activity, not just
--                    historical viable rows.
WHERE o.detected_at >= NOW() - ($3::int * INTERVAL '1 second')
  AND ($2::bool = false
       OR (o.status IN ('detected', 'validated', 'simulated', 'scored')
           AND o.rejection_reason IS NULL))
ORDER BY o.detected_at DESC
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
 * Derives the paper-mode visibility status from rejection state. Paper mode
 * is the default operational mode (`ARBX_PAPER_TRADE=true`), so every
 * opportunity is either viable for the paper P&L or rejected by some gate.
 *
 *   rejection_reason IS NULL  →  paper_viable
 *   rejection_reason !== NULL →  paper_rejected
 *
 * The status field exists so the dashboard can filter / count without
 * re-doing the rejection_reason null-check inline. R8 fail-honest: derivation
 * is exact, not synthesised.
 */
function paperStatusFromRow(row: OpportunityLiveRow): "paper_viable" | "paper_rejected" {
  return row.rejection_reason == null ? "paper_viable" : "paper_rejected";
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
 * Derives the unique set of DEX adapter names from `dex_a` + `dex_b`. Empty
 * when both are blank. Lowercase-stable for case-insensitive joins.
 */
function dexesUsedFromRow(row: OpportunityLiveRow): string[] {
  const set = new Set<string>();
  if (row.dex_a) set.add(row.dex_a.toLowerCase());
  if (row.dex_b) set.add(row.dex_b.toLowerCase());
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
  const simulated_net_profit_usd =
    sim?.forward != null ? sim.forward.net_usd : null;
  const simulated_amount_in_usd =
    sim?.forward != null ? sim.forward.amount_in_usd : null;
  const simulated_roi_pct =
    sim?.forward != null ? sim.forward.roi_pct : null;
  const simulated_cost_breakdown: SimulatedCostBreakdown | null =
    sim?.forward != null ? sim.forward.cost_breakdown : null;
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
    dex_a:                    row.dex_a,
    dex_b:                    row.dex_b,
    pair_symbol:              row.pair_symbol,
    token_in:                 row.token_in,
    token_in_info:            tokenInfoFromRow(row, "token_in", validationIn),
    token_out:                row.token_out,
    token_out_info:           tokenInfoFromRow(row, "token_out", validationOut),
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
    block_number:             row.block_number,
    status:                   row.status,
    detected_at:              row.detected_at instanceof Date
                                ? row.detected_at.toISOString()
                                : row.detected_at,
    trace_id:                 row.trace_id,
    chain_id_out:             row.chain_id_out,
    bridge:                   row.bridge,
    bridge_fee_usd:           row.bridge_fee_usd,
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
    // Default TRUE — the route contract (see header) and the regression guard
    // for commit 97ffc52 require that, with no param, only viable opportunities
    // are returned. Callers pass viable_only=false to also see rejected rows.
    // The interactive client always sends the flag explicitly; SSR/default
    // callers get the safe viable-only view.
    const viableOnly =
      String(req.query["viable_only"] ?? "true").toLowerCase() !== "false";

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

    try {
      const q = await pool.query<OpportunityLiveRow>(LIVE_QUERY, [
        limit,
        viableOnly,
        maxAgeSeconds,
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
        // Spine output always wins — never overwrite canonical net.
        if (r.net_expected_profit_usd != null) continue;
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

      res.status(200).json({
        count:           q.rows.length,
        window:          "latest",
        viable_only:     viableOnly,
        max_age_seconds: maxAgeSeconds,
        items:           q.rows.map((r) =>
                            rowToOpportunity(
                              r,
                              simByRowId.get(r.id),
                              snapshots.get(r.chain_id)?.base_token_symbol ?? null,
                              validations,
                            ),
                          ),
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
