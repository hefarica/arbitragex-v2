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

// ── Types ────────────────────────────────────────────────────────────────────

interface TokenInfoResult {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: string | null;
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
WHERE o.status IN ('detected', 'validated', 'simulated', 'scored')
  AND ($2::bool = false OR o.rejection_reason IS NULL)
ORDER BY o.detected_at DESC
LIMIT $1
`.trim();

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Assembles a TokenInfoResult from prefixed columns in a query row.
 * Returns null when ALL token-side columns are null (no tokens row matched).
 * resolved_via will always be non-null when a row exists (NOT NULL column),
 * so its presence is used as the sentinel.
 */
function tokenInfoFromRow(
  row: OpportunityLiveRow,
  prefix: "token_in" | "token_out",
): TokenInfoResult | null {
  const resolvedVia = row[`${prefix}_resolved_via`];
  if (resolvedVia === null || resolvedVia === undefined) {
    // No tokens row joined — surface null per R8 fail-honest.
    return null;
  }
  return {
    symbol:       row[`${prefix}_symbol`]      ?? null,
    decimals:     row[`${prefix}_decimals`]    ?? null,
    logo_url:     row[`${prefix}_logo_url`]    ?? null,
    resolved_via: resolvedVia,
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

function rowToOpportunity(row: OpportunityLiveRow) {
  return {
    id:                       row.id,
    chain_id:                 row.chain_id,
    strategy_kind:            row.strategy_kind,
    dex_a:                    row.dex_a,
    dex_b:                    row.dex_b,
    pair_symbol:              row.pair_symbol,
    token_in:                 row.token_in,
    token_in_info:            tokenInfoFromRow(row, "token_in"),
    token_out:                row.token_out,
    token_out_info:           tokenInfoFromRow(row, "token_out"),
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
  };
}

// ── Route mount ───────────────────────────────────────────────────────────────

/**
 * Mounts GET /api/v1/opportunities/live on the given Express app.
 *
 * @param app  - Express Application instance (passed by index.ts)
 * @param pool - pg.Pool | null (null when DATABASE_URL not configured)
 * @param log  - Structured logger (pino-compatible { warn(obj, msg?) })
 */
export function mountOpportunitiesLive(
  app: Application,
  pool: Pool | null,
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
    // Default true so /opportunities UI no longer shows a wall of $0.00 rows
    // dominated by TokenNotAllowed et al. Rejections still surface in the
    // Pipeline Funnel widget on /operations and via direct PG query.
    const viableOnly =
      String(req.query["viable_only"] ?? "true").toLowerCase() !== "false";

    try {
      const q = await pool.query<OpportunityLiveRow>(LIVE_QUERY, [limit, viableOnly]);

      res.status(200).json({
        count:       q.rows.length,
        window:      "latest",
        viable_only: viableOnly,
        items:       q.rows.map(rowToOpportunity),
        ts:          new Date().toISOString(),
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
