/**
 * GET /api/metrics/paper-shadow/daily-audit  (+ /api/v1/metrics/paper-shadow/daily-audit)
 *
 * ARBX-RDY-03 — A.5 paper-shadow daily ledger audit. A.5's required action is
 * "run paper-shadow continuously and audit the daily ledger for revert rate,
 * latency, sim error rate". The accumulation data already exists in
 * `paper_trade_runs` (prod: 598,877 rows since 2026-07-05, chain 1); this
 * endpoint makes it auditable per-day, READ-ONLY (no behavior changes).
 *
 * Optional `?days=N` (default 14, clamped 1..90) bounds the daily window.
 *
 * ── Reason classification rules (explicit, derived from OBSERVED reason values)
 * ─────────────────────────────────────────────────────────────────────────────
 * `paper_trade_runs.reason` carries the opportunity's `rejection_reason`
 * VERBATIM (migration 091; written by relays-client persistence.rs and — pre
 * ARBX-R-0001 — by the Shadow Archiver). Observed families in the codebase:
 *
 *   sim-error semantics (classified):
 *     - reason ILIKE '%simulation_failed%'     → gates.rs RejectReason tags
 *       `simulation_failed` / `bundle_simulation_failed`, emitted by scanner.rs
 *       as `{tag}:{reason:?}` (e.g. "simulation_failed:SimulationFailed").
 *     - reason ILIKE 'revm_reverted%'          → sim_orchestrator.rs
 *       `reason_tag()` ("revm_reverted:{Solidity revert string}").
 *
 *   revert semantics (classified):
 *     - reason ILIKE '%revert%'                → paper mode never broadcasts, so
 *       the only revert-bearing family in the ledger is `revm_reverted:*`.
 *       NOTE: `revm_reverted:*` rows count in BOTH sim_error_runs and
 *       reverted_runs (overlapping families by design — both are true).
 *
 *   everything else with a non-NULL reason is counted honestly in
 *   `sim_error_unclassified_runs` (e.g. `gas_floor_breach`, `TokenNotAllowed:*`,
 *   `no_price_oracle`, `impact_zero`, `missing_reserves`, NULL = accepted run).
 *   The per-day `reason_histogram` ALWAYS exposes the RAW reason strings +
 *   counts (null = no rejection reason recorded) — never invented labels.
 *
 * ── two_signal (the honesty core) ─────────────────────────────────────────────
 * A.5 needs BOTH signals before its day-7 review can mean anything:
 *   1. accumulation_signal — the paper-trade run streak over paper_trade_runs
 *      (same SQL as paper-shadow-metrics.ts).
 *   2. calibration_signal — the Gate C scoring volume over scored_opportunities
 *      (migration 097). The table may legitimately be EMPTY: zeros +
 *      scored_latest_at null are reported honestly, never fabricated. The
 *      scored signal is gated by the A.8 wiring (ARBX_SCORING_ARCHIVER_MODE +
 *      scored-opportunities-archiver) — see the `note` field.
 *
 * R8 fail-honest (mirrors paper-shadow-metrics.ts):
 *   - pool null / query error → 503 (verbatim detail surfaced).
 *   - 0 rows → 200 with zeros + status "INACTIVE" (never fabricates runs).
 *   - all-NULL latency for a day → null percentiles (NOT 0 — percentile_cont
 *     ignores NULL inputs and returns NULL when no non-NULL value exists).
 *
 * Response numbers are JSON numbers (not strings) except nulls where "not
 * computed / not present" is the honest answer.
 */

import type { Request, Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

// Frontend paper-shadow surface is chain 1 (ethereum) only for now.
const CHAIN_ID = 1;
const PNL_EXPR = "COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)";

const DEFAULT_DAYS = 14;
const MIN_DAYS = 1;
const MAX_DAYS = 90;

// Two-signal note (kept as a constant so the contract is stable for consumers).
const TWO_SIGNAL_NOTE =
  "A.5 requires BOTH signals: (1) the accumulation streak of paper-trade runs " +
  "in paper_trade_runs AND (2) the Gate C scored-opportunity calibration volume " +
  "in scored_opportunities. The scored signal is gated by the A.8 wiring " +
  "(ARBX_SCORING_ARCHIVER_MODE + the scored-opportunities archiver) — zeros " +
  "mean no scored rows observed, not a pass.";

export function mountPaperShadowAudit(app: import("express").Express, deps: Deps): void {
  const handler = async (req: Request, res: Response): Promise<void> => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    // ?days=N — default 14, clamped to [1, 90]. Non-numeric/negative input
    // falls back to the default (the effective value is echoed in the response).
    const daysRaw = req.query["days"];
    const parsed = Number(Array.isArray(daysRaw) ? daysRaw[0] : daysRaw);
    const days =
      Number.isFinite(parsed) && parsed >= 1 ? Math.min(Math.floor(parsed), MAX_DAYS) : DEFAULT_DAYS;

    try {
      // (1) Per-day audit aggregates (parameterized window via make_interval).
      const daily = await deps.pool.query<{
        day: Date;
        total_runs: string;
        latency_ms_p50: string | null;
        latency_ms_p95: string | null;
        sim_error_runs: string;
        sim_error_unclassified_runs: string;
        reverted_runs: string;
        green_runs: string;
        red_runs: string;
        pnl_day_usd: string;
      }>(
        `SELECT
            date_trunc('day', created_at)                                              AS day,
            COUNT(*)::text                                                             AS total_runs,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY execution_time_ms)            AS latency_ms_p50,
            percentile_cont(0.95) WITHIN GROUP (ORDER BY execution_time_ms)            AS latency_ms_p95,
            COUNT(*) FILTER (WHERE reason ILIKE '%simulation_failed%'
                              OR reason ILIKE 'revm_reverted%')::text                   AS sim_error_runs,
            COUNT(*) FILTER (WHERE reason IS NOT NULL
                              AND reason NOT ILIKE '%simulation_failed%'
                              AND reason NOT ILIKE 'revm_reverted%')::text              AS sim_error_unclassified_runs,
            COUNT(*) FILTER (WHERE reason ILIKE '%revert%')::text                       AS reverted_runs,
            COUNT(*) FILTER (WHERE ${PNL_EXPR} > 0)::text                               AS green_runs,
            COUNT(*) FILTER (WHERE ${PNL_EXPR} <= 0)::text                              AS red_runs,
            COALESCE(SUM(${PNL_EXPR}), 0)::text                                        AS pnl_day_usd
           FROM paper_trade_runs
          WHERE chain_id = $1
            AND created_at >= NOW() - make_interval(days => $2::int)
          GROUP BY date_trunc('day', created_at)
          ORDER BY date_trunc('day', created_at) DESC`,
        [CHAIN_ID, days],
      );

      // (2) Per-day RAW reason histogram (never classified here — raw strings
      // + counts; NULL reason = accepted run, surfaced as a null reason entry).
      const reasons = await deps.pool.query<{ day: Date; reason: string | null; runs: string }>(
        `SELECT date_trunc('day', created_at) AS day,
                reason,
                COUNT(*)::text               AS runs
           FROM paper_trade_runs
          WHERE chain_id = $1
            AND created_at >= NOW() - make_interval(days => $2::int)
          GROUP BY date_trunc('day', created_at), reason`,
        [CHAIN_ID, days],
      );

      const histogramByDay = new Map<string, Array<{ reason: string | null; runs: number }>>();
      for (const row of reasons.rows) {
        const key = row.day.toISOString();
        const bucket = histogramByDay.get(key) ?? [];
        bucket.push({ reason: row.reason, runs: Number(row.runs) });
        histogramByDay.set(key, bucket);
      }

      const dailyRows = daily.rows.map((row) => {
        const totalRuns = Number(row.total_runs);
        return {
          day: row.day.toISOString(),
          total_runs: totalRuns,
          reason_histogram: histogramByDay.get(row.day.toISOString()) ?? [],
          latency_ms_p50: row.latency_ms_p50 === null ? null : Number(row.latency_ms_p50),
          latency_ms_p95: row.latency_ms_p95 === null ? null : Number(row.latency_ms_p95),
          sim_error_runs: Number(row.sim_error_runs),
          sim_error_rate: totalRuns === 0 ? 0 : Number(row.sim_error_runs) / totalRuns,
          sim_error_unclassified_runs: Number(row.sim_error_unclassified_runs),
          reverted_runs: Number(row.reverted_runs),
          reverted_rate: totalRuns === 0 ? 0 : Number(row.reverted_runs) / totalRuns,
          green_runs: Number(row.green_runs),
          red_runs: Number(row.red_runs),
          pnl_day_usd: Number(row.pnl_day_usd),
        };
      });

      // (3) Accumulation signal — totals (same shape as paper-shadow-metrics).
      const totals = await deps.pool.query<{ total_trades: string; last_trade_at: Date | null }>(
        `SELECT COUNT(*)::text AS total_trades,
                MAX(created_at) AS last_trade_at
           FROM paper_trade_runs
          WHERE chain_id = $1`,
        [CHAIN_ID],
      );

      // (4) Accumulation signal — consecutive-green-day streak over ALL days
      // (not bounded by ?days): identical SQL to paper-shadow-metrics.ts.
      const streakDays = await deps.pool.query<{ day_pnl: string }>(
        `SELECT COALESCE(SUM(${PNL_EXPR}), 0)::text AS day_pnl
           FROM paper_trade_runs
          WHERE chain_id = $1
          GROUP BY date_trunc('day', created_at)
          ORDER BY date_trunc('day', created_at) DESC`,
        [CHAIN_ID],
      );

      let consecutiveGreenDays = 0;
      for (const row of streakDays.rows) {
        if (Number(row.day_pnl) > 0) consecutiveGreenDays += 1;
        else break;
      }

      // (5) Calibration signal — Gate C scored volume (may legitimately be
      // empty: zeros + null latest are the honest answer, never fabricated).
      const scored = await deps.pool.query<{
        scored_total: string;
        scored_last_7d: string;
        scored_latest_at: Date | null;
      }>(
        `SELECT COUNT(*)::text                                                             AS scored_total,
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '7 days')::text      AS scored_last_7d,
                MAX(created_at)                                                            AS scored_latest_at
           FROM scored_opportunities`,
      );

      const t = totals.rows[0]!;
      const s = scored.rows[0]!;
      const totalTrades = Number(t.total_trades);
      const status: "ACTIVE" | "INACTIVE" = totalTrades === 0 ? "INACTIVE" : "ACTIVE";

      res.status(200).json({
        audit: {
          chain_id: CHAIN_ID,
          days,
          daily: dailyRows,
          status,
        },
        two_signal: {
          accumulation_signal: {
            consecutive_green_days: consecutiveGreenDays,
            total_trades: totalTrades,
            last_trade_at: t.last_trade_at ? t.last_trade_at.toISOString() : null,
          },
          calibration_signal: {
            scored_opportunities_total: Number(s.scored_total),
            scored_last_7d: Number(s.scored_last_7d),
            scored_latest_at: s.scored_latest_at ? s.scored_latest_at.toISOString() : null,
          },
          note: TWO_SIGNAL_NOTE,
        },
        generated_at: new Date().toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "paper_shadow_audit.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  };

  // Dual-path: the FE fetches /api/...; the edge maps /api/v1/... Both resolve here.
  app.get("/api/metrics/paper-shadow/daily-audit", handler);
  app.get("/api/v1/metrics/paper-shadow/daily-audit", handler);
}
