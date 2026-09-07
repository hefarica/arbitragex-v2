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
 *   - NOT_AVAILABLE      → no honest data for this breaker right now
 *                          (threshold unconfigured, query failed, empty or
 *                          insufficient ledger window). Never zero-fabricated.
 *   - UNKNOWN            → runtime probe failed.
 *
 * Sources reused:
 *   - KillSwitchClient.state() — Redis-backed real runtime.
 *   - verifyAll() — 16 readiness items, source of truth for risk + sim + rpc.
 *   - process.env — RPC_HTTP_1, EXECUTOR_1, ARBX_TRADE_MODE, ARBX_CB_*.
 *   - GET /api/v1/scoring/status (in-process via __forTesting export) — A.8.
 *   - paper_trade_runs (chain-filtered) — equity marks for the DD breaker,
 *     reason-pattern revert classification, and actual-gas-burn sums. The
 *     ledger EXISTS in prod (A.5+); these evaluators read it read-only and
 *     stay NOT_AVAILABLE (with an R8 reason) whenever it cannot answer.
 *
 * Trip persistence: on TRANSITION to a tripped state (KILLED/PAUSED) one row
 * is written to risk_events (event_type 'circuit_breaker' / 'kill_switch',
 * source_service 'api-server', breaker_id in payload) — best-effort, deduped
 * per episode so repeated polls do not flood the table.
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
import {
  KillSwitchClient,
  riskCbEvalFailuresTotal,
  riskCbLastEvalUnixtime,
  riskCbStateGauge,
  riskCbTripsTotal,
} from "@arbx/shared";

import { verifyAll } from "../readiness/verifiers/index.js";
import type { ReadinessReport } from "../readiness/types.js";
import { isScoringPipelineWired } from "./scoring-status.js";
import {
  computeBreakerWindow,
  loadThresholdsFromEnv,
  type BreakerMetric,
  type TradeOutcome,
} from "../risk/rolling-breakers.js";

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
  source: "kill_switch" | "readiness_verifier" | "env_probe" | "scoring_status" | "paper_ledger" | "not_configured";
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

/** One persisted trip row from risk_events (payload carries breaker_id/state). */
interface RiskEventRow {
  id: string;
  event_type: string;
  severity: string;
  source_service: string;
  payload: unknown;
  created_at: string;
}

interface CircuitBreakerEventsResponse {
  generated_at: string;
  event_source: "not_configured" | "persistent_store";
  events: RiskEventRow[];
  blocked_reason: string | null;
  next_action: string;
}

const VERSION = "0.2.0";

// ---------------------------------------------------------------------------
// Ledger-fed breaker config (ARBX_CB_* env) + pure math.
//
// Doctrine: trip thresholds are operator-supplied — when absent the breaker
// reports NOT_AVAILABLE, we never invent a cap (arbx-no-hardcode). The window
// and chain are structural (not trip thresholds) so they carry code defaults
// documented in .env.example. DD tiers default to the doctrine 10/20/30/40
// unless ARBX_RISK_DD_TIERS overrides (same var the rolling-breaker evaluator
// uses — one tier definition per deployment).
// ---------------------------------------------------------------------------

/** Evidence floors for the DD curve — below these a DD% is noise, not signal (R8). */
const DD_MIN_RUNS = 100;
const DD_MIN_SPAN_HOURS = 24;

type DdTierSet = readonly [warn: number, pause: number, hardPause: number, kill: number];
const DOCTRINE_DD_TIERS: DdTierSet = [10, 20, 30, 40];

interface CbConfig {
  chainId: number;
  /** Rolling window (hours) shared by revert-rate + actual-gas breakers. null = misconfigured. */
  windowHours: number | null;
  /** ARBX_CB_MAX_REVERT_RATE in % — null = absent (not_configured) or invalid. */
  maxRevertRatePct: number | null;
  /** True when ARBX_CB_MAX_REVERT_RATE is present in env (set-but-invalid vs absent). */
  revertRateSet: boolean;
  /** ARBX_CB_MAX_GAS_BURN_USD — null = absent (falls back to ARBX_RISK_* sim-gas) or invalid. */
  maxGasBurnUsd: number | null;
  /** True when ARBX_CB_MAX_GAS_BURN_USD is present in env (set-but-invalid vs absent). */
  gasBurnSet: boolean;
  /** ARBX_RISK_NAV_USD equity anchor for the DD curve; 0 = unanchored (raw cumulative PnL). */
  navUsd: number;
  ddTiers: DdTierSet;
}

export function loadCbConfig(env: NodeJS.ProcessEnv = process.env): CbConfig {
  const chainId = Number(env["ARBX_CB_CHAIN_ID"]);
  // Empty string (or absent) means "unset → structural default 24h", NOT invalid.
  const windowRaw = (env["ARBX_CB_REVERT_WINDOW_H"] ?? "").trim();
  const windowHours = windowRaw.length === 0 ? 24 : Number(windowRaw);
  const revertRateRaw = (env["ARBX_CB_MAX_REVERT_RATE"] ?? "").trim();
  const revertRate = Number(revertRateRaw);
  const gasCapRaw = (env["ARBX_CB_MAX_GAS_BURN_USD"] ?? "").trim();
  const gasCap = Number(gasCapRaw);
  const nav = Number(env["ARBX_RISK_NAV_USD"]);
  const tiers = (env["ARBX_RISK_DD_TIERS"] ?? "").split(",").map((s) => Number(s.trim()));

  const revertRateSet = revertRateRaw.length > 0;
  const gasCapSet = gasCapRaw.length > 0;

  return {
    chainId: Number.isFinite(chainId) && Number.isInteger(chainId) && chainId > 0 ? chainId : 1,
    windowHours: Number.isFinite(windowHours) && windowHours > 0 ? windowHours : null,
    maxRevertRatePct:
      revertRateSet && Number.isFinite(revertRate) && revertRate > 0 && revertRate <= 100 ? revertRate : null,
    revertRateSet,
    maxGasBurnUsd: gasCapSet && Number.isFinite(gasCap) && gasCap > 0 ? gasCap : null,
    gasBurnSet: gasCapSet,
    navUsd: Number.isFinite(nav) && nav > 0 ? nav : 0,
    ddTiers:
      tiers.length === 4 && tiers.every((x) => Number.isFinite(x) && x > 0)
        ? (tiers as unknown as DdTierSet)
        : DOCTRINE_DD_TIERS,
  };
}

/** One hourly equity mark from paper_trade_runs (chain-filtered). */
interface DrawdownMark {
  markAt: string;
  runs: number;
  pnlUsd: number;
}

interface DrawdownStats {
  samples: number;
  spanHours: number;
  peakUsd: number;
  maxDdUsd: number;
  /** null when the peak is <= 0 — % of a non-positive peak is meaningless (fail-honest). */
  maxDdPct: number | null;
}

/**
 * Peak-to-trough drawdown over the marked equity curve (equity = nav + Σ pnl).
 * Mirrors the rolling-breaker/risk_ledger walk: track running peak, deepest
 * drop after it; % measured against peak equity. Flat marks between trades
 * cannot deepen the drawdown, so hourly marks are sufficient granularity.
 */
export function computeDrawdownStats(marks: readonly DrawdownMark[], navUsd: number): DrawdownStats {
  let equity = navUsd;
  let peak = navUsd;
  let maxDdUsd = 0;
  let maxDdPct: number | null = null;
  let samples = 0;
  for (const m of marks) {
    equity += m.pnlUsd;
    samples += m.runs;
    if (equity > peak) peak = equity;
    const ddUsd = peak - equity;
    if (ddUsd > maxDdUsd) maxDdUsd = ddUsd;
    if (peak > 0) {
      const ddPct = (ddUsd / peak) * 100;
      if (maxDdPct === null || ddPct > maxDdPct) maxDdPct = ddPct;
    }
  }
  const first = marks.length > 0 ? Date.parse(marks[0]!.markAt) : NaN;
  const last = marks.length > 0 ? Date.parse(marks[marks.length - 1]!.markAt) : NaN;
  const spanHours = Number.isFinite(first) && Number.isFinite(last) ? (last - first) / 3_600_000 : 0;
  return { samples, spanHours, peakUsd: peak, maxDdUsd, maxDdPct };
}

/** DD tier crossing → breaker state/action/severity (10 warn · 20 pause · 30 hard-pause · 40 kill). */
function classifyDrawdown(pct: number, t: DdTierSet): { state: BreakerState; action: BreakerAction; severity: BreakerSeverity } {
  if (pct >= t[3]) return { state: "KILLED", action: "kill_switch", severity: "critical" };
  if (pct >= t[2]) return { state: "PAUSED", action: "hard_pause", severity: "critical" };
  if (pct >= t[1]) return { state: "PAUSED", action: "pause", severity: "high" };
  if (pct >= t[0]) return { state: "WARN", action: "warn", severity: "medium" };
  return { state: "PASS", action: "none", severity: "low" };
}

/** Per-tier tripped/PASS report — the evidence detail exposes every tier state. */
function ddTierReport(pct: number, t: DdTierSet): string {
  return t.map((tier) => `${tier}%:${pct >= tier ? "TRIPPED" : "PASS"}`).join(" · ");
}

// ---------------------------------------------------------------------------
// Evaluation context — gathered once per request.
// ---------------------------------------------------------------------------

/** Ledger-fed breaker data collected once per request (A.6).
 *  Every `reason` is the R8 explanation for why its breaker must NOT_AVAILABLE. */
interface LedgerBreakerData {
  drawdown: {
    /** Hourly equity marks over the full chain ledger; null = query failed / no pool. */
    marks: DrawdownMark[] | null;
    reason: string;
    navUsd: number;
    tiers: DdTierSet;
  };
  revertRate: {
    windowHours: number;
    /** null = query failed / invalid window. */
    counts: { total: number; reverted: number } | null;
    reason: string;
    /** null = threshold absent or invalid (reason below). */
    thresholdPct: number | null;
    thresholdReason: string;
  };
  gasBurn: {
    windowHours: number;
    /** Machine-readable path status (drives fail-closed vs legacy fallback). */
    status: "ok" | "cap_not_configured" | "cap_invalid" | "window_invalid" | "no_pool" | "query_failed";
    /** null = window not evaluated. */
    window: { rowsInWindow: number; withActualGas: number; sumUsd: number } | null;
    reason: string;
    capUsd: number | null;
  };
}

interface EvalCtx {
  now: string;
  chainId: number;
  killSwitchEnabled: boolean | null;
  killSwitchReason: string | null;
  readiness: ReadinessReport | null;
  readinessError: string | null;
  envRpc: boolean;
  envExecutor: boolean;
  envTradeMode: string | null;
  scoringPipelineWired: boolean;
  // A.5 rolling gas-burn from paper_trade_runs (null = thresholds unset / no pool / query failed).
  gasBurn: BreakerMetric | null;
  gasBurnReason: string;
  gasBurnCapUsd: number | null;
  // A.6 ledger-fed DD / revert-rate / actual-gas data.
  ledger: LedgerBreakerData;
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

  // A.8 scoring wire status — derive from the same runtime evidence the
  // /api/v1/scoring/status surface uses (scored_opportunities table presence).
  const scoringPipelineWired = await isScoringPipelineWired(deps.pool);

  // A.5 — rolling gas-burn breaker from paper_trade_runs (sim gas, ARBX_RISK_*
  // thresholds). Math is the TS mirror of shared-rs/src/risk_ledger.rs.
  let gasBurn: BreakerMetric | null = null;
  let gasBurnReason = "thresholds_not_configured";
  let gasBurnCapUsd: number | null = null;
  const thresholds = loadThresholdsFromEnv();
  if (!thresholds) {
    gasBurnReason = "operator risk thresholds not configured (ARBX_RISK_* env)";
  } else if (!deps.pool) {
    gasBurnReason = "no database pool";
  } else {
    gasBurnCapUsd = thresholds.maxGasBurnUsd;
    try {
      const r = await deps.pool.query(
        `SELECT extract(epoch from created_at)::float8 AS ts,
                sim_expected_profit_usd::float8 AS pnl,
                COALESCE(sim_gas_cost_usd, 0)::float8 AS gas
           FROM paper_trade_runs
          WHERE created_at >= NOW() - make_interval(secs => $1)
          ORDER BY created_at ASC`,
        [thresholds.windowSecs],
      );
      const outcomes: TradeOutcome[] = (r.rows as Array<{ ts: number; pnl: number; gas: number }>).map(
        (row) => ({ tsUnix: Number(row.ts), pnlUsd: Number(row.pnl), gasUsd: Number(row.gas) }),
      );
      const win = computeBreakerWindow(Date.now() / 1000, outcomes, thresholds);
      gasBurn = win.gasBurnUsd;
      gasBurnReason = win.gasBurnUsd.sufficient
        ? "ok"
        : `insufficient samples (${win.windowSamples}/${thresholds.minSamples})`;
    } catch (e) {
      gasBurnReason = `query_failed: ${(e as Error).message.slice(0, 80)}`;
    }
  }

  // A.6 — ledger-fed breakers. The paper ledger exists in prod; these are the
  // honest DD-curve / revert-rate / actual-gas feeds. Every failure path keeps
  // an R8 reason so the evaluator reports NOT_AVAILABLE, never a fabricated 0.
  const cb = loadCbConfig();

  let ddMarks: DrawdownMark[] | null = null;
  let ddReason = "no database pool";
  if (deps.pool) {
    try {
      const r = await deps.pool.query(
        `SELECT date_trunc('hour', created_at) AS mark_at,
                COUNT(*)::int AS runs,
                SUM(COALESCE(actual_profit_usd, sim_expected_profit_usd, 0))::float8 AS pnl
           FROM paper_trade_runs
          WHERE chain_id = $1
          GROUP BY 1
          ORDER BY 1`,
        [cb.chainId],
      );
      ddMarks = (r.rows as Array<{ mark_at: Date | string; runs: number; pnl: number }>).map((row) => ({
        markAt: new Date(row.mark_at).toISOString(),
        runs: Number(row.runs),
        pnlUsd: Number(row.pnl),
      }));
      ddReason = "ok";
    } catch (e) {
      ddReason = `query_failed: ${(e as Error).message.slice(0, 80)}`;
    }
  }

  // Revert-rate window. Classification rule is explicit and surfaced in the
  // breaker evidence: a run counts as reverted when its recorded `reason`
  // matches the revert pattern (ILIKE '%revert%').
  let revertCounts: { total: number; reverted: number } | null = null;
  let revertReason = cb.windowHours === null ? "invalid ARBX_CB_REVERT_WINDOW_H" : "no database pool";
  if (cb.windowHours !== null && deps.pool) {
    try {
      const r = await deps.pool.query(
        `SELECT COUNT(*)::int AS total,
                COUNT(*) FILTER (WHERE reason ILIKE '%revert%')::int AS reverted
           FROM paper_trade_runs
          WHERE chain_id = $1
            AND created_at >= NOW() - make_interval(hours => $2)`,
        [cb.chainId, cb.windowHours],
      );
      const row = (r.rows as Array<{ total: number; reverted: number }>)[0];
      revertCounts = { total: Number(row?.total ?? 0), reverted: Number(row?.reverted ?? 0) };
      revertReason = "ok";
    } catch (e) {
      revertReason = `query_failed: ${(e as Error).message.slice(0, 80)}`;
    }
  }
  const revertThresholdPct = cb.maxRevertRatePct;
  const revertThresholdReason =
    cb.maxRevertRatePct !== null ? ""
    : cb.revertRateSet ? "invalid ARBX_CB_MAX_REVERT_RATE (expected 0 < pct <= 100)"
    : "ARBX_CB_MAX_REVERT_RATE not configured";

  // Actual-gas-burn window (same window var as revert). When the cap is absent
  // the CB path stays off and the A.5 sim-gas evaluator above remains the gas
  // breaker's data source; when the cap is set but invalid we fail closed.
  type GasStatus = "ok" | "cap_not_configured" | "cap_invalid" | "window_invalid" | "no_pool" | "query_failed";
  let gasWindow: { rowsInWindow: number; withActualGas: number; sumUsd: number } | null = null;
  let gasStatus: GasStatus;
  let gasReason: string;
  if (cb.gasBurnSet && cb.maxGasBurnUsd === null) {
    gasStatus = "cap_invalid";
    gasReason = "invalid ARBX_CB_MAX_GAS_BURN_USD (expected > 0)";
  } else if (cb.maxGasBurnUsd === null) {
    gasStatus = "cap_not_configured";
    gasReason = "ARBX_CB_MAX_GAS_BURN_USD not configured";
  } else if (cb.windowHours === null) {
    gasStatus = "window_invalid";
    gasReason = "invalid ARBX_CB_REVERT_WINDOW_H";
  } else if (!deps.pool) {
    gasStatus = "no_pool";
    gasReason = "no database pool";
  } else {
    gasStatus = "ok";
    gasReason = "ok";
    try {
      const r = await deps.pool.query(
        `SELECT COUNT(*)::int AS rows_in_window,
                COUNT(actual_gas_cost_usd)::int AS with_actual,
                COALESCE(SUM(actual_gas_cost_usd), 0)::float8 AS gas_usd
           FROM paper_trade_runs
          WHERE chain_id = $1
            AND created_at >= NOW() - make_interval(hours => $2)`,
        [cb.chainId, cb.windowHours],
      );
      const row = (r.rows as Array<{ rows_in_window: number; with_actual: number; gas_usd: number }>)[0];
      gasWindow = {
        rowsInWindow: Number(row?.rows_in_window ?? 0),
        withActualGas: Number(row?.with_actual ?? 0),
        sumUsd: Number(row?.gas_usd ?? 0),
      };
    } catch (e) {
      gasStatus = "query_failed";
      gasReason = `query_failed: ${(e as Error).message.slice(0, 80)}`;
    }
  }

  return {
    now,
    chainId: cb.chainId,
    killSwitchEnabled,
    killSwitchReason,
    readiness,
    readinessError,
    envRpc,
    envExecutor,
    envTradeMode,
    scoringPipelineWired,
    gasBurn,
    gasBurnReason,
    gasBurnCapUsd,
    ledger: {
      drawdown: { marks: ddMarks, reason: ddReason, navUsd: cb.navUsd, tiers: cb.ddTiers },
      revertRate: {
        windowHours: cb.windowHours ?? 24,
        counts: revertCounts,
        reason: revertReason,
        thresholdPct: revertThresholdPct,
        thresholdReason: revertThresholdReason,
      },
      gasBurn: {
        windowHours: cb.windowHours ?? 24,
        status: gasStatus,
        window: gasWindow,
        reason: gasReason,
        capUsd: cb.maxGasBurnUsd,
      },
    },
  };
}

// ---------------------------------------------------------------------------
// Per-breaker evaluators — pure functions over EvalCtx. Each returns the
// complete CircuitBreaker row. NO fake values; missing data → NOT_AVAILABLE.
// ---------------------------------------------------------------------------

function makeDrawdownBreaker(ctx: EvalCtx): CircuitBreaker {
  const dd = ctx.ledger.drawdown;
  const tierLabel = `${dd.tiers.join("/")} %`;
  const base = {
    id: "drawdown_breaker",
    name: "Drawdown DD-10/20/30/40",
    category: "drawdown" as BreakerCategory,
    last_evaluated_at: ctx.now,
    description: "DD tiers from peak equity. 10% warn · 20% pause · 30% hard-pause · 40% kill-switch.",
  };
  // R8 ladder — every NOT_AVAILABLE branch carries the exact reason and NEVER
  // fabricates a drawdown value.
  if (!dd.marks) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "critical",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail: `paper_trade_runs equity curve unavailable — ${dd.reason}.`,
        current_value: null,
        threshold: tierLabel,
        unit: "% peak-to-trough",
      },
      blocks: ["LIVE"],
      operator_required: true,
      required_action: "Restore database access so the equity curve over paper_trade_runs can be read.",
    };
  }
  const stats = computeDrawdownStats(dd.marks, dd.navUsd);
  if (stats.samples < DD_MIN_RUNS || stats.spanHours < DD_MIN_SPAN_HOURS) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "critical",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail:
          `Insufficient evidence: ${stats.samples} runs (<${DD_MIN_RUNS}) over ` +
          `${stats.spanHours.toFixed(1)}h (<${DD_MIN_SPAN_HOURS}h) of paper_trade_runs — a DD% from this would be noise, not signal.`,
        current_value: null,
        threshold: tierLabel,
        unit: "% peak-to-trough",
      },
      blocks: ["LIVE"],
      operator_required: true,
      required_action: `Accumulate ≥${DD_MIN_RUNS} paper runs spanning ≥${DD_MIN_SPAN_HOURS}h, then re-evaluate.`,
    };
  }
  if (stats.maxDdPct === null) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "critical",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail:
          `Peak equity $${stats.peakUsd.toFixed(2)} is ≤ 0 across ${stats.samples} runs — ` +
          "drawdown % of a non-positive peak is undefined. Set ARBX_RISK_NAV_USD to anchor the curve.",
        current_value: null,
        threshold: tierLabel,
        unit: "% peak-to-trough",
      },
      blocks: ["LIVE"],
      operator_required: true,
      required_action: "Set ARBX_RISK_NAV_USD (capital base) so equity = NAV + Σ pnl has a positive peak.",
    };
  }
  const cls = classifyDrawdown(stats.maxDdPct, dd.tiers);
  return {
    ...base,
    state: cls.state,
    severity: cls.severity,
    action: cls.action,
    evidence: {
      source: "paper_ledger",
      detail:
        `Max peak-to-trough drawdown $${stats.maxDdUsd.toFixed(2)} (${stats.maxDdPct.toFixed(2)}% of peak ` +
        `$${stats.peakUsd.toFixed(2)}) over ${stats.samples} paper runs / ${stats.spanHours.toFixed(0)}h ` +
        `(NAV anchor $${dd.navUsd.toFixed(2)}). Tiers: ${ddTierReport(stats.maxDdPct, dd.tiers)}.`,
      current_value: Number(stats.maxDdPct.toFixed(2)),
      threshold: tierLabel,
      unit: "% peak-to-trough",
    },
    blocks: cls.state === "PASS" ? [] : ["LIVE"],
    operator_required: cls.state !== "PASS",
    required_action:
      cls.state === "PASS" ? null
      : cls.state === "WARN" ? "Monitor equity curve; tighten strategy filters before the pause tier trips."
      : "Halt submissions, investigate the losing strategy mix, and disarm only after drawdown recovers below tier.",
  };
}

function makeRevertRateBreaker(ctx: EvalCtx): CircuitBreaker {
  const rr = ctx.ledger.revertRate;
  const classification = "reverted = paper_trade_runs.reason ILIKE '%revert%'";
  const base = {
    id: "revert_rate_breaker",
    name: "Max SIM_REVERT rate (rolling window)",
    category: "revert_rate" as BreakerCategory,
    last_evaluated_at: ctx.now,
    description: "If SIM_REVERT rate over the rolling window exceeds threshold, pause new submissions.",
  };
  if (!rr.counts) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail: `Revert-rate window over paper_trade_runs unavailable — ${rr.reason}.`,
        current_value: null,
        threshold: rr.thresholdPct,
        unit: `% of runs in ${rr.windowHours}h window`,
      },
      blocks: ["LIVE"],
      operator_required: false,
      required_action: "Restore database access / fix ARBX_CB_REVERT_WINDOW_H so the window can be evaluated.",
    };
  }
  if (rr.counts.total === 0) {
    // R8: zero runs is NO evidence — the rate is undefined, never 0%.
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail: `0 paper runs in the last ${rr.windowHours}h — revert rate undefined, not 0%.`,
        current_value: null,
        threshold: rr.thresholdPct,
        unit: `% of runs in ${rr.windowHours}h window`,
      },
      blocks: ["LIVE"],
      operator_required: false,
      required_action: "Accumulate paper runs in the window, then re-evaluate.",
    };
  }
  if (rr.thresholdPct === null) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail:
          `${rr.counts.reverted}/${rr.counts.total} runs match the revert pattern in ${rr.windowHours}h, ` +
          `but the trip threshold is unavailable — ${rr.thresholdReason}.`,
        current_value: null,
        threshold: null,
        unit: `% of runs in ${rr.windowHours}h window`,
      },
      blocks: ["LIVE"],
      operator_required: true,
      required_action: "Set ARBX_CB_MAX_REVERT_RATE (percent) so the rate can be judged — never invented here.",
    };
  }
  const ratePct = (rr.counts.reverted / rr.counts.total) * 100;
  const tripped = ratePct >= rr.thresholdPct;
  return {
    ...base,
    state: tripped ? "PAUSED" : "PASS",
    severity: tripped ? "high" : "low",
    action: tripped ? "pause" : "none",
    evidence: {
      source: "paper_ledger",
      detail:
        `${rr.counts.reverted}/${rr.counts.total} paper runs reverted in the last ${rr.windowHours}h ` +
        `(${ratePct.toFixed(2)}%). Classification rule: ${classification}.`,
      current_value: Number(ratePct.toFixed(2)),
      threshold: rr.thresholdPct,
      unit: `% of runs in ${rr.windowHours}h window`,
    },
    blocks: tripped ? ["LIVE"] : [],
    operator_required: tripped,
    required_action: tripped
      ? "Pause new submissions; inspect reverted-run reasons and the strategies emitting them."
      : null,
  };
}

function makeGasBurnBreaker(ctx: EvalCtx): CircuitBreaker {
  const cbg = ctx.ledger.gasBurn;
  const base = {
    id: "gas_burn_breaker",
    name: "Max gas burn (rolling window)",
    category: "gas_burn" as BreakerCategory,
    operator_required: false,
    last_evaluated_at: ctx.now,
    description: "Cumulative paper/live gas burn over a rolling window vs the operator cap (mirror of risk_ledger).",
  };
  const unit = "USD per window";

  if (cbg.status === "cap_invalid" || cbg.status === "window_invalid") {
    // Misconfigured env fails closed — never silently fall back to the sim path
    // and hide the operator error.
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail: `Actual-gas evaluation unavailable — ${cbg.reason}.`,
        current_value: null,
        threshold: cbg.capUsd,
        unit,
      },
      blocks: ["LIVE"],
      required_action: "Fix the ARBX_CB_* gas-burn configuration so the window can be judged honestly.",
    };
  }

  // --- A.6 path: actual gas burned, summed from the ledger window ---
  let cbState: BreakerState | null = null;
  let cbDetail = "";
  let cbValue: number | null = null;
  const cap = cbg.capUsd;
  const w = cbg.window;
  if (cbg.status !== "cap_not_configured") {
    if (!w) {
      cbState = "NOT_AVAILABLE";
      cbDetail = `actual-gas window unavailable — ${cbg.reason}`;
    } else if (w.rowsInWindow === 0) {
      // R8: an empty window is NO evidence — the sum is undefined, never $0.
      cbState = "NOT_AVAILABLE";
      cbDetail = `0 paper runs in the last ${cbg.windowHours}h — gas burn undefined, not $0`;
    } else if (w.withActualGas === 0) {
      cbState = "NOT_AVAILABLE";
      cbDetail = `${w.rowsInWindow} runs in the window but none has actual_gas_cost_usd recorded yet`;
    } else if (cap !== null) {
      cbValue = Number(w.sumUsd.toFixed(2));
      cbState = w.sumUsd >= cap ? "PAUSED" : "PASS";
      cbDetail =
        `actual gas $${w.sumUsd.toFixed(2)} over ${w.withActualGas} runs with actuals ` +
        `(${w.rowsInWindow} total) in ${cbg.windowHours}h`;
    }
  }

  // --- A.5 path: simulated gas vs ARBX_RISK_* thresholds (preserved) ---
  const m = ctx.gasBurn;
  let simState: BreakerState | null = null;
  let simDetail = "";
  let simValue: number | null = null;
  if (m) {
    simState =
      !m.sufficient ? "NOT_AVAILABLE"
      : m.level === "ok" ? "PASS"
      : m.level === "warn" ? "WARN"
      : m.level === "kill" ? "KILLED"
      : "PAUSED";
    simValue = m.sufficient ? Number(m.value.toFixed(2)) : null;
    simDetail = m.sufficient
      ? `simulated gas $${m.value.toFixed(2)} over ${m.samples} paper runs`
      : ctx.gasBurnReason;
  }

  if (cbState === null && simState === null) {
    return {
      ...base,
      state: "NOT_AVAILABLE",
      severity: "high",
      action: "none",
      evidence: {
        source: "paper_ledger",
        detail: `Rolling gas-burn from paper_trade_runs needs operator thresholds + samples — actual-gas: ${cbg.reason}; sim-gas: ${ctx.gasBurnReason}.`,
        current_value: null,
        threshold: cap ?? ctx.gasBurnCapUsd,
        unit,
      },
      blocks: ["LIVE"],
      required_action:
        "Set ARBX_CB_MAX_GAS_BURN_USD (actual gas) or ARBX_RISK_{NAV_USD,WINDOW_SECS,MIN_SAMPLES,DD_TIERS,GAS_CAP_USD} (sim gas) and accumulate paper_trade_runs.",
    };
  }

  // --- Combine: fail-closed — the worst evaluated path wins the row state ---
  const states: BreakerState[] = [];
  if (cbState !== null) states.push(cbState);
  if (simState !== null) states.push(simState);
  const state = states.reduce<BreakerState>(
    (acc, s) => (ORDER_PRIORITY[s] < ORDER_PRIORITY[acc] ? s : acc),
    "PASS",
  );
  const details = [cbDetail, simDetail].filter((d) => d.length > 0).join(" · ");
  return {
    ...base,
    state,
    severity: state === "PASS" ? "low" : state === "WARN" ? "medium" : "high",
    action: state === "KILLED" ? "hard_pause" : state === "PAUSED" ? "pause" : state === "WARN" ? "warn" : "none",
    evidence: {
      source: "paper_ledger",
      detail: details.length > 0 ? `${details}.` : "no gas evidence in window.",
      // Contract invariant: NOT_AVAILABLE never carries a fabricated value even
      // when the OTHER path (sim) has one — the row state must not lie.
      current_value: state === "NOT_AVAILABLE" ? null : cbValue ?? simValue,
      threshold: cap ?? ctx.gasBurnCapUsd,
      unit,
    },
    blocks: state === "PASS" ? [] : ["LIVE"],
    required_action:
      state === "PASS" || state === "NOT_AVAILABLE"
        ? null
        : "Investigate gas spend; reduce candidate volume or raise the operator cap if intended.",
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
  // Real runtime signal straight from Redis — the fastest breaker on the board.
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
// A.6 Prometheus emission — the evaluated breaker state leaves the process on
// the shared registry (scraped at /metrics) so alerts.rules.yml (group
// `circuit_breakers`) can fire without anyone polling the status endpoint.
// ---------------------------------------------------------------------------

/** Wire-state → gauge value. Mirrors the help string on arbx_risk_cb_state. */
const CB_STATE_METRIC: Record<BreakerState, number> = {
  PASS: 0,
  WARN: 1,
  PAUSED: 2,
  KILLED: 3,
  BLOCKED: 4,
  NOT_AVAILABLE: 5,
  UNKNOWN: 6,
};

function emitBreakerMetrics(breakers: CircuitBreaker[], now: Date): void {
  for (const b of breakers) {
    riskCbStateGauge.labels(b.id).set(CB_STATE_METRIC[b.state]);
  }
  riskCbLastEvalUnixtime.set(Math.floor(now.getTime() / 1000));
}

/** Cadence of the background evaluation loop started by mountRiskCircuitBreakers. */
const CB_EMIT_INTERVAL_MS = 60_000;

// ---------------------------------------------------------------------------
// Trip persistence → risk_events (best-effort, deduplicated per episode).
//
// Shape mirrors the recon / selector-api inserts (migration 009 + 060):
// event_type must satisfy the table CHECK ('circuit_breaker' | 'kill_switch'),
// so the per-breaker identity rides in payload.breaker_id. A row is written
// ONLY on transition into a tripped state (KILLED/PAUSED): the latest
// persisted row for the breaker decides whether this is a new episode, so
// repeated polls while tripped do not flood the table. A persistence failure
// is logged and never breaks the status response.
// ---------------------------------------------------------------------------

const TRIPPED_STATES: readonly BreakerState[] = ["KILLED", "PAUSED"];

/** In-memory episode tracker (per process); seeded from risk_events on cold start. */
const tripEpisodeState = new Map<string, BreakerState>();

function eventTypeForBreaker(id: string): "circuit_breaker" | "kill_switch" {
  return id === "global_kill_switch" ? "kill_switch" : "circuit_breaker";
}

async function latestPersistedTripState(
  pool: pg.Pool,
  breakerId: string,
): Promise<BreakerState | null> {
  const r = await pool.query(
    `SELECT payload->>'state' AS state
       FROM risk_events
      WHERE event_type = $1
        AND payload->>'breaker_id' = $2
      ORDER BY created_at DESC
      LIMIT 1`,
    [eventTypeForBreaker(breakerId), breakerId],
  );
  const s = (r.rows as Array<{ state?: string } | undefined>)[0]?.["state"];
  return s === "KILLED" || s === "PAUSED" ? s : null;
}

async function persistBreakerTrips(
  deps: {
    pool: pg.Pool | null;
    logger: { warn: (obj: object, msg?: string) => void };
  },
  breakers: CircuitBreaker[],
  chainId: number,
): Promise<void> {
  if (!deps.pool) return;
  for (const b of breakers) {
    try {
      if (!TRIPPED_STATES.includes(b.state)) {
        tripEpisodeState.delete(b.id);
        continue;
      }
      const prev = tripEpisodeState.get(b.id) ?? (await latestPersistedTripState(deps.pool, b.id));
      if (prev === b.state) {
        tripEpisodeState.set(b.id, b.state);
        continue; // same episode — already persisted, do not insert again
      }
      // New trip episode: count EXACTLY once. The episode map is set BEFORE the
      // INSERT await (REVIEW-FIX, adversarial): setting it only on insert
      // success meant a persistently failing INSERT re-classified the same
      // physical episode as new on every 60s tick, inflating
      // arbx_risk_cb_trips_total +1/min (~1440/day) for one trip — contradicting
      // the help text "trip episodes". Node is single-threaded, so the
      // synchronous set() also closes the concurrent status-poll/periodic-tick
      // double-count window. Known residual (accepted): if the INSERT fails the
      // durable risk_events row is lost for this process lifetime (warn below),
      // and a process restart re-counts a still-tripped breaker once.
      riskCbTripsTotal.labels(b.id, b.state).inc();
      tripEpisodeState.set(b.id, b.state);
      await deps.pool.query(
        `INSERT INTO risk_events (event_type, severity, source_service, payload, chain_id)
         VALUES ($1, $2, 'api-server', $3::jsonb, $4)`,
        [
          eventTypeForBreaker(b.id),
          b.state === "KILLED" ? "critical" : "warning",
          JSON.stringify({
            breaker_id: b.id,
            state: b.state,
            action: b.action,
            category: b.category,
            current_value: b.evidence.current_value,
            threshold: b.evidence.threshold,
            detail: b.evidence.detail.slice(0, 300),
            evidence_source: b.evidence.source,
            generated_at: b.last_evaluated_at,
          }),
          chainId,
        ],
      );
    } catch (e) {
      // Best-effort by contract: log and keep serving the status response.
      deps.logger.warn({ event: "circuit_breakers.trip_persist_failed", breaker: b.id, err: (e as Error).message });
    }
  }
}

/** Test-only: reset the in-memory trip episode tracker. */
function resetTripEpisodeState(): void {
  tripEpisodeState.clear();
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
  loadCbConfig,
  computeDrawdownStats,
  DD_MIN_RUNS,
  DD_MIN_SPAN_HOURS,
  persistBreakerTrips,
  resetTripEpisodeState,
  emitBreakerMetrics,
  CB_STATE_METRIC,
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
      emitBreakerMetrics(breakers, new Date(ctx.now));
      const summary = summarize(breakers);
      const overallState = overall(breakers);
      // Best-effort trip persistence — internal failures are logged, never surfaced as 5xx.
      await persistBreakerTrips(deps, breakers, ctx.chainId);
      const nextAction =
        overallState === "KILLED" ? "Operator disarm kill-switch with explicit reason."
        : overallState === "PAUSED" ? "Investigate worst-state breaker; review readiness items G-RPC-1/G-SIM-1."
        : overallState === "BLOCKED" ? "Resolve env prerequisites (RPC_HTTP_1, EXECUTOR_1) and run A.4 fork validation."
        : overallState === "NOT_AVAILABLE" ? "Configure ARBX_CB_* thresholds and accumulate paper_trade_runs history so DD/revert/gas evaluators have evidence."
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

  // Events endpoint — serves persisted breaker trips from risk_events (written
  // by persistBreakerTrips on transition to a tripped state). Honest empty +
  // blocked_reason when no pool / the query fails (fail-honest, never 5xx).
  app.get("/api/v1/risk/circuit-breakers/events", async (_req: Request, res: Response) => {
    const generatedAt = new Date().toISOString();
    if (!deps.pool) {
      const response: CircuitBreakerEventsResponse = {
        generated_at: generatedAt,
        event_source: "not_configured",
        events: [],
        blocked_reason: "No database pool — breaker trip persistence requires PostgreSQL (risk_events).",
        next_action: "Provide DATABASE_URL so circuit-breaker trips can be persisted and queried.",
      };
      res.status(200).json(response);
      return;
    }
    try {
      const q = await deps.pool.query(
        `SELECT id::text AS id, event_type, severity, source_service, payload, created_at
           FROM risk_events
          WHERE event_type IN ('circuit_breaker', 'kill_switch')
            AND payload->>'breaker_id' IS NOT NULL
          ORDER BY created_at DESC
          LIMIT 100`,
      );
      const events: RiskEventRow[] = (
        q.rows as Array<{ id: string; event_type: string; severity: string; source_service: string; payload: unknown; created_at: Date | string }>
      ).map((row) => ({
        id: row.id,
        event_type: row.event_type,
        severity: row.severity,
        source_service: row.source_service,
        payload: row.payload,
        created_at: new Date(row.created_at).toISOString(),
      }));
      const response: CircuitBreakerEventsResponse = {
        generated_at: generatedAt,
        event_source: "persistent_store",
        events,
        blocked_reason: null,
        next_action:
          "Review tripped breakers via /api/v1/risk/circuit-breakers/status; act on each breaker's required_action.",
      };
      res.status(200).json(response);
    } catch (e) {
      deps.logger.warn({ event: "circuit_breakers.events_failed", err: (e as Error).message });
      const response: CircuitBreakerEventsResponse = {
        generated_at: generatedAt,
        event_source: "not_configured",
        events: [],
        blocked_reason: `risk_events query failed: ${(e as Error).message.slice(0, 120)}`,
        next_action: "Check PostgreSQL health; trips keep being written best-effort on each status evaluation.",
      };
      res.status(200).json(response);
    }
  });

  // A.6 periodic emission: evaluate on a fixed cadence so the gauges exist
  // (and trips persist to risk_events) even when nobody polls the status
  // endpoint. First run fires immediately at boot so /metrics is populated
  // from the start; the timer is unref'd so it never keeps the process alive
  // on shutdown. Failures increment arbx_risk_cb_eval_failures_total and are
  // logged — they never crash the service (R8: an evaluation failure is
  // reported, never silently skipped or fabricated as a state).
  const emitTick = async (): Promise<void> => {
    try {
      const ctx = await collectCtx({ pool: deps.pool, killSwitch: deps.killSwitch });
      const breakers = buildAllBreakers(ctx);
      emitBreakerMetrics(breakers, new Date(ctx.now));
      await persistBreakerTrips(deps, breakers, ctx.chainId);
    } catch (e) {
      riskCbEvalFailuresTotal.labels("periodic").inc();
      deps.logger.warn({ event: "circuit_breakers.periodic_emit_failed", err: (e as Error).message });
    }
  };
  void emitTick();
  const emitTimer = setInterval(() => void emitTick(), CB_EMIT_INTERVAL_MS);
  emitTimer.unref?.();
}
