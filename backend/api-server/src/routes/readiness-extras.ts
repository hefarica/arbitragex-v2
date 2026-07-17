/**
 * readiness-extras — derived views over the existing readiness report.
 *
 * Two endpoints:
 *
 *   GET /api/v1/readiness/blockers
 *     Returns a flat, redacted list of every concrete obstacle between the
 *     current state and a live flip. Combines:
 *       - Non-green items from the canonical readiness report (verifyAll).
 *       - Direct environment probes (presence only — values redacted) for
 *         runtime variables required by A.4 fork validation, paper-shadow,
 *         circuit breakers, etc.
 *       - Doctrinal phase blockers (A.4 fork real, A.5 paper-shadow, ...,
 *         A.9 GO/NO-GO formal sign-off) — these stay BLOCKED until their
 *         corresponding workspace milestone PASSES.
 *
 *   GET /api/v1/readiness/decision
 *     Derives the go/no-go verdict from the blockers list. Honest contract:
 *       - go_live is ALWAYS false in this phase (no submission code path).
 *       - go_a5 is false until A.4 fork real PASSES.
 *       - capital_exposure_usd = 0 (paper-only).
 *       - verdict = "NO_GO" whenever any critical blocker is present.
 *
 * R8 fail-honest contract:
 *   - Missing env var → blocker with status="missing", env_present=false.
 *     The raw value is NEVER returned.
 *   - Present env var → status="present", redacted_value="present" (the
 *     literal string), and the value's length to help diagnostics without
 *     leaking the secret.
 *   - Doctrine items default to status="blocked" until an operator-side
 *     event (commit, deploy, sign-off) flips them.
 *
 * Reuses verifyAll from the canonical readiness module — does NOT
 * reimplement any of the 16 underlying verifiers.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";
import type { Redis } from "ioredis";

import { verifyAll } from "../readiness/verifiers/index.js";
import type { ReadinessItem } from "../readiness/types.js";
import { resolvePaperModeState } from "../readiness/paper-mode-state.js";

// ---------------------------------------------------------------------------
// Types — wire contract for the two endpoints. Frontend Zod schemas mirror
// these exactly; treat any breaking change here as a coordinated FE+BE bump.
// ---------------------------------------------------------------------------

type BlockerSeverity = "critical" | "high" | "medium" | "low";
type BlockerStatus =
  | "missing"
  | "present"
  | "partial"
  | "blocked"
  | "pending"
  | "unknown";
type BlockerCategory =
  | "infrastructure"
  | "runtime_config"
  | "doctrinal_phase"
  | "readiness_check"
  | "risk_circuit"
  | "audit_trail"
  | "data_layer";
type BlockedPhase = "A.4" | "A.5" | "A.6" | "A.7" | "A.8" | "A.9" | "LIVE";

interface BlockerEvidence {
  env_present: boolean;
  // Set to the literal "present" when the var is set; null when missing.
  // The raw value is NEVER included. Length is included to disambiguate
  // empty strings from real values without leaking content.
  redacted_value: "present" | null;
  value_length: number | null;
  source: "env" | "readiness_report" | "doctrine";
  readiness_id?: string;
  readiness_status?: "green" | "yellow" | "red" | "pending";
}

interface Blocker {
  id: string;
  category: BlockerCategory;
  severity: BlockerSeverity;
  status: BlockerStatus;
  title: string;
  description: string;
  required_action: string;
  operator_required: boolean;
  can_auto_resolve: boolean;
  blocks: BlockedPhase[];
  evidence: BlockerEvidence;
}

interface BlockersResponse {
  generated_at: string;
  source: "runtime";
  overall_status: "blocked" | "partial" | "ready";
  blockers: Blocker[];
  summary: {
    critical: number;
    high: number;
    medium: number;
    low: number;
    blocked_phases: BlockedPhase[];
  };
}

interface DecisionResponse {
  generated_at: string;
  go_a5: boolean;
  go_live: boolean;
  verdict: "NO_GO" | "GO";
  phase: "P2_READINESS";
  capital_exposure_usd: 0;
  live_trading: false;
  private_relay: false;
  submit_enabled: false;
  paper_mode: boolean;
  reasons: string[];
  next_action: string;
  blockers_ref: "/api/v1/readiness/blockers";
  required_for_go_live: string[];
}

// ---------------------------------------------------------------------------
// Env probe — presence + length only, NEVER the value.
// ---------------------------------------------------------------------------

function probeEnv(name: string): BlockerEvidence {
  const raw = process.env[name];
  const present = typeof raw === "string" && raw.length > 0;
  return {
    env_present: present,
    redacted_value: present ? "present" : null,
    value_length: present ? raw!.length : null,
    source: "env",
  };
}

// ---------------------------------------------------------------------------
// Env-based blockers — required for A.4 fork execution + downstream phases.
// ---------------------------------------------------------------------------

function envBlockers(): Blocker[] {
  const out: Blocker[] = [];

  // A.4 fork real requires an archive-capable RPC endpoint.
  const rpc = probeEnv("RPC_HTTP_1");
  if (!rpc.env_present) {
    out.push({
      id: "rpc_http_1_missing",
      category: "infrastructure",
      severity: "critical",
      status: "missing",
      title: "RPC_HTTP_1 archive node missing",
      description:
        "A.4 fork validation cannot execute without an archive-capable Ethereum RPC URL. " +
        "REVM needs historical state at the pinned block.",
      required_action:
        "Set RPC_HTTP_1 to a Tier-1 archive RPC (Alchemy/QuickNode/Infura archive plan) in the .env on the VPS, then restart api-server.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5", "LIVE"],
      evidence: rpc,
    });
  }

  // A.4 fork real requires an ArbitrageExecutor deployment to simulate against.
  const exec = probeEnv("EXECUTOR_1");
  if (!exec.env_present) {
    out.push({
      id: "executor_1_missing",
      category: "infrastructure",
      severity: "critical",
      status: "missing",
      title: "EXECUTOR_1 ArbitrageExecutor address missing",
      description:
        "A.4 fork test needs a deployed ArbitrageExecutor contract address to forward the round-trip calldata to.",
      required_action:
        "Deploy contracts/executor/ArbitrageExecutor.sol via the existing Foundry script, then export EXECUTOR_1 in the .env on the VPS.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5", "LIVE"],
      evidence: exec,
    });
  }

  // Single-tx orchestrator gas-price input. Without it, net-of-gas profit
  // accounting is impossible (G-NET-1 doctrine).
  const gp = probeEnv("SIM_ORCHESTRATOR_GAS_PRICE_WEI");
  if (!gp.env_present) {
    out.push({
      id: "sim_orchestrator_gas_price_missing",
      category: "runtime_config",
      severity: "high",
      status: "missing",
      title: "SIM_ORCHESTRATOR_GAS_PRICE_WEI not set",
      description:
        "Simulator orchestrator needs an explicit gas-price input (or a wire to the gas oracle) to compute net-of-gas profit.",
      required_action:
        "Export SIM_ORCHESTRATOR_GAS_PRICE_WEI (operator-provided) OR wire SimulatorV2 to the gas-price oracle in env config.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5"],
      evidence: gp,
    });
  }

  // Orchestrator mode toggle: must be "single_tx" or "multistep" depending on
  // the doctrinal path. Absent = scanner cannot dispatch to v2.
  const mode = probeEnv("SIM_ORCHESTRATOR_MODE");
  if (!mode.env_present) {
    out.push({
      id: "sim_orchestrator_mode_missing",
      category: "runtime_config",
      severity: "medium",
      status: "missing",
      title: "SIM_ORCHESTRATOR_MODE not set",
      description:
        "Without SIM_ORCHESTRATOR_MODE the scanner cannot decide which orchestrator pipeline to invoke for v2 candidates.",
      required_action:
        "Set SIM_ORCHESTRATOR_MODE=single_tx (default) or =multistep on the VPS .env.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5"],
      evidence: mode,
    });
  }

  // ARBX_TRADE_MODE must remain 'paper' until A.9 formal GO/NO-GO sign-off.
  // We do NOT block on absence — paper is the safe default — but we DO block
  // on any value other than "paper".
  const trade = probeEnv("ARBX_TRADE_MODE");
  if (trade.env_present && process.env["ARBX_TRADE_MODE"] !== "paper") {
    out.push({
      id: "arbx_trade_mode_not_paper",
      category: "runtime_config",
      severity: "critical",
      status: "blocked",
      title: `ARBX_TRADE_MODE is "${process.env["ARBX_TRADE_MODE"]}" — must be "paper" pre-A.9`,
      description:
        "Doctrinal invariant: until A.9 GO/NO-GO formal sign-off, ARBX_TRADE_MODE must equal 'paper'. Any other value is a kill-switch trigger.",
      required_action:
        "Set ARBX_TRADE_MODE=paper on the VPS .env and restart searcher-rs. Do not flip to live unless the 9-gate readiness PASSES.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: trade,
    });
  }

  // DATABASE_URL — without it api-server cannot serve readiness queries at all.
  const db = probeEnv("DATABASE_URL");
  if (!db.env_present) {
    out.push({
      id: "database_url_missing",
      category: "data_layer",
      severity: "critical",
      status: "missing",
      title: "DATABASE_URL missing",
      description: "api-server cannot reach Postgres. Almost every readiness check fails.",
      required_action: "Set DATABASE_URL in the .env. Format: postgres://USER:PASS@HOST:5432/arbitragex",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5", "LIVE"],
      evidence: db,
    });
  }

  return out;
}

// ---------------------------------------------------------------------------
// Readiness-report-derived blockers — every non-green item is surfaced.
// ---------------------------------------------------------------------------

function readinessItemsToBlockers(items: ReadinessItem[]): Blocker[] {
  const out: Blocker[] = [];
  for (const it of items) {
    if (it.status === "green") continue;

    const severity: BlockerSeverity =
      it.status === "red" ? "critical" : it.status === "yellow" ? "high" : "medium";
    const status: BlockerStatus = it.status === "pending" ? "pending" : "blocked";

    out.push({
      id: `readiness_${it.id.toLowerCase().replace(/[^a-z0-9]+/g, "_")}`,
      category: "readiness_check",
      severity,
      status,
      title: it.label,
      description: it.reason,
      required_action: `Resolve readiness item ${it.id} (${it.label}).`,
      operator_required: it.status !== "pending",
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: {
        env_present: false,
        redacted_value: null,
        value_length: null,
        source: "readiness_report",
        readiness_id: it.id,
        readiness_status: it.status,
      },
    });
  }
  return out;
}

// ---------------------------------------------------------------------------
// Doctrinal phase blockers — workspace milestones not yet PASSED.
// ---------------------------------------------------------------------------

function doctrinalBlockers(): Blocker[] {
  // These are workspace-verified facts at the time of this commit. They flip
  // to "resolved" only when the corresponding milestone PASSES (which would
  // require a code change to this list — exactly what we want: a deliberate
  // re-evaluation at each milestone).
  return [
    {
      id: "a4_fork_real_not_executed",
      category: "doctrinal_phase",
      severity: "critical",
      status: "blocked",
      title: "A.4 fork validation has not executed against real RPC",
      description:
        "tests/multistep_fork.rs exists with #[ignore]; it has not been run against a real archive RPC + deployed EXECUTOR. Without this, REVM cannot be trusted on mainnet state.",
      required_action:
        "Provide RPC_HTTP_1 + EXECUTOR_1 + verify ERC20 storage layouts with `cast storage`, then run `cargo test -p searcher-rs --test multistep_fork -- --ignored --nocapture`.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.4", "A.5", "LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a5_paper_shadow_not_executed",
      category: "doctrinal_phase",
      severity: "critical",
      status: "blocked",
      title: "A.5 paper-shadow runtime has not executed",
      description:
        "Continuous paper-shadow accumulation (≥7 days, detection→sim→evidence) is the prerequisite to A.6 circuit-breakers calibration.",
      required_action:
        "After A.4 PASSES, run paper-shadow continuously and audit the daily ledger for revert rate, latency, sim error rate.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["A.5", "LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a6_circuit_breakers_partial",
      category: "risk_circuit",
      severity: "high",
      status: "partial",
      title: "A.6 circuit breakers comprehensive — partial",
      description:
        "Doctrinal CBs required: DD 10/20/30/40 tiers, max revert rate, max gas burn, max latency, max SIM_ERROR, RPC health, route/token blacklists, executor health, confidence-scoring tie-in.",
      required_action:
        "Implement comprehensive CB suite gated by SystemGuardBanner + readiness; persist trips to risk_events table; emit Prometheus alerts.",
      operator_required: false,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a7_private_relay_no_submit_pending",
      category: "doctrinal_phase",
      severity: "high",
      status: "pending",
      title: "A.7 private relay no-submit simulation not implemented",
      description:
        "Simulating the bundle path against Flashbots Protect / MEV-Blocker / Titan WITHOUT submitting is required before any go_live evaluation.",
      required_action:
        "Implement a paper-only no-submit relay client that builds + signs the bundle, validates relay acceptance shape, and discards.",
      operator_required: false,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a8_confidence_scoring_not_wired",
      category: "doctrinal_phase",
      severity: "medium",
      status: "pending",
      title: "A.8 confidence scoring (Bayesian/VPIN) not wired",
      description:
        "Primitives exist in searcher-rs (kelly_sizing, bayesian_filter); they are NOT consumed by the orchestrator yet.",
      required_action:
        "Wire bayesian_filter::adverse_selection_score + kelly_sizing::compute_position_size into the scoring path; surface via /api/v1/sim/pipeline.",
      operator_required: false,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a9_go_no_go_formal_pending",
      category: "audit_trail",
      severity: "critical",
      status: "pending",
      title: "A.9 GO/NO-GO formal sign-off pending",
      description:
        "Even when A.4–A.8 all PASS, a formal sign-off (operator + audit trail entry) is required before any flip to live. This phase has not started.",
      required_action:
        "After A.4–A.8 PASS, generate the formal GO/NO-GO ledger; require two-operator sign-off; persist to audit_logs.",
      operator_required: true,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
  ];
}

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

async function collectBlockers(deps: {
  pool: pg.Pool | null;
  redis: Redis | null;
}): Promise<{ blockers: Blocker[]; paperMode: boolean }> {
  const env = envBlockers();
  const doc = doctrinalBlockers();

  let readinessBlockers: Blocker[] = [];
  let paperMode = true;
  try {
    const report = await verifyAll({ pool: deps.pool });
    readinessBlockers = readinessItemsToBlockers(report.items);
    const authority = await resolvePaperModeState({
      redis: deps.redis,
      env: process.env,
      enabledChainIds: [1],
    });
    paperMode = authority.enabled;
  } catch {
    // verifyAll failed — DO NOT swallow; surface as a single blocker. This
    // preserves R8: we never pretend readiness was green when it failed.
    readinessBlockers = [
      {
        id: "readiness_verifyall_failed",
        category: "readiness_check",
        severity: "critical",
        status: "unknown",
        title: "verifyAll readiness check failed",
        description: "The 16-item readiness verifier threw an exception; treat as fully blocked.",
        required_action: "Inspect api-server logs for the readiness verifier stack trace.",
        operator_required: true,
        can_auto_resolve: false,
        blocks: ["LIVE"],
        evidence: { env_present: false, redacted_value: null, value_length: null, source: "readiness_report" },
      },
    ];
  }

  return { blockers: [...env, ...readinessBlockers, ...doc], paperMode };
}

function summarize(blockers: Blocker[]): BlockersResponse["summary"] {
  const counts = { critical: 0, high: 0, medium: 0, low: 0 };
  const phasesSet = new Set<BlockedPhase>();
  for (const b of blockers) {
    counts[b.severity]++;
    for (const p of b.blocks) phasesSet.add(p);
  }
  return {
    ...counts,
    blocked_phases: Array.from(phasesSet).sort(),
  };
}

function overallStatus(summary: BlockersResponse["summary"]): BlockersResponse["overall_status"] {
  if (summary.critical > 0) return "blocked";
  if (summary.high > 0 || summary.medium > 0 || summary.low > 0) return "partial";
  return "ready";
}

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

/**
 * Test-only exports. Consumed by routes/readiness-extras.test.ts to assert
 * the pure-function behaviour (env probing, doctrinal list, summarisation)
 * without spinning up an Express app.
 */
export const __forTesting = {
  probeEnv,
  envBlockers,
  doctrinalBlockers,
  readinessItemsToBlockers,
  summarize,
  overallStatus,
};

export function mountReadinessExtras(
  app: Application,
  deps: {
    pool: pg.Pool | null;
    redis: Redis | null;
    logger: { warn: (obj: object, msg?: string) => void };
  },
): void {
  app.get("/api/v1/readiness/blockers", async (_req: Request, res: Response) => {
    try {
      const { blockers } = await collectBlockers({ pool: deps.pool, redis: deps.redis });
      const summary = summarize(blockers);
      const response: BlockersResponse = {
        generated_at: new Date().toISOString(),
        source: "runtime",
        overall_status: overallStatus(summary),
        blockers,
        summary,
      };
      res.status(200).json(response);
    } catch (e) {
      deps.logger.warn({ event: "readiness_blockers.failed", err: (e as Error).message });
      res.status(503).json({ error: "blockers_collection_failed", detail: (e as Error).message });
    }
  });

  app.get("/api/v1/readiness/decision", async (_req: Request, res: Response) => {
    try {
      const { blockers, paperMode } = await collectBlockers({ pool: deps.pool, redis: deps.redis });
      const summary = summarize(blockers);

      const a4Blocker = blockers.find((b) => b.id === "a4_fork_real_not_executed");
      const goA5 = a4Blocker == null && summary.blocked_phases.indexOf("A.5") === -1;
      // go_live is structurally false in this phase. There is no submission
      // code path; the doctrine requires A.9 formal sign-off; SystemGuardBanner
      // also enforces this client-side. Three layers of NO.
      const goLive = false;

      const reasons = blockers
        .filter((b) => b.severity === "critical")
        .map((b) => b.title)
        .slice(0, 8);

      const nextActionParts: string[] = [];
      if (a4Blocker) {
        nextActionParts.push(
          "Provide RPC_HTTP_1 + EXECUTOR_1, verify ERC20 storage layouts, then run multistep_fork ignored test.",
        );
      }
      if (
        blockers.find((b) => b.id === "a5_paper_shadow_not_executed") &&
        !a4Blocker
      ) {
        nextActionParts.push("Run paper-shadow continuously for ≥7 days post A.4 PASS.");
      }
      const nextAction =
        nextActionParts.length > 0
          ? nextActionParts.join(" Then: ")
          : "All known blockers cleared at this layer; await A.9 formal GO/NO-GO sign-off.";

      const response: DecisionResponse = {
        generated_at: new Date().toISOString(),
        go_a5: goA5,
        go_live: goLive,
        verdict: summary.critical > 0 ? "NO_GO" : "NO_GO",
        // verdict stays NO_GO until go_live is unlocked by the formal A.9 sign-off
        // path (which lives outside this endpoint).
        phase: "P2_READINESS",
        capital_exposure_usd: 0,
        live_trading: false,
        private_relay: false,
        submit_enabled: false,
        paper_mode: paperMode,
        reasons,
        next_action: nextAction,
        blockers_ref: "/api/v1/readiness/blockers",
        required_for_go_live: [
          "A.4 fork validation PASS",
          "A.5 paper-shadow PASS",
          "A.6 circuit breakers comprehensive PASS",
          "A.7 private relay no-submit PASS",
          "A.8 confidence scoring wire PASS",
          "A.9 formal GO/NO-GO sign-off",
        ],
      };
      res.status(200).json(response);
    } catch (e) {
      deps.logger.warn({ event: "readiness_decision.failed", err: (e as Error).message });
      res.status(503).json({ error: "decision_collection_failed", detail: (e as Error).message });
    }
  });
}
