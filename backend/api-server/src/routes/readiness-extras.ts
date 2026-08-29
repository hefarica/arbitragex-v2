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
  //
  // A.4 (a4_fork_real_not_executed) was RESOLVED 2026-08-20 and removed from
  // this list: the canonical runner `scripts/run_a4_fork_validation.sh`
  // executed the ignored `multistep_fork` test against a real mainnet archive
  // RPC and recorded the pass — gate_c_validation row
  // ('a4_fork_validation','passed','a4_fork_validation_20260820T013304Z.log'),
  // dashboard `a4_state = A4_PASSED` (GET /api/v1/scoring/status). Outcome:
  // A4_OUTCOME=SIM_REVERT reason=multistep_gross_spread_non_positive (the
  // full 4-step wrapped-flash sequence ran against real mainnet state —
  // real reserves quote, real WETH bytecode balance reads, honest typed
  // rejection at the spread gate; SIM_SUCCESS stays deferred to M5 testnet).
  // Enablers: PR #431 (EIP-3607 quote caller + scoped RUSTFLAGS for the
  // revm 1.3.0 ub-checks abort + 7702-safe EXECUTOR_1 doctrine note).
  //
  // A.8 (a8_confidence_scoring_not_wired) was RESOLVED 2026-08-29 and removed
  // from this list: PR #470 (7a5967ce) wires the orchestrator's
  // `score_and_publish` to consume `scoring_pipeline::ConfidenceScore`
  // (bayesian_accepted + kelly_fraction) on EVERY paper opportunity — accepted
  // AND rejected — XADD'ing to Redis stream `arbx:scoring:scored` with
  // scored_opportunities rows carrying emission_outcome labels. Prod evidence
  // (VPS, 2026-08-29): XLEN arbx:scoring:scored = 87; last-hour rows
  // `rejected|87` (the negative class flows to the priors — the calibration
  // loop is warm); two_signal calibration_signal scored_opportunities_total=90
  // (GET /api/metrics/paper-shadow/daily-audit). Wire status surfaced at
  // /api/v1/scoring/status (edge /api/scoring/status) instead of the older
  // /api/v1/sim/pipeline path named in the original required_action.
  return [
    // A.5 RESOLVED (2026-08-29, A5-STALL closure). Root cause of the frozen
    // runtime: the 2026-08-25 deploy recreated Redis with ALL persistence
    // disabled, wiping `arbx:killswitch`; the fail-closed default
    // (configs/app.toml kill_switch_enabled_default=true) then halted
    // selector-api's consumer loop SILENTLY (4 days, lag 1781, zero logs).
    // A second defect froze the sim hop: anvil forks PIN at boot-block and
    // publicnode (the old ANVIL_FORK_URL) refuses historical state
    // ("Archive requests require a personal token") → every state fetch 403
    // ~25 min after each deploy. Fixed in practice (canonical mechanisms, no
    // display hacks): kill-switch disarmed + arbx:papermode:1 set explicit
    // via admin endpoints (audited), ANVIL_FORK_URL → alchemy (archive-
    // capable), compose Redis appendonly=yes, halt logs in both consumers,
    // new gate G-PIPE-1 (consumer-group lag + kill-switch state). Prod
    // evidence (VPS, 2026-08-29 16:30Z): selector-g0 consuming again,
    // arbx:opps:validated flowing (2k+ entries in minutes), sim-ctl
    // draining validated at lag 0 with state fetches succeeding (the
    // `failed to get account` class collapsed 511→3), G-PAP-1 green
    // ("explicit and accumulation sufficient", MIN(detected_at)=2026-05-03,
    // 343k detections/7d). Accumulation: 51 days of trade-run ledger
    // (598,877 rows, 33-day green streak through 08-25) + continuous
    // detection→sim→evidence since (simulations ~300-600/h, scored rows
    // flowing). KNOWN FOLLOW-UP (honest, not hidden): 0 of 639,955 sims
    // have EVER passed — S4 probes need token_in the placeholder sim signer
    // does not hold (TRANSFER_FROM_FAILED) and 97% of strategy kinds are
    // not simulatable in S4 — so NEW accepted paper trades need the S4
    // sim-pass work (consumer-level backend sprint). The daily-audit
    // two_signal (RDY-03) remains live to observe it.
    {
      id: "a6_circuit_breakers_partial",
      category: "risk_circuit",
      severity: "high",
      status: "partial",
      title: "A.6 circuit breakers comprehensive — partial (Prometheus emission pending)",
      description:
        "SHIPPED (PR #470, 7a5967ce): the 10 doctrinal CBs (DD 10/20/30/40 tiers, max revert rate, max gas burn, max latency, max SIM_ERROR, RPC health, route/token blacklists, executor health, confidence-scoring tie-in) compute in /api/v1/risk/circuit-breakers/status; DD tiers read the REAL paper ledger (prod verified 2026-08-29); kill_switch is live runtime state; unset ARBX_CB_* thresholds surface honestly as NOT_AVAILABLE. REMAINING: Prometheus alert emission from breaker state.",
      required_action:
        "Emit Prometheus alerts for breaker state (alerts.rules.yml currently has no CB rule); operator may set ARBX_CB_* env thresholds on the VPS to move revert/gas breakers off NOT_AVAILABLE.",
      operator_required: false,
      can_auto_resolve: false,
      blocks: ["LIVE"],
      evidence: { env_present: false, redacted_value: null, value_length: null, source: "doctrine" },
    },
    {
      id: "a7_private_relay_no_submit_partial",
      category: "doctrinal_phase",
      severity: "high",
      status: "partial",
      title: "A.7 private relay no-submit simulation — module shipped, runtime call-site pending",
      description:
        "SHIPPED (PR #470, 7a5967ce): relays-client `relay_no_submit_sim` builds + eth-signs the bundle locally, validates the acceptance shape against the 3 relay schemas (Flashbots Protect / MEV-Blocker / Titan wire shapes), and discards — zero network egress by construction (no HTTP client import on the path). REMAINING: the execution loop does not invoke it yet (no runtime call-site).",
      required_action:
        "Wire validate_and_discard into the paper execution terminus so each simulated bundle runs the no-submit validation and logs relay_sim.no_submit.* events.",
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
        "Even when every other phase blocker clears (A.4 resolved 2026-08-20, A.8 resolved 2026-08-29, A.5 resolved 2026-08-29; A.6/A.7 partials open), a formal sign-off (operator + audit trail entry) is required before any flip to live. This phase has not started.",
      required_action:
        "Clear the A.6/A.7 partials, then generate the formal GO/NO-GO ledger; require two-operator sign-off; persist to audit_logs.",
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

// R6: in-process cache for collectBlockers. readiness/decision is polled every
// ~15-30s by the dashboard + SSR, and each call re-runs all 16 verifiers
// (several hit Postgres). Under polling load the PG pool saturates → verifiers
// hit the 5s connectionTimeoutMillis ("timeout exceeded trying to connect") →
// readiness/decision takes 5s+ → SSR/edge 5s timeout → 503s + decision=null
// (the C-02 intermittent). Readiness state changes slowly; a 20s TTL cuts the
// PG demand from readiness ~10-20x. In-flight dedup prevents a thundering herd
// of concurrent cold computes at cache expiry.
const BLOCKERS_CACHE_TTL_MS = 20_000;
let blockersCache: { result: { blockers: Blocker[]; paperMode: boolean }; ts: number } | null = null;
let blockersInFlight: Promise<{ blockers: Blocker[]; paperMode: boolean }> | null = null;

async function getCachedBlockers(deps: {
  pool: pg.Pool | null;
  redis: Redis | null;
}): Promise<{ blockers: Blocker[]; paperMode: boolean }> {
  const now = Date.now();
  if (blockersCache && now - blockersCache.ts < BLOCKERS_CACHE_TTL_MS) {
    return blockersCache.result;
  }
  if (blockersInFlight) {
    return blockersInFlight;
  }
  blockersInFlight = (async () => {
    try {
      const result = await collectBlockers(deps);
      blockersCache = { result, ts: Date.now() };
      return result;
    } finally {
      blockersInFlight = null;
    }
  })();
  return blockersInFlight;
}

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
      const { blockers } = await getCachedBlockers({ pool: deps.pool, redis: deps.redis });
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
      const { blockers, paperMode } = await getCachedBlockers({ pool: deps.pool, redis: deps.redis });
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
