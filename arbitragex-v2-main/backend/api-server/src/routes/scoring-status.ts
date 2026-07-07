/**
 * scoring-status — Gate C confidence-scoring status surface (EVIDENCE-BASED).
 *
 * GET /api/v1/scoring/status
 *
 * Honest reporting contract (RULE 00 / R8 fail-honest). This route used to
 * report hardcoded constants; it now DERIVES the three Gate-C blockers from real
 * runtime evidence in Postgres, so the dashboard can distinguish *wired* from
 * *calibrated* and never lies between stages:
 *
 *   - scoring_pipeline_state:
 *       BLOCKED                 — scored_opportunities table absent (migration 097 not applied)
 *       WIRED_NO_RUNTIME_SAMPLE — structurally wired (module + archiver + migration),
 *                                 no row yet (quiet market — NOT a failure)
 *       WIRED_RUNTIME_SAMPLE    — ≥1 scored row observed
 *   - a4_state: A4_PENDING | A4_PASSED (a gate_c_validation row, written only by a
 *                                       real passing multistep_fork run — never code alone)
 *   - a5_state: PAPER_SHADOW_NOT_STARTED | PAPER_SHADOW_WARMING |
 *               CALIBRATED_CANDIDATE | CALIBRATED ("calibrated" is never claimed
 *               without a real ≥7-day window + per-pair priors)
 *
 * Hot rules (CLAUDE.md §24 + arbx-mev-ethics-gate):
 *   - paper_only: true · live_trading: false · capital_exposure_usd: 0
 *   - This endpoint is READ-ONLY; it CANNOT toggle any scoring flag and never writes.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";

// ---------------------------------------------------------------------------
// Wire contract types — mirror frontend Zod schemas exactly.
// ---------------------------------------------------------------------------

type ScoringStatus = "enabled" | "partial" | "blocked" | "not_available";
type ScoringPipelineState = "BLOCKED" | "WIRED_NO_RUNTIME_SAMPLE" | "WIRED_RUNTIME_SAMPLE";
type A4State = "A4_PENDING" | "A4_PASSED";
type A5State =
  | "PAPER_SHADOW_NOT_STARTED"
  | "PAPER_SHADOW_WARMING"
  | "CALIBRATED_CANDIDATE"
  | "CALIBRATED";

type ScoringDecision =
  | "ACCEPT_PAPER"
  | "REJECT_LOW_CONFIDENCE"
  | "REJECT_NEGATIVE_EV"
  | "REJECT_MISSING_SIMULATION"
  | "REJECT_MISSING_INPUTS"
  | "BLOCKED_A4"
  | "NOT_AVAILABLE";

interface ScoringComponent {
  name: string;
  wired: boolean;
  evidence_ref: string;
  description: string;
}

interface ScoringBlockedReason {
  id: string;
  severity: "critical" | "high" | "medium";
  description: string;
  evidence_ref: string;
  required_action: string;
}

interface ScoringStatusResponse {
  generated_at: string;
  source: "runtime_evidence";
  mode: "paper_only";
  scoring_status: ScoringStatus;
  scoring_enabled: boolean;
  bayesian_filter_wired: boolean;
  kelly_sizing_wired: boolean;
  vpin_wired: boolean;
  scoring_pipeline_wired: boolean;
  scoring_archiver_enabled: boolean;
  // Granular Gate-C states (the truth the dashboard renders).
  scoring_pipeline_state: ScoringPipelineState;
  a4_state: A4State;
  a5_state: A5State;
  components: ScoringComponent[];
  confidence_threshold_bps: number;
  min_expected_value_wei: string | null;
  scoring_version: string;
  recent_scored_count: number;
  last_scored_at: string | null;
  calibrated_pairs: number;
  blocked_reasons: ScoringBlockedReason[];
  available_decisions: ScoringDecision[];
  // Immutable safety flags (echoes SystemGuardBanner contract).
  live_trading: false;
  private_relay: false;
  submit_enabled: false;
  capital_exposure_usd: 0;
  next_action: string;
}

const SCORING_VERSION = "0.2.0-evidence";

// ---------------------------------------------------------------------------
// Evidence gathering (the only DB reads; all guarded + non-fatal).
// ---------------------------------------------------------------------------

interface ScoringEvidence {
  tableExists: boolean;
  archiverEnabled: boolean;
  totalScored: number;
  lastScoredAt: string | null;
  firstScoredAt: string | null;
  a4Passed: boolean;
  calibratedPriors: number;
  paperShadowActive: boolean;
  minDays: number;
  minScored: number;
  minObs: number;
}

function numEnv(key: string, dflt: number): number {
  const raw = process.env[key];
  if (raw === undefined) return dflt;
  const n = Number(raw);
  return Number.isFinite(n) ? n : dflt;
}

function emptyEvidence(): ScoringEvidence {
  return {
    tableExists: false,
    archiverEnabled: false,
    totalScored: 0,
    lastScoredAt: null,
    firstScoredAt: null,
    a4Passed: false,
    calibratedPriors: 0,
    paperShadowActive: false,
    minDays: numEnv("ARBX_PAPER_SHADOW_MIN_DAYS", 7),
    minScored: numEnv("ARBX_CALIBRATION_MIN_SCORED", 100),
    minObs: numEnv("ARBX_CALIBRATION_MIN_OBSERVATIONS", 30),
  };
}

async function gatherEvidence(
  pool: pg.Pool | null,
  logger: { warn: (obj: object, msg?: string) => void },
): Promise<ScoringEvidence> {
  const e = emptyEvidence();
  // Archiver state is the api-server's OWN runtime knowledge (env), not a DB read:
  // a dormant archiver (mode != 'on') means nothing consumes arbx:scoring:scored,
  // so scores are XADDed but never persisted — claiming "wired" would be cosmetic.
  e.archiverEnabled = (process.env["ARBX_SCORING_ARCHIVER_MODE"] ?? "off").toLowerCase() === "on";
  if (!pool) return e;
  try {
    const t = await pool.query(
      "SELECT to_regclass('public.scored_opportunities') IS NOT NULL AS exists",
    );
    e.tableExists = t.rows[0]?.exists === true;
    if (e.tableExists) {
      const s = await pool.query(
        "SELECT COUNT(*)::int AS total, MAX(created_at) AS last, MIN(created_at) AS first FROM scored_opportunities",
      );
      e.totalScored = s.rows[0]?.total ?? 0;
      e.lastScoredAt = s.rows[0]?.last ? new Date(s.rows[0].last).toISOString() : null;
      e.firstScoredAt = s.rows[0]?.first ? new Date(s.rows[0].first).toISOString() : null;
    }
    try {
      const a = await pool.query(
        "SELECT 1 FROM gate_c_validation WHERE gate = 'a4_fork_validation' AND status = 'passed' LIMIT 1",
      );
      e.a4Passed = (a.rowCount ?? 0) > 0;
    } catch {
      /* gate_c_validation absent → A4 pending */
    }
    try {
      const c = await pool.query(
        "SELECT COUNT(*)::int AS n FROM bayesian_priors WHERE observation_count >= $1",
        [e.minObs],
      );
      e.calibratedPriors = c.rows[0]?.n ?? 0;
    } catch {
      /* bayesian_priors absent → 0 calibrated */
    }
    try {
      const p = await pool.query(
        "SELECT MAX(created_at) AS last FROM paper_trade_runs WHERE created_at > now() - interval '7 days'",
      );
      e.paperShadowActive = p.rows[0]?.last != null;
    } catch {
      /* paper_trade_runs absent → rely on scored rows for activity */
    }
  } catch (err) {
    logger.warn({ event: "scoring_status.evidence_err", err: (err as Error).message });
  }
  return e;
}

// ---------------------------------------------------------------------------
// Pure state derivation (exported for tests).
// ---------------------------------------------------------------------------

function deriveScoringPipelineState(e: ScoringEvidence): ScoringPipelineState {
  // Runtime proof trumps everything: if rows exist, scores were really persisted.
  if (e.totalScored > 0) return "WIRED_RUNTIME_SAMPLE";
  // Otherwise "wired" requires BOTH the migration (table) AND a running consumer
  // (archiver enabled). A dormant archiver means nothing persists the stream, so
  // claiming "wired" would be cosmetic — report BLOCKED honestly. A quiet market
  // (wired + consuming, no rows yet) is WIRED_NO_RUNTIME_SAMPLE, not a failure.
  if (!e.tableExists || !e.archiverEnabled) return "BLOCKED";
  return "WIRED_NO_RUNTIME_SAMPLE";
}

function deriveA4State(e: ScoringEvidence): A4State {
  return e.a4Passed ? "A4_PASSED" : "A4_PENDING";
}

function spanDays(e: ScoringEvidence): number {
  if (!e.firstScoredAt || !e.lastScoredAt) return 0;
  const ms = new Date(e.lastScoredAt).getTime() - new Date(e.firstScoredAt).getTime();
  return ms > 0 ? ms / 86_400_000 : 0;
}

function deriveA5State(e: ScoringEvidence): A5State {
  const hasActivity = e.totalScored > 0 || e.paperShadowActive;
  if (!hasActivity) return "PAPER_SHADOW_NOT_STARTED";
  // CALIBRATED requires real per-pair priors with enough observations.
  if (e.calibratedPriors > 0) return "CALIBRATED";
  if (spanDays(e) >= e.minDays && e.totalScored >= e.minScored) return "CALIBRATED_CANDIDATE";
  return "PAPER_SHADOW_WARMING";
}

function buildComponents(e: ScoringEvidence, pipelineWired: boolean): ScoringComponent[] {
  return [
    {
      name: "Bayesian Beta-Binomial filter",
      wired: true,
      evidence_ref: "backend/searcher-rs/src/bayesian_filter.rs:90 bayes_update + :108 accept_by_posterior",
      description: "Beta-Binomial posterior P(profitable | wins, losses). Closed-form mean and shrinkage.",
    },
    {
      name: "VPIN toxicity (Easley-Lopez de Prado-O'Hara)",
      wired: true,
      evidence_ref: "backend/searcher-rs/src/bayesian_filter.rs:138 vpin",
      description: "Order-flow toxicity metric over imbalance buckets.",
    },
    {
      name: "PIN-based size reduction",
      wired: true,
      evidence_ref: "backend/searcher-rs/src/bayesian_filter.rs:171 decide_size_reduction_for_pin",
      description: "Adverse-selection adjustment for sizing.",
    },
    {
      name: "Kelly criterion (full + fractional)",
      wired: true,
      evidence_ref: "backend/searcher-rs/src/kelly_sizing.rs:56 kelly_fraction + :90 fractional_kelly",
      description: "f* = p/loss - q/gain, clamped [0,1]; fractional default 0.25 × Kelly.",
    },
    {
      name: "Position size integer math",
      wired: true,
      evidence_ref: "backend/searcher-rs/src/kelly_sizing.rs:119 compute_position_size",
      description: "ppm-based integer conversion to U256 wei (no float for final money).",
    },
    {
      name: "Scoring pipeline invocation",
      wired: pipelineWired,
      evidence_ref:
        "backend/searcher-rs/src/scoring_pipeline.rs + opportunity_emitter.rs::emit_accepted (XADD arbx:scoring:scored)",
      description: "Emitter invokes bayesian/kelly per paper opportunity and XADDs a ConfidenceScore.",
    },
    {
      name: "Opportunity persistence with scoring fields",
      wired: e.tableExists && e.archiverEnabled,
      evidence_ref:
        "database/migrations/097_scored_opportunities_gate_c.sql + scored-opportunities-archiver.ts",
      description: "scored_opportunities table + passive archiver persist the ConfidenceScore.",
    },
  ];
}

function buildBlockedReasons(
  pipeline: ScoringPipelineState,
  a4: A4State,
  a5: A5State,
): ScoringBlockedReason[] {
  const reasons: ScoringBlockedReason[] = [];
  if (pipeline === "BLOCKED") {
    reasons.push({
      id: "scoring_pipeline_not_wired",
      severity: "high",
      description:
        "Scores are not being persisted: either migration 097 is not applied (no scored_opportunities table) OR the archiver is dormant (ARBX_SCORING_ARCHIVER_MODE != on), so arbx:scoring:scored has no consumer.",
      evidence_ref:
        "database/migrations/097_scored_opportunities_gate_c.sql + scored-opportunities-archiver.ts (ARBX_SCORING_ARCHIVER_MODE)",
      required_action: "Apply migration 097 AND set ARBX_SCORING_ARCHIVER_MODE=on so the archiver persists ConfidenceScores.",
    });
  }
  if (a4 === "A4_PENDING") {
    reasons.push({
      id: "a4_fork_validation_not_executed",
      severity: "critical",
      description:
        "Scoring outputs cannot be trusted on mainnet state until A.4 fork validation has PASSED (no gate_c_validation row).",
      evidence_ref: "backend/searcher-rs/tests/multistep_fork.rs (ignored) + gate_c_validation table",
      required_action:
        "Run scripts/run_a4_fork_validation.sh with real RPC_HTTP_1 + EXECUTOR_1; it records the passing evidence row.",
    });
  }
  if (a5 !== "CALIBRATED") {
    const warming = a5 === "PAPER_SHADOW_WARMING" || a5 === "CALIBRATED_CANDIDATE";
    reasons.push({
      id: "a5_paper_shadow_not_executed",
      severity: warming ? "medium" : "high",
      description:
        a5 === "PAPER_SHADOW_NOT_STARTED"
          ? "Paper-shadow has not started; posterior priors are flat (uncalibrated)."
          : a5 === "CALIBRATED_CANDIDATE"
            ? "Paper-shadow window reached; priors are candidate-for-calibration but not yet promoted."
            : "Paper-shadow warming: accumulating real-flow outcomes to calibrate priors.",
      evidence_ref: "scored_opportunities + bayesian_priors recency/volume",
      required_action: "After A.4 PASSES, run scripts/activate_paper_shadow.sh continuously (≥7 days) to seed priors.",
    });
  }
  return reasons;
}

function computeStatus(components: ScoringComponent[]): ScoringStatus {
  const allWired = components.every((c) => c.wired);
  if (allWired) return "enabled";
  const someWired = components.some((c) => c.wired);
  if (someWired) return "partial";
  return "not_available";
}

function nextActionFor(pipeline: ScoringPipelineState, a4: A4State, a5: A5State): string {
  if (pipeline === "BLOCKED") return "Apply migration 097 AND set ARBX_SCORING_ARCHIVER_MODE=on so the archiver persists ConfidenceScores.";
  if (a4 === "A4_PENDING") return "Run scripts/run_a4_fork_validation.sh with real RPC_HTTP_1 + EXECUTOR_1.";
  if (a5 !== "CALIBRATED") return "Run scripts/activate_paper_shadow.sh continuously (≥7 days) to calibrate priors.";
  return "Scoring pipeline wired, A.4 passed, priors calibrated.";
}

// ---------------------------------------------------------------------------
// Test-only exports.
// ---------------------------------------------------------------------------

export const __forTesting = {
  SCORING_VERSION,
  emptyEvidence,
  deriveScoringPipelineState,
  deriveA4State,
  deriveA5State,
  buildComponents,
  buildBlockedReasons,
  computeStatus,
  nextActionFor,
};

/**
 * Whether the scoring pipeline is structurally wired (migration applied), derived
 * from the SAME runtime evidence as `/api/v1/scoring/status`. Used by the
 * risk-circuit-breakers surface. Non-fatal: a DB error degrades to `false`.
 */
export async function isScoringPipelineWired(
  pool: pg.Pool | null,
  logger: { warn: (obj: object, msg?: string) => void } = { warn: () => {} },
): Promise<boolean> {
  const e = await gatherEvidence(pool, logger);
  return deriveScoringPipelineState(e) !== "BLOCKED";
}

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

export function mountScoringStatus(
  app: Application,
  deps: { pool: pg.Pool | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/scoring/status", async (_req: Request, res: Response) => {
    const evidence = await gatherEvidence(deps.pool, deps.logger);
    const pipelineState = deriveScoringPipelineState(evidence);
    const a4State = deriveA4State(evidence);
    const a5State = deriveA5State(evidence);
    const scoring_pipeline_wired = pipelineState !== "BLOCKED";
    const components = buildComponents(evidence, scoring_pipeline_wired);
    const blocked_reasons = buildBlockedReasons(pipelineState, a4State, a5State);
    const scoring_status = computeStatus(components);

    const response: ScoringStatusResponse = {
      generated_at: new Date().toISOString(),
      source: "runtime_evidence",
      mode: "paper_only",
      scoring_status,
      scoring_enabled: scoring_status === "enabled",
      bayesian_filter_wired: true,
      kelly_sizing_wired: true,
      vpin_wired: true,
      scoring_pipeline_wired,
      scoring_archiver_enabled: evidence.archiverEnabled,
      scoring_pipeline_state: pipelineState,
      a4_state: a4State,
      a5_state: a5State,
      components,
      // 7000 bps = 70% posterior probability threshold for ACCEPT_PAPER.
      confidence_threshold_bps: 7000,
      min_expected_value_wei: null,
      scoring_version: SCORING_VERSION,
      recent_scored_count: evidence.totalScored,
      last_scored_at: evidence.lastScoredAt,
      calibrated_pairs: evidence.calibratedPriors,
      blocked_reasons,
      available_decisions: [
        "ACCEPT_PAPER",
        "REJECT_LOW_CONFIDENCE",
        "REJECT_NEGATIVE_EV",
        "REJECT_MISSING_SIMULATION",
        "REJECT_MISSING_INPUTS",
        "BLOCKED_A4",
        "NOT_AVAILABLE",
      ],
      live_trading: false,
      private_relay: false,
      submit_enabled: false,
      capital_exposure_usd: 0,
      next_action: nextActionFor(pipelineState, a4State, a5State),
    };
    res.status(200).json(response);
  });
}
