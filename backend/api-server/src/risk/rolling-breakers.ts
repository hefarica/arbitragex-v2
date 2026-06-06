/**
 * rolling-breakers — observability mirror of the canonical Rust breaker math.
 *
 * SINGLE SOURCE OF TRUTH is `backend/shared-rs/src/risk_ledger.rs`
 * (`compute_breakers`) — that Rust module is canonical for the *execution* gate
 * (pre_execute_checklist item 6). This TS port exists ONLY so the always-running
 * api-server can surface the same drawdown / gas-burn verdict on the risk
 * dashboard from `paper_trade_runs`, without a cross-language Redis-snapshot
 * producer for a 3-field aggregation. Keep the two in lockstep — the unit test
 * `rolling-breakers.test.ts` pins the shared semantics (tier boundaries,
 * insufficient-evidence floor, peak-to-trough drawdown).
 *
 * Doctrine: read-only over already-recorded shadow data. No capital, no I/O here
 * (pure functions). All thresholds are operator-supplied (no-hardcode); when they
 * are absent the breaker stays NOT_AVAILABLE — we never invent a NAV or a tier.
 */

/** One trade outcome in the window. `pnlUsd` from sim_expected_profit_usd,
 *  `gasUsd` from sim_gas_cost_usd. paper rows carry no revert signal. */
export interface TradeOutcome {
  tsUnix: number;
  pnlUsd: number;
  gasUsd: number;
}

/** Operator-supplied thresholds. Mirrors `risk_ledger::BreakerThresholds`. */
export interface BreakerThresholds {
  windowSecs: number;
  minSamples: number;
  navBaselineUsd: number; // drawdown is measured against this; must be > 0
  ddWarnPct: number;
  ddPausePct: number;
  ddHardPausePct: number;
  ddKillPct: number;
  maxGasBurnUsd: number;
}

/** Escalating level — mirrors `risk_ledger::BreakerLevel`. */
export type BreakerLevel = "ok" | "warn" | "pause" | "hard_pause" | "kill";

export interface BreakerMetric {
  value: number;
  level: BreakerLevel;
  samples: number;
  sufficient: boolean;
}

export interface BreakerWindow {
  windowSamples: number;
  equityUsd: number;
  drawdownPct: BreakerMetric;
  gasBurnUsd: BreakerMetric;
  insufficientEvidence: boolean;
}

function ddLevel(pct: number, t: BreakerThresholds): BreakerLevel {
  if (pct >= t.ddKillPct) return "kill";
  if (pct >= t.ddHardPausePct) return "hard_pause";
  if (pct >= t.ddPausePct) return "pause";
  if (pct >= t.ddWarnPct) return "warn";
  return "ok";
}

/**
 * Pure rolling-window evaluation. Mirrors `risk_ledger::compute_breakers`:
 * filters to `[now - windowSecs, now]`, walks the equity curve for peak-to-trough
 * drawdown (% of peak equity, equity = NAV + Σ pnl), sums gas. Below `minSamples`
 * → `sufficient=false`, `level="ok"` (insufficient evidence NEVER trips).
 */
export function computeBreakerWindow(
  nowUnix: number,
  outcomes: readonly TradeOutcome[],
  t: BreakerThresholds,
): BreakerWindow {
  const windowStart = nowUnix - Math.max(0, t.windowSecs);
  const win = outcomes
    .filter((o) => o.tsUnix >= windowStart && o.tsUnix <= nowUnix)
    .sort((a, b) => a.tsUnix - b.tsUnix);

  const n = win.length;
  const sufficient = n >= t.minSamples;
  const navOk = t.navBaselineUsd > 0;

  let equity = t.navBaselineUsd;
  let peak = t.navBaselineUsd;
  let maxDrawdownPct = 0;
  let gasSum = 0;

  for (const o of win) {
    equity += o.pnlUsd;
    gasSum += Math.max(0, o.gasUsd);
    if (equity > peak) peak = equity;
    if (navOk && peak > 0) {
      const dd = ((peak - equity) / peak) * 100;
      if (dd > maxDrawdownPct) maxDrawdownPct = dd;
    }
  }

  const drawdownLevel: BreakerLevel = sufficient ? ddLevel(maxDrawdownPct, t) : "ok";
  const gasLevel: BreakerLevel = sufficient && gasSum >= t.maxGasBurnUsd ? "pause" : "ok";

  return {
    windowSamples: n,
    equityUsd: equity,
    drawdownPct: { value: maxDrawdownPct, level: drawdownLevel, samples: n, sufficient },
    gasBurnUsd: { value: gasSum, level: gasLevel, samples: n, sufficient },
    insufficientEvidence: !sufficient,
  };
}

/**
 * Load operator thresholds from env. Returns null when ANY required threshold is
 * absent — we never fabricate a NAV / tier (no-hardcode). Caller reports
 * NOT_AVAILABLE("thresholds not configured") in that case.
 *
 *   ARBX_RISK_NAV_USD            drawdown baseline (capital base)
 *   ARBX_RISK_WINDOW_SECS        rolling window width
 *   ARBX_RISK_MIN_SAMPLES        evidence floor
 *   ARBX_RISK_DD_TIERS           "warn,pause,hard,kill" percents (e.g. "10,20,30,40")
 *   ARBX_RISK_GAS_CAP_USD        per-window gas-burn cap
 */
export function loadThresholdsFromEnv(
  env: NodeJS.ProcessEnv = process.env,
): BreakerThresholds | null {
  const nav = Number(env["ARBX_RISK_NAV_USD"]);
  const windowSecs = Number(env["ARBX_RISK_WINDOW_SECS"]);
  const minSamples = Number(env["ARBX_RISK_MIN_SAMPLES"]);
  const gasCap = Number(env["ARBX_RISK_GAS_CAP_USD"]);
  const tiers = (env["ARBX_RISK_DD_TIERS"] ?? "").split(",").map((s) => Number(s.trim()));

  if (
    !Number.isFinite(nav) || nav <= 0 ||
    !Number.isFinite(windowSecs) || windowSecs <= 0 ||
    !Number.isFinite(minSamples) || minSamples < 1 ||
    !Number.isFinite(gasCap) || gasCap <= 0 ||
    tiers.length !== 4 || tiers.some((x) => !Number.isFinite(x))
  ) {
    return null;
  }

  const tierVals = tiers as [number, number, number, number];
  return {
    windowSecs,
    minSamples,
    navBaselineUsd: nav,
    ddWarnPct: tierVals[0],
    ddPausePct: tierVals[1],
    ddHardPausePct: tierVals[2],
    ddKillPct: tierVals[3],
    maxGasBurnUsd: gasCap,
  };
}
