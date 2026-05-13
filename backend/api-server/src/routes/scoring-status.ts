/**
 * scoring-status — A.8 confidence scoring wire status surface.
 *
 * GET /api/v1/scoring/status
 *
 * Honest reporting contract:
 *   - bayesian_filter_wired: true   — primitives compiled into searcher-rs
 *     (bayesian_filter.rs: BetaParams, bayes_update, accept_by_posterior,
 *     vpin, decide_size_reduction_for_pin)
 *   - kelly_sizing_wired:    true   — primitives compiled into searcher-rs
 *     (kelly_sizing.rs: kelly_fraction, fractional_kelly,
 *     compute_position_size, V3 liquidity helpers)
 *   - scoring_pipeline_wired: false — primitives NOT yet invoked from the
 *     scanner hot path (scanner.rs::dispatch_orchestrator_and_classify
 *     does not produce a ConfidenceScore on every candidate). This is the
 *     true blocker between "primitives ready" and "pipeline ready".
 *   - last_scored_at: null          — never (no pipeline call site yet)
 *   - recent_scored_count: 0        — same
 *
 * R8 fail-honest: every "true" is grounded in source. Every "false" surfaces
 * the specific evidence pointer (file:function) that needs to change.
 *
 * Hot rules (CLAUDE.md §24 + arbx-mev-ethics-gate):
 *   - paper_only: true
 *   - live_trading: false
 *   - capital_exposure_usd: 0
 *   - This endpoint is read-only; it CANNOT toggle any scoring flag.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";

// ---------------------------------------------------------------------------
// Wire contract types — mirror frontend Zod schemas exactly.
// ---------------------------------------------------------------------------

type ScoringStatus = "enabled" | "partial" | "blocked" | "not_available";
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
  source: "workspace_verified";
  mode: "paper_only";
  scoring_status: ScoringStatus;
  scoring_enabled: boolean;
  bayesian_filter_wired: boolean;
  kelly_sizing_wired: boolean;
  vpin_wired: boolean;
  scoring_pipeline_wired: boolean;
  components: ScoringComponent[];
  confidence_threshold_bps: number;
  min_expected_value_wei: string | null;
  scoring_version: string;
  recent_scored_count: number;
  last_scored_at: string | null;
  blocked_reasons: ScoringBlockedReason[];
  available_decisions: ScoringDecision[];
  // Immutable safety flags (echoes SystemGuardBanner contract).
  live_trading: false;
  private_relay: false;
  submit_enabled: false;
  capital_exposure_usd: 0;
  next_action: string;
}

// ---------------------------------------------------------------------------
// Workspace-verified wire status (this commit reflects the source of truth).
// Bumped only when a milestone-completing commit lands. The endpoint's value
// is honesty: a future commit that actually wires the pipeline must update
// `scoring_pipeline_wired` here AND in this file's audit comment.
// ---------------------------------------------------------------------------

const SCORING_VERSION = "0.1.0-partial";

const COMPONENTS: ScoringComponent[] = [
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
    wired: false,
    evidence_ref: "backend/searcher-rs/src/scanner.rs (dispatch_orchestrator_and_classify) — no ConfidenceScore emitted yet",
    description: "Scanner does NOT invoke bayesian/kelly per candidate. This is the integration point that must change.",
  },
  {
    name: "Opportunity persistence with scoring fields",
    wired: false,
    evidence_ref: "PG opportunities table — no confidence_score_bps, posterior_probability_bps, kelly_fraction_bps columns",
    description: "Schema migration + writer changes needed to persist scoring outputs.",
  },
];

const BLOCKED_REASONS: ScoringBlockedReason[] = [
  {
    id: "scoring_pipeline_not_wired",
    severity: "high",
    description: "Bayesian + Kelly primitives are compiled into searcher-rs but the scanner pipeline does not invoke them per opportunity. No ConfidenceScore is produced yet.",
    evidence_ref: "backend/searcher-rs/src/scanner.rs",
    required_action: "In a follow-up commit, wire bayesian_filter::accept_by_posterior + kelly_sizing::compute_position_size into dispatch_orchestrator_and_classify; emit ConfidenceScore on every paper opportunity.",
  },
  {
    id: "a4_fork_validation_not_executed",
    severity: "critical",
    description: "Even if the pipeline is wired, scoring outputs cannot be trusted on mainnet state until A.4 fork validation has PASSED against a real archive RPC + deployed EXECUTOR.",
    evidence_ref: "backend/searcher-rs/tests/multistep_fork.rs (ignored)",
    required_action: "Provide RPC_HTTP_1 + EXECUTOR_1 + verify ERC20 storage layouts; run multistep_fork ignored test.",
  },
  {
    id: "a5_paper_shadow_not_executed",
    severity: "high",
    description: "Without continuous paper-shadow accumulation, posterior priors cannot be calibrated to real-flow base rates. Scoring would operate on flat priors only.",
    evidence_ref: "no paper_shadow_writer service running",
    required_action: "After A.4 PASSES, run paper-shadow continuously (≥7 days) to seed prior weights.",
  },
];

// ---------------------------------------------------------------------------
// Test-only exports.
// ---------------------------------------------------------------------------

export const __forTesting = {
  COMPONENTS,
  BLOCKED_REASONS,
  SCORING_VERSION,
  computeStatus,
};

function computeStatus(): ScoringStatus {
  const allWired = COMPONENTS.every((c) => c.wired);
  if (allWired) return "enabled";
  const someWired = COMPONENTS.some((c) => c.wired);
  if (someWired) return "partial";
  return "not_available";
}

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

export function mountScoringStatus(
  app: Application,
  _deps: { pool: pg.Pool | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/scoring/status", async (_req: Request, res: Response) => {
    const scoringStatus = computeStatus();
    const response: ScoringStatusResponse = {
      generated_at: new Date().toISOString(),
      source: "workspace_verified",
      mode: "paper_only",
      scoring_status: scoringStatus,
      scoring_enabled: scoringStatus === "enabled",
      bayesian_filter_wired: true,
      kelly_sizing_wired: true,
      vpin_wired: true,
      // CRITICAL: this is `false` and will remain `false` until a future
      // commit actually invokes the primitives from scanner hot path. A test
      // pins this — when the wire commit lands, BOTH the constant here AND
      // the test must be updated together.
      scoring_pipeline_wired: false,
      components: COMPONENTS,
      // 7000 bps = 70% posterior probability threshold for ACCEPT_PAPER.
      // Chosen conservatively; will be calibrated by paper-shadow runs.
      confidence_threshold_bps: 7000,
      min_expected_value_wei: null,
      scoring_version: SCORING_VERSION,
      recent_scored_count: 0,
      last_scored_at: null,
      blocked_reasons: BLOCKED_REASONS,
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
      next_action: "Wire bayesian_filter + kelly_sizing into scanner::dispatch_orchestrator_and_classify; emit ConfidenceScore on every paper opportunity; persist to PG.",
    };
    res.status(200).json(response);
  });
}
