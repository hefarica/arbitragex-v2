/**
 * GET /api/v1/sim/pipeline — Gate-C confidence-scoring pipeline surface (A.8).
 *
 * Read-only. Serves the state of the confidence-scoring circuit
 * (`scoring_pipeline.rs` wired at `OpportunityEmitter::score_and_publish`)
 * **per STRATEGY** (STRAT-IDENT-01): each of the 264 cartridges / 5 core
 * engines is an individual strategy with its OWN declared operator combo and
 * its OWN Bayesian calibration bucket. No class-level (pair / router / family)
 * aggregation is served here.
 *
 * ## R8 Fail-Honest
 * - 503 when the PG pool is null; `null` fields when a table is missing.
 * - Counts are only what the tables hold — zero rows is reported as zero,
 *   never fabricated.
 *
 * ## Sources
 * - `scored_opportunities` (archiver consumes `arbx:scoring:scored`): per-
 *   strategy score aggregates.
 * - `bayesian_priors` (per-strategy calibration store; writer is a follow-up):
 *   calibrated-strategy count.
 */
import type { Application, Request, Response } from "express";
import type { Pool } from "pg";

interface PerStrategyRow {
  strategy_key: string | null;
  scored: number;
  accepted: number;
  avg_posterior_prob: number | null;
  avg_kelly_fraction: number | null;
  avg_recommended_usd: number | null;
  evidence_rows: number;
  last_scored_at: string | null;
  source_context: string | null;
}

export function mountSimPipeline(
  app: Application,
  deps: { pool: Pool | null; logger: { warn: (obj: object, msg?: string) => void } }
): void {
  app.get("/api/v1/sim/pipeline", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }

    // ── Per-strategy score aggregates ─────────────────────────────────
    // strategy_key IS NULL for rows scored before STRAT-IDENT-01 — they are
    // grouped separately as legacy (honest: their per-strategy identity was
    // not recorded), never back-filled.
    let per_strategy: PerStrategyRow[] | null = null;
    try {
      const q = await deps.pool.query(
        `SELECT
           strategy_key,
           COUNT(*)::int AS scored,
           COUNT(*) FILTER (WHERE bayesian_accepted = TRUE)::int AS accepted,
           ROUND(AVG(posterior_prob)::numeric, 6)::float AS avg_posterior_prob,
           ROUND(AVG(kelly_fraction)::numeric, 6)::float AS avg_kelly_fraction,
           ROUND(AVG(recommended_usd)::numeric, 2)::float AS avg_recommended_usd,
           COUNT(evidence_vector)::int AS evidence_rows,
           MAX(created_at) AS last_scored_at,
           (ARRAY_REMOVE(ARRAY_AGG(source_context ORDER BY created_at DESC), NULL))[1] AS source_context
         FROM scored_opportunities
         WHERE created_at >= NOW() - interval '7 days'
         GROUP BY strategy_key
         ORDER BY scored DESC
         LIMIT 300`,
      );
      per_strategy = q.rows.map((r: Record<string, unknown>) => ({
        strategy_key: (r["strategy_key"] as string | null) ?? null,
        scored: r["scored"] as number,
        accepted: r["accepted"] as number,
        avg_posterior_prob: (r["avg_posterior_prob"] as number | null) ?? null,
        avg_kelly_fraction: (r["avg_kelly_fraction"] as number | null) ?? null,
        avg_recommended_usd: (r["avg_recommended_usd"] as number | null) ?? null,
        evidence_rows: r["evidence_rows"] as number,
        last_scored_at: r["last_scored_at"]
          ? new Date(r["last_scored_at"] as string).toISOString()
          : null,
        source_context: (r["source_context"] as string | null) ?? null,
      }));
    } catch (e) {
      deps.logger.warn({ event: "sim_pipeline.per_strategy_query_failed", err: (e as Error).message });
      per_strategy = null;
    }

    // ── Calibration store state (per-strategy priors) ─────────────────
    let calibrated_strategies: number | null = null;
    try {
      const q = await deps.pool.query(
        `SELECT COUNT(*)::int AS n FROM bayesian_priors WHERE observation_count > 0`,
      );
      calibrated_strategies = q.rows[0]?.n ?? 0;
    } catch (e) {
      deps.logger.warn({ event: "sim_pipeline.priors_query_failed", err: (e as Error).message });
      calibrated_strategies = null;
    }

    // ── Circuit summary (only observable facts) ───────────────────────
    const identified = per_strategy?.filter((r) => r.strategy_key !== null) ?? null;
    res.json({
      generated_at: new Date().toISOString(),
      scoring_circuit: {
        // Wired at OpportunityEmitter::score_and_publish (searcher-rs
        // scoring_pipeline.rs) — advisory only, never gates emission.
        posture: "observe_only_advisory",
        calibration_identity: "strategy (STRAT-IDENT-01)",
        prior_source: "bayesian_priors (per-strategy; writer pending follow-up)",
      },
      strategy_count: identified ? identified.length : null,
      calibrated_strategies,
      per_strategy,
    });
  });
}
