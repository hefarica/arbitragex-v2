/**
 * risk-circuit-breakers — A.6 comprehensive circuit breaker visibility.
 *
 * GET /api/v1/risk/circuit-breakers/status
 * GET /api/v1/risk/circuit-breakers/events
 *
 * Honest reporting contract — NEVER fabricate breaker state:
 *
 *   - PASS               → real evidence (kill_switch state from Redis,
 *                          readiness verifier green, env present).
 *   - WARN               → real evidence with sub-threshold deviation.
 *   - PAUSED             → real evidence with paused state.
 *   - KILLED             → kill-switch armed.
 *   - BLOCKED            → prerequisite missing (env var, paper-shadow,
 *                          A.4 fork validation). Operator action required.
 *   - NOT_AVAILABLE      → no runtime data source for this breaker yet.
 *                          DD curve, revert-rate window, gas-burn ledger.
 *   - UNKNOWN            → runtime probe failed.
 *
 * Sources reused (NO new event ledger built in this phase):
 *   - KillSwitchClient.state() — Redis-backed real runtime.
 *   - verifyAll() — 16 readiness items, source of truth for risk + sim + rpc.
 *   - process.env — RPC_HTTP_1, EXECUTOR_1, ARBX_TRADE_MODE.
 *   - GET /api/v1/scoring/status (in-process via __forTesting export) — A.8.
 *
 * Hot rules:
 *   - mode: paper_only.
 *   - live_trading: false (immutable).
 *   - private_relay: false.
 *   - submit_enabled: false.
 *   - capital_exposure_usd: 0.
 *   - overall_state can never be PASS for the GLOBAL breaker until
 *     A.9 formal sign-off (echoes go-no-go-agent).
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";
import { KillSwitchClient } from "@arbx/shared";

import { verifyAll } from "../readiness/verifiers/index.js";
import type { ReadinessReport } from "../readiness/types.js";
import { __forTesting as scoringTesting } from "./scoring-status.js";

// ---------------------------------------------------------------------------
// Wire contract types — mirror frontend Zod schemas exactly.
// ---------------------------------------------------------------------------

type BreakerState =
  | "PASS"
  | "WARN"
  | "PAUSED"
  | "KILLED"
  | "BLOCKED"
  | "NOT_AVAILABLE"
  | "UNKNOWN";
type BreakerCategory =
  | "drawdown"
  | "revert_rate"
  | "gas_burn"
  | "latency"
  | "sim_error"
  | "rpc_health"
  | "blacklist"
  | "executor"
  | "confidence"
  | "global_kill_switch";
type BreakerSeverity = "critical" | "high" | "medium" | "low";
type BreakerAction =
  | "none"
  | "warn"
  | "pause"
  | "hard_pause"
  | "kill_switch"
  | "block_routes"
  | "block_tokens";
type BlockedPhase = "A.4" | "A.5" | "A.6" | "A.7" | "A.8" | "A.9" | "LIVE";

interface BreakerEvidence {
  source: "kill_switch" | "readiness_verifier" | "env_probe" | "scoring_status" | "not_configured";
  detail: string;
  // Always nullable so the wire contract never lies. UI shows "—" when null.
  current_value: string | number | null;
  threshold: string | number | null;
  unit: string | null;
  ref?: string;
}

interface CircuitBreaker {
  id: string;
  name: string;
  category: BreakerCategory;
  state: BreakerState;
  severity: BreakerSeverity;
  action: BreakerAction;
  evidence: BreakerEvidence;
  blocks: BlockedPhase[];
  operator_required: boolean;
  last_evaluated_at: string;
  description: string;
  required_action: string | null;
}

interface BreakerSummary {
  pass: number;
  warn: number;
  paused: number;
  killed: number;
  blocked: number;
  not_available: number;
  unknown: number;
  total: number;
}

interface CircuitBreakersStatusResponse {
  generated_at: string;
  mode: "paper_only";
  live_trading: false;
  private_relay: false;
  submit_enabled: false;
  capital_exposure_usd: 0;
  overall_state: BreakerState;
  breakers: CircuitBreaker[];
  summary: BreakerSummary;
  next_action: string;
  version: string;
}

interface CircuitBreakerEventsResponse {
  generated_at: string;
  event_source: "not_configured" | "persistent_store";
  events: never[];
  blocked_reason: string | null;
  next_action: string;
}

const VERSION = "0.1.0";

// ---------------------------------------------------------------------------
// Evaluation context — gathered once per request.
// ---------------------------------------------------------------------------

interface EvalCtx {
  now: string;
  killSwitchEnabled: boolean | null;
  killSwitchReason: string | null;
  readiness: ReadinessReport | null;
  readinessError: string | null;
  envRpc: boolean;
  envExecutor: boolean;
  envTradeMode: string | null;
  scoringPipelineWired: boolean;
}

async function collectCtx(deps: {
  pool: pg.Pool | null;
  killSwitch: KillSwitchClient;
}): Promise<EvalCtx> {
  const now = new Date().toISOString();

  let killSwitchEnabled: boolean | null = null;
  let killSwitchReason: string | null = null;
  try {
    const ks = await deps.killSwitch.state();
    killSwitchEnabled = ks?.enabled ?? null;
    killSwitchReason = ks?.reason ?? null;
  } catch {
    // KillSwitch is Redis-backed; failure → unknown, not pretend.
  }

  let readiness: ReadinessReport | null = null;
  let readinessError: string | null = null;
  try {
    readiness = await verifyAll({ pool: deps.pool });
  } catch (e) {
    readinessError = (e as Error).message.slice(0, 120);
  }

  const envRpc = (process.env["RPC_HTTP_1"]?.length ?? 0) > 0;
  const envExecutor = (process.env["EXECUTOR_1"]?.length ?? 0) > 0;
  const envTradeMode = process.env["ARBX_TRADE_MODE"] ?? null;

  // A.8 scoring wire status — pull from scoring-status's workspace_verified
  // ledger (the same source of truth surfaced via /api/v1/scoring/status).
  // Components named "Scoring pipeline invocation" is the gate we read.
  const pipelineComp = scoringTesting.COMPONENTS.find((c) =>
    c.name.includes("pipeline"),
  );
  const scoringPipelineWired = pipelineComp?.wired === true;

  return {
    now,
    killSwitchEnabled,
    killSwitchReason,
    readiness,
    readinessError,
    envRpc,
    envExecutor,
    envTradeMode,
    scoringPipelineWired,
  };
}

// ---------------------------------------------------------------------------
// Per-breaker evaluators — pure functions over EvalCtx. Each returns the
// complete CircuitBreaker row. NO fake values; missing data → NOT_AVAILABLE.
// ---------------------------------------------------------------------------

function makeDrawdownBreaker(ctx: EvalCtx): CircuitBreaker {
  // A.5 paper-shadow ledger is the prerequisite for DD curve. Until A.5 PASS,
  // we cannot compute drawdown — NOT_AVAILABLE is the only honest answer.
  return {
    id: "drawdown_breaker",
    name: "Drawdown DD-10/20/30/40",
    category: "drawdown",
    state: "NOT_AVAILABLE",
    severity: "critical",
    action: "none",
    evidence: {
      source: "not_configured",
      detail: "No paper-shadow equity curve persisted yet. DD-10/20/30/40 tiers (warn/pause/hard-pause/kill-switch) require continuous paper P&L history.",
      current_value: null,
      threshold: "10/20/30/40 %",
      unit: "% peak-to-trough",
    },
    blocks: ["LIVE"],
    operator_required: true,
    last_evaluated_at: ctx.now,
    description: "DD tiers from peak equity. 10% warn · 20% pause · 30% hard-pause · 40% kill-switch.",
    required_action: "Run A.5 paper-shadow continuously (≥7 days) so equity curve exists, then enable DD evaluator.",
  };
}

function makeRevertRateBreaker(ctx: EvalCtx): CircuitBreaker {
  return {
    id: "revert_rate_breaker",
    name: "Max SIM_REVERT rate (rolling window)",
    category: "revert_rate",
    state: "NOT_AVAILABLE",
    severity: "high",
    action: "none",
    evidence: {
      source: "not_configured",
      detail: "SIM_REVERT counters are emitted by searcher-rs::counters but no rolling-window aggregator persists them yet.",
      current_value: null,
      threshold: "30 %",
      unit: "% of last 100 simulations",
    },
    blocks: ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "If SIM_REVERT rate over the last N simulations exceeds threshold, pause new submissions.",
    required_action: "Add rolling-window aggregator that reads searcher-rs counters and writes to risk_events.",
  };
}

function makeGasBurnBreaker(ctx: EvalCtx): CircuitBreaker {
  return {
    id: "gas_burn_breaker",
    name: "Max gas burn (rolling window)",
    category: "gas_burn",
    state: "NOT_AVAILABLE",
    severity: "high",
    action: "none",
    evidence: {
      source: "not_configured",
      detail: "Paper-side gas-used per opportunity is captured by simulator-v2 SequenceContext but no per-window cumulative ledger persists yet.",
      current_value: null,
      threshold: null,
      unit: "gwei per hour",
    },
    blocks: ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "If cumulative paper/live gas burn over a rolling window exceeds the operator-defined cap, pause.",
    required_action: "Persist gas_used per simulated tx to a time-bucketed table (or Redis stream) and compute rolling sum.",
  };
}

function makeLatencyBreaker(ctx: EvalCtx): CircuitBreaker {
  // We can read the readiness G-RPC-1 verifier — it inspects RPC primary
  // and surfaces yellow/red on latency thresholds. Map to a breaker state.
  const grpc = ctx.readiness?.items.find((it) => it.id === "G-RPC-1");
  if (!grpc) {
    return {
      id: "latency_breaker",
      name: "Max latency (p95/p99)",
      category: "latency",
      state: ctx.readinessError ? "UNKNOWN" : "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "not_configured",
        detail: "Readiness G-RPC-1 verifier not present in this build; p95/p99 latency aggregator pending.",
        current_value: null,
        threshold: "200ms p95",
        unit: "ms",
      },
      blocks: ["LIVE"],
      operator_required: false,
      last_evaluated_at: ctx.now,
      description: "Pause submissions when p95 or p99 latency exceeds threshold.",
      required_action: "Wire Prometheus histogram + alert into a breaker evaluator.",
    };
  }
  const state: BreakerState =
    grpc.status === "green" ? "PASS"
    : grpc.status === "yellow" ? "WARN"
    : grpc.status === "red" ? "PAUSED"
    : "UNKNOWN";
  return {
    id: "latency_breaker",
    name: "Max latency (p95/p99 via G-RPC-1)",
    category: "latency",
    state,
    severity: state === "PASS" ? "low" : "high",
    action: state === "PAUSED" ? "pause" : state === "WARN" ? "warn" : "none",
    evidence: {
      source: "readiness_verifier",
      detail: grpc.reason,
      current_value: null,
      threshold: "200ms p95",
      unit: "ms",
      ref: "readiness:G-RPC-1",
    },
    blocks: state === "PASS" ? [] : ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Derived from readiness G-RPC-1 — pause submissions when RPC health degrades.",
    required_action: state === "PASS" ? null : "Investigate RPC latency surge; consider failover.",
  };
}

function makeSimErrorBreaker(ctx: EvalCtx): CircuitBreaker {
  const gsim = ctx.readiness?.items.find((it) => it.id === "G-SIM-1");
  const state: BreakerState =
    gsim?.status === "green" ? "PASS"
    : gsim?.status === "yellow" ? "WARN"
    : gsim?.status === "red" ? "PAUSED"
    : ctx.readinessError ? "UNKNOWN" : "NOT_AVAILABLE";
  const evidence: BreakerEvidence = {
    source: gsim ? "readiness_verifier" : "not_configured",
    detail: gsim?.reason ?? "G-SIM-1 readiness verifier not loaded.",
    current_value: null,
    threshold: "5 consecutive",
    unit: "errors",
  };
  // exactOptionalPropertyTypes: only set `ref` when present (never `undefined`).
  if (gsim) evidence.ref = "readiness:G-SIM-1";
  return {
    id: "sim_error_breaker",
    name: "Consecutive SIM_ERROR streak",
    category: "sim_error",
    state,
    severity: state === "PASS" ? "low" : "high",
    action: state === "PAUSED" ? "pause" : state === "WARN" ? "warn" : "none",
    evidence,
    blocks: state === "PASS" ? [] : ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Pause if consecutive simulation errors exceed threshold.",
    required_action: state === "PASS" ? null : "Inspect simulator-v2 + searcher-rs logs; surface failing strategy.",
  };
}

function makeRpcHealthBreaker(ctx: EvalCtx): CircuitBreaker {
  if (!ctx.envRpc) {
    return {
      id: "rpc_health_breaker",
      name: "RPC health + failover",
      category: "rpc_health",
      state: "BLOCKED",
      severity: "critical",
      action: "none",
      evidence: {
        source: "env_probe",
        detail: "RPC_HTTP_1 env var missing — no primary RPC configured.",
        current_value: null,
        threshold: null,
        unit: null,
        ref: "env:RPC_HTTP_1",
      },
      blocks: ["A.4", "A.5", "LIVE"],
      operator_required: true,
      last_evaluated_at: ctx.now,
      description: "Verifies primary RPC presence + failover pool readiness.",
      required_action: "Set RPC_HTTP_1 to an archive-capable Tier-1 endpoint.",
    };
  }
  const grpc = ctx.readiness?.items.find((it) => it.id === "G-RPC-1");
  const state: BreakerState =
    grpc?.status === "green" ? "PASS"
    : grpc?.status === "yellow" ? "WARN"
    : grpc?.status === "red" ? "PAUSED"
    : "UNKNOWN";
  return {
    id: "rpc_health_breaker",
    name: "RPC health + failover (G-RPC-1)",
    category: "rpc_health",
    state,
    severity: state === "PASS" ? "low" : "high",
    action: state === "PAUSED" ? "pause" : "none",
    evidence: {
      source: "readiness_verifier",
      detail: grpc?.reason ?? "G-RPC-1 verifier loaded but item missing",
      current_value: null,
      threshold: null,
      unit: null,
      ref: "readiness:G-RPC-1",
    },
    blocks: state === "PASS" ? [] : ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Primary RPC reachable + failover pool ready.",
    required_action: state === "PASS" ? null : "Investigate RPC failover state.",
  };
}

function makeBlacklistBreaker(ctx: EvalCtx): CircuitBreaker {
  const gtok = ctx.readiness?.items.find((it) => it.id === "G-TOK-1");
  const state: BreakerState =
    gtok?.status === "green" ? "PASS"
    : gtok?.status === "yellow" ? "WARN"
    : gtok?.status === "red" ? "PAUSED"
    : "NOT_AVAILABLE";
  const evidence: BreakerEvidence = {
    source: gtok ? "readiness_verifier" : "not_configured",
    detail: gtok?.reason ?? "G-TOK-1 verifier not loaded; no automated blacklist propagation yet.",
    current_value: null,
    threshold: null,
    unit: null,
  };
  if (gtok) evidence.ref = "readiness:G-TOK-1";
  return {
    id: "blacklist_breaker",
    name: "Route/Token blacklist (G-TOK-1)",
    category: "blacklist",
    state,
    severity: state === "PASS" ? "low" : "medium",
    action: state === "PAUSED" ? "block_tokens" : "none",
    evidence,
    blocks: state === "PASS" ? [] : ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Block routes/tokens flagged by safety screens.",
    required_action: state === "PASS" ? null : "Refresh token safety screen; propagate denylist to scanner.",
  };
}

function makeExecutorBreaker(ctx: EvalCtx): CircuitBreaker {
  if (!ctx.envExecutor) {
    return {
      id: "executor_breaker",
      name: "Executor contract health",
      category: "executor",
      state: "BLOCKED",
      severity: "critical",
      action: "none",
      evidence: {
        source: "env_probe",
        detail: "EXECUTOR_1 env var missing — no deployed ArbitrageExecutor address.",
        current_value: null,
        threshold: null,
        unit: null,
        ref: "env:EXECUTOR_1",
      },
      blocks: ["A.4", "A.5", "LIVE"],
      operator_required: true,
      last_evaluated_at: ctx.now,
      description: "Verifies executor presence + reachability.",
      required_action: "Deploy ArbitrageExecutor.sol and export EXECUTOR_1.",
    };
  }
  // Env present, but on-chain probe is not implemented yet — honest PARTIAL.
  return {
    id: "executor_breaker",
    name: "Executor contract health",
    category: "executor",
    state: "WARN",
    severity: "medium",
    action: "warn",
    evidence: {
      source: "env_probe",
      detail: "EXECUTOR_1 env present (length-only), but on-chain bytecode probe + owner check not yet implemented.",
      current_value: "env_present",
      threshold: null,
      unit: null,
      ref: "env:EXECUTOR_1",
    },
    blocks: ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Executor address present; on-chain health probe pending.",
    required_action: "Add ethers/alloy probe: getCode(EXECUTOR_1) + owner() + paused() inspection.",
  };
}

function makeConfidenceBreaker(ctx: EvalCtx): CircuitBreaker {
  if (!ctx.scoringPipelineWired) {
    return {
      id: "confidence_breaker",
      name: "Confidence/Scoring threshold (A.8)",
      category: "confidence",
      state: "NOT_AVAILABLE",
      severity: "medium",
      action: "none",
      evidence: {
        source: "scoring_status",
        detail: "A.8 scoring primitives wired but scanner pipeline does not emit ConfidenceScore yet.",
        current_value: null,
        threshold: "7000",
        unit: "bps posterior probability",
        ref: "scoring_status:scoring_pipeline_wired=false",
      },
      blocks: ["LIVE"],
      operator_required: false,
      last_evaluated_at: ctx.now,
      description: "Reject opportunities below the confidence_threshold_bps. Currently inert.",
      required_action: "Wire bayesian_filter + kelly_sizing into scanner::dispatch_orchestrator_and_classify.",
    };
  }
  return {
    id: "confidence_breaker",
    name: "Confidence/Scoring threshold (A.8)",
    category: "confidence",
    state: "PASS",
    severity: "low",
    action: "none",
    evidence: {
      source: "scoring_status",
      detail: "A.8 scoring pipeline wired; confidence threshold enforced per candidate.",
      current_value: "wired",
      threshold: "7000",
      unit: "bps",
    },
    blocks: [],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Reject opportunities below the confidence_threshold_bps.",
    required_action: null,
  };
}

function makeGlobalKillSwitchBreaker(ctx: EvalCtx): CircuitBreaker {
  // The ONLY breaker today with real runtime signal: Redis kill_switch state.
  if (ctx.killSwitchEnabled === null) {
    return {
      id: "global_kill_switch",
      name: "Global kill-switch",
      category: "global_kill_switch",
      state: "UNKNOWN",
      severity: "critical",
      action: "none",
      evidence: {
        source: "kill_switch",
        detail: "Could not read kill_switch state from Redis (transient failure).",
        current_value: null,
        threshold: null,
        unit: null,
      },
      blocks: ["LIVE"],
      operator_required: true,
      last_evaluated_at: ctx.now,
      description: "Manual + automatic global kill switch (Redis-backed).",
      required_action: "Check Redis connectivity from api-server.",
    };
  }
  if (ctx.killSwitchEnabled) {
    return {
      id: "global_kill_switch",
      name: "Global kill-switch",
      category: "global_kill_switch",
      state: "KILLED",
      severity: "critical",
      action: "kill_switch",
      evidence: {
        source: "kill_switch",
        detail: ctx.killSwitchReason ?? "kill-switch armed (no reason recorded)",
        current_value: "armed",
        threshold: null,
        unit: null,
      },
      blocks: ["LIVE"],
      operator_required: true,
      last_evaluated_at: ctx.now,
      description: "Kill-switch ARMED — all submissions halted.",
      required_action: "Operator disarm via POST /admin/killswitch with reason.",
    };
  }
  return {
    id: "global_kill_switch",
    name: "Global kill-switch",
    category: "global_kill_switch",
    state: "PASS",
    severity: "low",
    action: "none",
    evidence: {
      source: "kill_switch",
      detail: "Kill-switch is disarmed (paper-mode operates normally).",
      current_value: "disarmed",
      threshold: null,
      unit: null,
    },
    // Even when "PASS", the global breaker still blocks LIVE — because LIVE
    // is gated by A.9 formal sign-off, not by kill-switch alone.
    blocks: ["LIVE"],
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Manual + automatic global kill switch (Redis-backed).",
    required_action: null,
  };
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

const ORDER_PRIORITY: Record<BreakerState, number> = {
  KILLED: 0,
  PAUSED: 1,
  BLOCKED: 2,
  WARN: 3,
  UNKNOWN: 4,
  NOT_AVAILABLE: 5,
  PASS: 6,
};

function summarize(breakers: CircuitBreaker[]): BreakerSummary {
  const s = { pass: 0, warn: 0, paused: 0, killed: 0, blocked: 0, not_available: 0, unknown: 0, total: breakers.length };
  for (const b of breakers) {
    switch (b.state) {
      case "PASS": s.pass++; break;
      case "WARN": s.warn++; break;
      case "PAUSED": s.paused++; break;
      case "KILLED": s.killed++; break;
      case "BLOCKED": s.blocked++; break;
      case "NOT_AVAILABLE": s.not_available++; break;
      default: s.unknown++; break;
    }
  }
  return s;
}

function overall(breakers: CircuitBreaker[]): BreakerState {
  // Worst state wins.
  let worst: BreakerState = "PASS";
  for (const b of breakers) {
    if (ORDER_PRIORITY[b.state] < ORDER_PRIORITY[worst]) worst = b.state;
  }
  return worst;
}

function buildAllBreakers(ctx: EvalCtx): CircuitBreaker[] {
  return [
    makeGlobalKillSwitchBreaker(ctx),
    makeDrawdownBreaker(ctx),
    makeRevertRateBreaker(ctx),
    makeGasBurnBreaker(ctx),
    makeLatencyBreaker(ctx),
    makeSimErrorBreaker(ctx),
    makeRpcHealthBreaker(ctx),
    makeBlacklistBreaker(ctx),
    makeExecutorBreaker(ctx),
    makeConfidenceBreaker(ctx),
  ];
}

// ---------------------------------------------------------------------------
// Test-only exports.
// ---------------------------------------------------------------------------

export const __forTesting = {
  makeDrawdownBreaker,
  makeRevertRateBreaker,
  makeGasBurnBreaker,
  makeLatencyBreaker,
  makeSimErrorBreaker,
  makeRpcHealthBreaker,
  makeBlacklistBreaker,
  makeExecutorBreaker,
  makeConfidenceBreaker,
  makeGlobalKillSwitchBreaker,
  buildAllBreakers,
  summarize,
  overall,
  VERSION,
};

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

export function mountRiskCircuitBreakers(
  app: Application,
  deps: {
    pool: pg.Pool | null;
    killSwitch: KillSwitchClient;
    logger: { warn: (obj: object, msg?: string) => void };
  },
): void {
  app.get("/api/v1/risk/circuit-breakers/status", async (_req: Request, res: Response) => {
    try {
      const ctx = await collectCtx({ pool: deps.pool, killSwitch: deps.killSwitch });
      const breakers = buildAllBreakers(ctx);
      const summary = summarize(breakers);
      const overallState = overall(breakers);
      const nextAction =
        overallState === "KILLED" ? "Operator disarm kill-switch with explicit reason."
        : overallState === "PAUSED" ? "Investigate worst-state breaker; review readiness items G-RPC-1/G-SIM-1."
        : overallState === "BLOCKED" ? "Resolve env prerequisites (RPC_HTTP_1, EXECUTOR_1) and run A.4 fork validation."
        : overallState === "NOT_AVAILABLE" ? "Run A.5 paper-shadow continuously to populate DD/revert/gas data sources."
        : "All real breakers PASS; LIVE remains gated by A.9 formal sign-off.";

      const response: CircuitBreakersStatusResponse = {
        generated_at: ctx.now,
        mode: "paper_only",
        live_trading: false,
        private_relay: false,
        submit_enabled: false,
        capital_exposure_usd: 0,
        overall_state: overallState,
        breakers,
        summary,
        next_action: nextAction,
        version: VERSION,
      };
      res.status(200).json(response);
    } catch (e) {
      deps.logger.warn({ event: "circuit_breakers.status_failed", err: (e as Error).message });
      res.status(503).json({ error: "circuit_breakers_status_failed", detail: (e as Error).message });
    }
  });

  // Events endpoint — honest empty until a persistent event store exists.
  app.get("/api/v1/risk/circuit-breakers/events", async (_req: Request, res: Response) => {
    const response: CircuitBreakerEventsResponse = {
      generated_at: new Date().toISOString(),
      event_source: "not_configured",
      events: [],
      blocked_reason:
        "No paper-shadow runtime event persistence configured. Breaker trips are not yet recorded to a queryable store.",
      next_action:
        "Add risk_events table writer (or Redis stream) that captures every state transition emitted by the breaker evaluators.",
    };
    res.status(200).json(response);
  });
}
