/**
 * orchestrator-mode — ORCH-MODE-READONLY-01 (the C-15b read-only surface).
 *
 * GET /api/v1/readiness/orchestrator-mode
 *
 * WHY THIS EXISTS
 *   The effective `ARBX_ORCHESTRATOR_MODE` decides whether the legacy emitter
 *   that XADDs `arbx:hot:simulated` is even reachable: under `V2` the
 *   orchestrator is the sole emit path and the legacy body — the one carrying
 *   the hot-sim XADD — is skipped
 *   (`backend/searcher-rs/src/scanner.rs:2016-2019`). So flipping
 *   `ARBX_PAPER_EXECUTOR_MODE=on` with the mode in V2 runs a green loop
 *   consuming nothing.
 *
 *   That value lives in the searcher's boot census
 *   (`arbx:config:boot_census`, `backend/searcher-rs/src/runtime_knobs.rs:60`,
 *   written once per searcher boot) and has no public surface: the control
 *   board deliberately does NOT read it (§15-R10 — that guard is about the
 *   toggle gate, not about reading a value for the operator). Until now the
 *   only way to answer “is the emitter reachable?” was a trip to the VPS.
 *
 * CONTRACT (R8 — never fabricate a measurement)
 *   { computed: true,  mode: "v1"|"v2"|"shadow"|"off", published_at,
 *     age_seconds, legacy_emitter_reachable, paper_executor_mode }
 *   { computed: false, mode: null, reason: "boot_census_absent" |
 *     "boot_census_unparseable" | "orchestrator_mode_field_absent" |
 *     "redis_unavailable: …" }
 *
 *   An absent key is NOT_COMPUTED, never a guessed `v1`. The searcher's own
 *   `OrchestratorMode::from_env()` defaults to V1, but a default is not a
 *   measurement of what the running process resolved — `published_at` is what
 *   says whether the census is a fresh boot or a stale one.
 *
 * READ-ONLY: one Redis `GET`. No writes, no mutation, no toggle.
 */

import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";

/** Written by `searcher-rs` at boot (`runtime_knobs::BOOT_CENSUS_REDIS_KEY`). */
export const BOOT_CENSUS_KEY = "arbx:config:boot_census";

export type OrchestratorMode = "v1" | "v2" | "shadow" | "off";

const MODES: ReadonlySet<string> = new Set(["v1", "v2", "shadow", "off"]);

export interface OrchestratorModeView {
  generated_at: string;
  computed: boolean;
  mode: OrchestratorMode | null;
  published_at: string | null;
  age_seconds: number | null;
  /**
   * true ⇒ the legacy body that XADDs `arbx:hot:simulated` runs, so the paper
   * ledger can accumulate. Only V2 skips it (`scanner.rs:2016-2019`).
   * null ⇒ not computed.
   */
  legacy_emitter_reachable: boolean | null;
  /** The other half of the pair, from this process's env — never merged into `mode`. */
  paper_executor_mode: string;
  source: string;
  reason: string | null;
}

function notComputed(
  nowMs: number,
  paperExecutorMode: string,
  reason: string,
): OrchestratorModeView {
  return {
    generated_at: new Date(nowMs).toISOString(),
    computed: false,
    mode: null,
    published_at: null,
    age_seconds: null,
    legacy_emitter_reachable: null,
    paper_executor_mode: paperExecutorMode,
    source: `redis:${BOOT_CENSUS_KEY}`,
    reason,
  };
}

/**
 * Pure mapping boot-census JSON → view. Exported so the whole decision table is
 * fixture-testable without Redis.
 */
export function parseBootCensus(
  raw: string | null | undefined,
  nowMs: number,
  paperExecutorMode: string,
): OrchestratorModeView {
  if (raw === null || raw === undefined || raw.trim() === "") {
    return notComputed(nowMs, paperExecutorMode, "boot_census_absent");
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return notComputed(nowMs, paperExecutorMode, "boot_census_unparseable");
  }
  const obj = (parsed ?? {}) as Record<string, unknown>;
  const rawMode = obj["arbx_orchestrator_mode"];
  if (typeof rawMode !== "string" || !MODES.has(rawMode.toLowerCase())) {
    return notComputed(nowMs, paperExecutorMode, "orchestrator_mode_field_absent");
  }
  const mode = rawMode.toLowerCase() as OrchestratorMode;
  const publishedAt = typeof obj["published_at"] === "string" ? obj["published_at"] : null;
  const publishedMs = publishedAt === null ? NaN : Date.parse(publishedAt);
  const ageSeconds = Number.isFinite(publishedMs)
    ? Math.max(0, Math.round((nowMs - publishedMs) / 1000))
    : null;
  return {
    generated_at: new Date(nowMs).toISOString(),
    computed: true,
    mode,
    published_at: publishedAt,
    age_seconds: ageSeconds,
    legacy_emitter_reachable: mode !== "v2",
    paper_executor_mode: paperExecutorMode,
    source: `redis:${BOOT_CENSUS_KEY}`,
    reason: null,
  };
}

export function mountOrchestratorMode(
  app: Application,
  deps: { redis: Redis | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/readiness/orchestrator-mode", async (_req: Request, res: Response) => {
    const now = Date.now();
    const paperExecutorMode = (process.env["ARBX_PAPER_EXECUTOR_MODE"] ?? "off").toLowerCase();
    if (!deps.redis) {
      res.status(200).json(notComputed(now, paperExecutorMode, "redis_unavailable: no client"));
      return;
    }
    try {
      const raw = await deps.redis.get(BOOT_CENSUS_KEY);
      res.status(200).json(parseBootCensus(raw, now, paperExecutorMode));
    } catch (e) {
      // R8: a failed read is NOT_COMPUTED with the cause named — never a guess.
      const reason = `redis_unavailable: ${(e as Error).message.slice(0, 80)}`;
      deps.logger.warn({ event: "orchestrator_mode.read_failed", reason }, "boot census unreadable");
      res.status(200).json(notComputed(now, paperExecutorMode, reason));
    }
  });
}
