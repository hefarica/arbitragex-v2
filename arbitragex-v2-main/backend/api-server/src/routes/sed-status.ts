import type { Application, Request, Response } from "express";
import type { Pool } from "pg";

/**
 * SED Convergence Status — Read-only telemetry endpoint.
 *
 * Queries the four SED pipeline tables (migration 062) and returns
 * structured JSON for dashboard observability.
 *
 * ## R8 Fail-Honest
 * - Returns 503 if PG pool is null
 * - Returns `null` for any metric that can't be computed
 * - Never fabricates filtration/eigenstate/allocation/hedge data
 *
 * ## Tables queried:
 * - sed_filtrations
 * - sed_eigenstates
 * - sed_allocations
 * - sed_hedges
 */
export function mountSedStatus(
  app: Application,
  deps: { pool: Pool | null; logger: { warn: (obj: object, msg?: string) => void } }
): void {
  app.get("/api/v1/sed/status", async (req: Request, res: Response) => {
    if (!deps.pool) {
      return res.status(503).json({ error: "db_unavailable" });
    }

    const chainId = Number(req.query["chain_id"] ?? 1);
    const windowMinutes = Number(req.query["window_minutes"] ?? 60);

    if (!Number.isFinite(chainId) || chainId < 1) {
      return res.status(400).json({ error: "invalid_chain_id" });
    }
    if (!Number.isFinite(windowMinutes) || windowMinutes < 1 || windowMinutes > 1440) {
      return res.status(400).json({ error: "invalid_window_minutes", detail: "must be 1-1440" });
    }

    const source = {
      postgres: "ok" as string,
    };

    try {
      // ── 1. Filtrations (Phase 2 — CDC) ──────────────────────────────
      let filtrations: {
        total: number;
        inefficiencies: number;
        latest_cdc: number | null;
        latest_confidence: number | null;
        latest_market: string | null;
        latest_at: string | null;
        avg_execution_ms: number | null;
      } | null = null;

      try {
        const fq = await deps.pool.query(
          `SELECT
             COUNT(*)::int AS total,
             COUNT(*) FILTER (WHERE cdc_is_inefficiency = TRUE)::int AS inefficiencies,
             (SELECT cdc_value FROM sed_filtrations WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_cdc,
             (SELECT cdc_confidence FROM sed_filtrations WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_confidence,
             (SELECT market_id FROM sed_filtrations WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_market,
             (SELECT created_at FROM sed_filtrations WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_at,
             AVG(execution_time_ms)::float AS avg_execution_ms
           FROM sed_filtrations
           WHERE chain_id = $1
             AND created_at >= NOW() - ($2::int * INTERVAL '1 minute')`,
          [chainId, windowMinutes]
        );

        const row = fq.rows[0];
        filtrations = {
          total: row?.total ?? 0,
          inefficiencies: row?.inefficiencies ?? 0,
          latest_cdc: row?.latest_cdc ?? null,
          latest_confidence: row?.latest_confidence ?? null,
          latest_market: row?.latest_market ?? null,
          latest_at: row?.latest_at ? new Date(row.latest_at).toISOString() : null,
          avg_execution_ms: row?.avg_execution_ms ?? null,
        };
      } catch (e) {
        deps.logger.warn({ event: "sed_status.filtrations_query_failed", err: (e as Error).message });
        // Table may not exist yet — R8: report null, not fabricate
        filtrations = null;
      }

      // ── 2. Eigenstates (Phase 3 — Lanczos) ─────────────────────────
      let eigenstates: {
        total: number;
        equilibrium_states: number;
        latest_energy: number | null;
        latest_spectral_gap: number | null;
        latest_at: string | null;
      } | null = null;

      try {
        const eq = await deps.pool.query(
          `SELECT
             COUNT(*)::int AS total,
             COUNT(*) FILTER (WHERE is_equilibrium_state = TRUE)::int AS equilibrium_states,
             (SELECT eigenstate_energy FROM sed_eigenstates WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_energy,
             (SELECT spectral_gap FROM sed_eigenstates WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_spectral_gap,
             (SELECT created_at FROM sed_eigenstates WHERE chain_id = $1 ORDER BY created_at DESC LIMIT 1) AS latest_at
           FROM sed_eigenstates
           WHERE chain_id = $1
             AND created_at >= NOW() - ($2::int * INTERVAL '1 minute')`,
          [chainId, windowMinutes]
        );

        const row = eq.rows[0];
        eigenstates = {
          total: row?.total ?? 0,
          equilibrium_states: row?.equilibrium_states ?? 0,
          latest_energy: row?.latest_energy ?? null,
          latest_spectral_gap: row?.latest_spectral_gap ?? null,
          latest_at: row?.latest_at ? new Date(row.latest_at).toISOString() : null,
        };
      } catch (e) {
        deps.logger.warn({ event: "sed_status.eigenstates_query_failed", err: (e as Error).message });
        eigenstates = null;
      }

      // ── 3. Allocations (Phase 5 — Pontryagin) ──────────────────────
      let allocations: {
        total: number;
        constraint_satisfied: number;
        avg_dirac_amplitude: number | null;
        latest_at: string | null;
      } | null = null;

      try {
        const aq = await deps.pool.query(
          `SELECT
             COUNT(*)::int AS total,
             COUNT(*) FILTER (WHERE hyperbolic_constraint_satisfied = TRUE)::int AS constraint_satisfied,
             AVG(dirac_amplitude)::float AS avg_dirac_amplitude,
             MAX(created_at) AS latest_at
           FROM sed_allocations
           WHERE chain_id = $1
             AND created_at >= NOW() - ($2::int * INTERVAL '1 minute')`,
          [chainId, windowMinutes]
        );

        const row = aq.rows[0];
        allocations = {
          total: row?.total ?? 0,
          constraint_satisfied: row?.constraint_satisfied ?? 0,
          avg_dirac_amplitude: row?.avg_dirac_amplitude ?? null,
          latest_at: row?.latest_at ? new Date(row.latest_at).toISOString() : null,
        };
      } catch (e) {
        deps.logger.warn({ event: "sed_status.allocations_query_failed", err: (e as Error).message });
        allocations = null;
      }

      // ── 4. Hedges (Phase 6 — OrthogonalVariance) ───────────────────
      let hedges: {
        total: number;
        fully_neutralized: number;
        avg_orthogonality_error: number | null;
        latest_at: string | null;
      } | null = null;

      try {
        const hq = await deps.pool.query(
          `SELECT
             COUNT(*)::int AS total,
             COUNT(*) FILTER (WHERE is_fully_neutralized = TRUE)::int AS fully_neutralized,
             AVG(orthogonality_error)::float AS avg_orthogonality_error,
             MAX(created_at) AS latest_at
           FROM sed_hedges
           WHERE chain_id = $1
             AND created_at >= NOW() - ($2::int * INTERVAL '1 minute')`,
          [chainId, windowMinutes]
        );

        const row = hq.rows[0];
        hedges = {
          total: row?.total ?? 0,
          fully_neutralized: row?.fully_neutralized ?? 0,
          avg_orthogonality_error: row?.avg_orthogonality_error ?? null,
          latest_at: row?.latest_at ? new Date(row.latest_at).toISOString() : null,
        };
      } catch (e) {
        deps.logger.warn({ event: "sed_status.hedges_query_failed", err: (e as Error).message });
        hedges = null;
      }

      // ── Response ────────────────────────────────────────────────────
      res.status(200).json({
        chain_id: chainId,
        window_minutes: windowMinutes,
        source,
        pipeline: {
          filtrations,
          eigenstates,
          allocations,
          hedges,
        },
        pipeline_operational:
          filtrations !== null &&
          eigenstates !== null &&
          allocations !== null &&
          hedges !== null,
        ts: new Date().toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "sed_status.unhandled_error", err: (e as Error).message });
      source.postgres = "unavailable";
      res.status(500).json({ error: "internal_error", detail: (e as Error).message, source });
    }
  });
}
