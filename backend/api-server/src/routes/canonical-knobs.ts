/**
 * GET /api/v1/config/canonical-knobs — the 42-knob canonical configuration
 * surface (XLS-CANON-01, workbook ULTRA sheet 01_CONFIG).
 *
 * Serves the EXACT snapshot published by searcher-rs at boot
 * (`CanonicalKnobs::to_json()` → Redis key `arbx:config:canonical_knobs`).
 * This route never computes or defaults anything (RULE 00): what Redis holds
 * is what the searcher booted with — precedence env `ARBX_KNOB_*` > deploy
 * yaml > workbook defaults.
 *
 * ## R8 Fail-Honest
 * - 503 `redis_unavailable` — no Redis connection.
 * - 503 `knobs_not_published` — key absent: searcher-rs has not booted/published
 *   on this Redis yet. Never a fabricated default (RULE 00).
 *
 * Mode-safety (§34): `execution_mode`/`killswitch` fields are declarative
 * observability only — the mode authority is relays-client `live_exec_policy`
 * (default-deny) and the kill-switch system, never this surface.
 */
import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";

export const CANONICAL_KNOBS_REDIS_KEY = "arbx:config:canonical_knobs";

export interface CanonicalKnobsDeps {
  redis: Redis | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountCanonicalKnobs(app: Application, deps: CanonicalKnobsDeps): void {
  app.get("/api/v1/config/canonical-knobs", async (_req: Request, res: Response) => {
    if (!deps.redis) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }
    let raw: string | null;
    try {
      raw = await deps.redis.get(CANONICAL_KNOBS_REDIS_KEY);
    } catch (e) {
      deps.logger.warn({
        event: "canonical_knobs.redis_get_failed",
        err: (e as Error).message,
      });
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }
    if (raw === null) {
      res.status(503).json({
        error: "knobs_not_published",
        detail:
          "searcher-rs publishes arbx:config:canonical_knobs at boot; key absent on this Redis",
      });
      return;
    }
    try {
      const knobs = JSON.parse(raw) as Record<string, unknown>;
      res.json({
        generated_at: new Date().toISOString(),
        source: "searcher-rs CanonicalKnobs (boot snapshot)",
        knobs,
      });
    } catch (e) {
      deps.logger.warn({
        event: "canonical_knobs.parse_failed",
        err: (e as Error).message,
      });
      res.status(503).json({ error: "knobs_snapshot_corrupted" });
    }
  });
}
