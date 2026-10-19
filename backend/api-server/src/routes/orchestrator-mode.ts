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
 * FRESHNESS IS NOT AGE (this is the point of the endpoint)
 *   The census is written at boot, so its `published_at` IS the searcher's
 *   boot instant. A six-week-old census with six weeks of uptime is
 *   `current_by_construction`, NOT stale: the mode cannot have changed without
 *   a restart. Wall-clock `age_seconds` therefore says nothing about whether
 *   the value still describes the live process — it is reported as a
 *   description, never as a freshness verdict.
 *
 *   What detects a broken census writer is the census's `published_at` against
 *   an INDEPENDENT witness of the current process start: if the searcher
 *   booted after the census was written, the writer did not rewrite it. That
 *   witness is `arbx:config:searcher_boot`, a boot marker written by
 *   `searcher-rs` in a path independent of the census assembly — precisely so
 *   the non-fatal `config.boot_census.publish_failed` case (main.rs:407-419)
 *   is detectable. Until that key exists the verdict is `not_computable` with
 *   the missing witness named: never `current_by_construction` by assumption.
 *
 * CONTRACT (R8 — never fabricate a measurement)
 *   { computed: true,  mode, boot_marker: { published_at, age_seconds },
 *     legacy_emitter_reachable, freshness: { verdict, witness,
 *     searcher_started_at, delta_seconds, reason, required_witness },
 *     paper_executor_mode, derivations }
 *   { computed: false, mode: null, freshness.verdict: "not_computable",
 *     reason: "boot_census_absent" | "boot_census_unparseable" |
 *     "orchestrator_mode_field_absent" | "redis_unavailable: …" }
 *
 *   An absent key is NOT_COMPUTED, never a guessed `v1`. The searcher's own
 *   `OrchestratorMode::from_env()` defaults to V1, but a default is not a
 *   measurement of what the running process resolved.
 *
 * READ-ONLY: two Redis `GET`s. No writes, no mutation, no toggle.
 */

import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";

/** Written by `searcher-rs` at boot (`runtime_knobs::BOOT_CENSUS_REDIS_KEY`). */
export const BOOT_CENSUS_KEY = "arbx:config:boot_census";
/**
 * NOT yet written by `searcher-rs`. The independent boot-marker witness the
 * freshness verdict needs: written in a path that runs even when the census
 * assembly fails, so `published_at` far behind it proves the census writer
 * broke. Contract: JSON `{ "started_at": <RFC3339> }`.
 */
export const SEARCHER_BOOT_KEY = "arbx:config:searcher_boot";

/** Boot writes a few seconds apart are the same boot, not a stale writer. */
export const FRESHNESS_TOLERANCE_SECONDS = 120;

/**
 * `legacy_emitter_reachable` is a RUST condition evaluated here, so it is
 * declared as derived and carries its source. If `OrchestratorMode` gains a
 * member, this constant is what has to move — the field must not lie silently.
 */
export const LEGACY_EMITTER_DERIVATION =
  "searcher-rs scanner.rs:2016 — `if orch_mode == OrchestratorMode::V2 { return Ok(()); }` skips the legacy body that carries the arbx:hot:simulated XADD; every other mode reaches it";

export type OrchestratorMode = "v1" | "v2" | "shadow" | "off";

const MODES: ReadonlySet<string> = new Set(["v1", "v2", "shadow", "off"]);

export type FreshnessVerdict = "current_by_construction" | "census_writer_stale" | "not_computable";

export interface FreshnessView {
  verdict: FreshnessVerdict;
  /** Which evidence decided it (null when nothing could). */
  witness: string | null;
  searcher_started_at: string | null;
  /** published_at − searcher_started_at, seconds. Negative ⇒ process started after. */
  delta_seconds: number | null;
  reason: string | null;
  /** What would have to exist for the verdict to be reachable. */
  required_witness: string | null;
}

export interface OrchestratorModeView {
  generated_at: string;
  computed: boolean;
  mode: OrchestratorMode | null;
  /** The boot instant (published_at) and its wall-clock age — NOT freshness. */
  boot_marker: { published_at: string | null; age_seconds: number | null };
  /**
   * true ⇒ the legacy body that XADDs `arbx:hot:simulated` runs, so the paper
   * ledger can accumulate. Only V2 skips it (`scanner.rs:2016-2019`).
   * DERIVED — see `derivations.legacy_emitter_reachable`. null ⇒ not computed.
   */
  legacy_emitter_reachable: boolean | null;
  freshness: FreshnessView;
  /** The other half of the pair, from this process's env — never merged into `mode`. */
  paper_executor_mode: string;
  source: string;
  reason: string | null;
  derivations: { legacy_emitter_reachable: string };
}

function freshness(
  verdict: FreshnessVerdict,
  fields: Partial<Omit<FreshnessView, "verdict">> = {},
): FreshnessView {
  return {
    verdict,
    witness: null,
    searcher_started_at: null,
    delta_seconds: null,
    reason: null,
    required_witness: null,
    ...fields,
  };
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
    boot_marker: { published_at: null, age_seconds: null },
    legacy_emitter_reachable: null,
    freshness: freshness("not_computable", { reason }),
    paper_executor_mode: paperExecutorMode,
    source: `redis:${BOOT_CENSUS_KEY}`,
    reason,
    derivations: { legacy_emitter_reachable: LEGACY_EMITTER_DERIVATION },
  };
}

function parseRfc3339(value: unknown): number | null {
  if (typeof value !== "string") return null;
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? ms : null;
}

/**
 * Pure mapping (boot census + boot marker) → view. Exported so the whole
 * decision table is fixture-testable without Redis.
 */
export function parseBootCensus(
  raw: string | null | undefined,
  bootMarkerRaw: string | null | undefined,
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
  const publishedMs = parseRfc3339(obj["published_at"]);
  const ageSeconds =
    publishedMs === null ? null : Math.max(0, Math.round((nowMs - publishedMs) / 1000));

  // --- freshness against the independent boot witness ---
  let fr: FreshnessView;
  let markerMs: number | null = null;
  if (bootMarkerRaw === null || bootMarkerRaw === undefined || bootMarkerRaw.trim() === "") {
    fr = freshness("not_computable", {
      reason: "searcher_boot_marker_absent",
      required_witness: `${SEARCHER_BOOT_KEY} (JSON {"started_at": RFC3339}), written by searcher-rs on a path independent of the census assembly`,
    });
  } else {
    let marker: unknown;
    try {
      marker = JSON.parse(bootMarkerRaw);
    } catch {
      marker = undefined;
    }
    markerMs = parseRfc3339((marker as Record<string, unknown> | undefined)?.["started_at"]);
    if (markerMs === null) {
      fr = freshness("not_computable", {
        reason: "searcher_boot_marker_unreadable",
        required_witness: `${SEARCHER_BOOT_KEY} (JSON {"started_at": RFC3339})`,
      });
    } else if (publishedMs === null) {
      fr = freshness("not_computable", {
        reason: "boot_census_published_at_unreadable",
        witness: SEARCHER_BOOT_KEY,
        searcher_started_at: new Date(markerMs).toISOString(),
        required_witness: null,
      });
    } else {
      const deltaSeconds = Math.round((publishedMs - markerMs) / 1000);
      const witnesses = {
        witness: SEARCHER_BOOT_KEY,
        searcher_started_at: new Date(markerMs).toISOString(),
        delta_seconds: deltaSeconds,
      };
      if (Math.abs(deltaSeconds) <= FRESHNESS_TOLERANCE_SECONDS) {
        fr = freshness("current_by_construction", witnesses);
      } else if (deltaSeconds < 0) {
        // The process started AFTER the census was written ⇒ the writer did not
        // rewrite it at this boot.
        fr = freshness("census_writer_stale", witnesses);
      } else {
        // The census is newer than the boot marker ⇒ the MARKER writer is the
        // suspect. Naming that is not the same as certifying the census.
        fr = freshness("not_computable", {
          ...witnesses,
          reason: "boot_marker_predates_census",
          required_witness: null,
        });
      }
    }
  }

  return {
    generated_at: new Date(nowMs).toISOString(),
    computed: true,
    mode,
    boot_marker: { published_at: publishedAt, age_seconds: ageSeconds },
    legacy_emitter_reachable: mode !== "v2",
    freshness: fr,
    paper_executor_mode: paperExecutorMode,
    source: `redis:${BOOT_CENSUS_KEY}`,
    reason: null,
    derivations: { legacy_emitter_reachable: LEGACY_EMITTER_DERIVATION },
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
      const [raw, markerRaw] = await Promise.all([
        deps.redis.get(BOOT_CENSUS_KEY),
        deps.redis.get(SEARCHER_BOOT_KEY),
      ]);
      res.status(200).json(parseBootCensus(raw, markerRaw, now, paperExecutorMode));
    } catch (e) {
      // R8: a failed read is NOT_COMPUTED with the cause named — never a guess.
      const reason = `redis_unavailable: ${(e as Error).message.slice(0, 80)}`;
      deps.logger.warn({ event: "orchestrator_mode.read_failed", reason }, "boot census unreadable");
      res.status(200).json(notComputed(now, paperExecutorMode, reason));
    }
  });
}
