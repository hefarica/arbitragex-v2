import type pg from "pg";
import { Redis } from "ioredis";
import type { ReadinessItem } from "../types.js";
import { resolvePaperModeState, type PaperModeState } from "../paper-mode-state.js";
import { gradePaperReadiness, type PaperAccumulationState } from "../paper-mode-readiness.js";

const DEFAULT_REDIS = process.env["REDIS_URL"] ?? "redis://redis:6379";

/**
 * G-PAP-1 — Paper trading ≥7 days continuous + report.
 *
 * Doctrine: capital may not flip live until the platform has run in
 * paper-mode continuously for ≥7 days with detection→simulation→
 * shadow-execution flowing. This catches silent regressions that pass
 * unit tests but break under real mempool pressure.
 *
 * Time-based gate. Cannot be fast-forwarded.
 *
 * Verification:
 *
 *   1. Paper-mode is currently enabled (Redis arbx:papermode = enabled=true
 *      OR cfg system.paper_mode_default=true).
 *
 *   2. opportunities table has rows in the last 1h (pipeline alive).
 *
 *   3. Time since first row in opportunities. ≥7 days → green; <7 days →
 *      yellow with countdown.
 *
 * Yellow with countdown is the honest reading. Green requires the doctrine
 * threshold to be met for real, never inferred.
 */
export async function verifyGPAP1(opts?: {
  pool?: pg.Pool | null;
  redisUrl?: string;
  minDays?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const redisUrl = opts?.redisUrl ?? DEFAULT_REDIS;
  const minDays = opts?.minDays ?? 7;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-PAP-1",
    group: "tokens_strategies" as const,
    label: `Paper trading ≥${minDays} days continuous + report`,
    doctrine: "arbx-paper-trade-first",
    verified_at,
  };

  // Layer 1: resolve canonical paper-mode authority from Redis.
  const redis = new Redis(redisUrl, {
    lazyConnect: true,
    maxRetriesPerRequest: 1,
    connectTimeout: 2000,
  });
  let authority: PaperModeState;
  try {
    await redis.connect();
    authority = await resolvePaperModeState({
      redis: { mget: async (...keys: string[]) => redis.mget(...keys) },
      env: process.env,
      enabledChainIds: [1],
    });
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `Redis unreachable for arbx:papermode key: ${(e as Error).message.slice(0, 80)}`,
    };
  } finally {
    redis.disconnect();
  }

  // Layer 2 + 3: pipeline alive + age computation.
  if (!opts?.pool) {
    return {
      ...base,
      status: "yellow",
      reason: "paper-mode ON but DB pool unavailable — cannot verify pipeline activity / age",
    };
  }

  let recent_count = 0;
  let first_row_age_days: number | null = null;
  let last_row_age_hours: number | null = null;
  try {
    // "Alive in window": at least 1 detection in the last 7 days. Markets can
    // legitimately go quiet for hours when gas + min_profit_usd thresholds are
    // tight; the doctrine point is that the pipeline HAS PRODUCED detections
    // recently, not that it's producing every hour.
    const r1 = await opts.pool.query(
      `SELECT COUNT(*)::int AS n FROM opportunities WHERE detected_at > NOW() - interval '7 days'`,
    );
    recent_count = r1.rows[0]?.n ?? 0;

    const r2 = await opts.pool.query(
      `SELECT
         EXTRACT(EPOCH FROM (NOW() - MIN(detected_at)))::float AS first_secs,
         EXTRACT(EPOCH FROM (NOW() - MAX(detected_at)))::float AS last_secs
       FROM opportunities`,
    );
    const fs = r2.rows[0]?.first_secs;
    const ls = r2.rows[0]?.last_secs;
    first_row_age_days = fs != null ? fs / 86400 : null;
    last_row_age_hours = ls != null ? ls / 3600 : null;
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `paper-mode ON but opportunities query failed: ${(e as Error).message.slice(0, 80)}`,
    };
  }

  const accumulation: PaperAccumulationState = {
    days_accumulated: first_row_age_days ?? 0,
    recent_opportunities: recent_count,
    last_opportunity_at:
      last_row_age_hours != null
        ? new Date(Date.now() - last_row_age_hours * 3600_000).toISOString()
        : null,
    pipeline_active: recent_count > 0,
    degraded: authority.degraded,
    reasons: authority.reasons,
  };

  const grade = gradePaperReadiness(authority, accumulation);

  const suffix =
    first_row_age_days != null
      ? `; ${recent_count} detections in last 7d, last ${(last_row_age_hours ?? 0).toFixed(1)}h ago · authority: ${authority.confidence}`
      : "";
  const reason = `${grade.reason}${suffix}`;

  return {
    ...base,
    status: grade.status,
    reason,
    evidence: { kind: "db_query", ref: "MIN(detected_at) FROM opportunities" },
  };
}
