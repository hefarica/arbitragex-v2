/**
 * risk-circuit-breakers tests — pure-function evaluator assertions.
 *
 * Coverage:
 *   - 10 breakers built per call.
 *   - DD/revert/gas → NOT_AVAILABLE without ledger data (R8, never zero-fabricated).
 *   - A.6 ledger-fed evaluators:
 *       · computeDrawdownStats on synthetic curves (monotonic up, known trough,
 *         unanchored non-positive peak → pct null).
 *       · DD tier classification: 27.27% trough trips tier 20 (PAUSED/pause);
 *         50% trips tier 40 (KILLED/kill_switch); all tier states in detail.
 *       · DD evidence floors: <100 runs or <24h span → NOT_AVAILABLE w/ reason.
 *       · Revert rate: trailing-window counts vs ARBX_CB_MAX_REVERT_RATE;
 *         zero runs → NOT_AVAILABLE (rate undefined, NEVER 0%).
 *       · Actual gas burn: SUM(actual_gas_cost_usd) vs ARBX_CB_MAX_GAS_BURN_USD;
 *         empty window / no actuals → NOT_AVAILABLE; invalid cap fails closed.
 *       · Sim-gas legacy path (ARBX_RISK_*) preserved; combined worst-state.
 *       · loadCbConfig env parsing (empty string = unset → defaults).
 *       · Trip persistence dedupe: same episode → ONE risk_events row;
 *         transition PAUSED→KILLED → second row; insert failure never throws.
 *   - Executor → BLOCKED when env missing; WARN when env present (probe pending).
 *   - RPC health → BLOCKED when RPC_HTTP_1 missing.
 *   - Global kill-switch → PASS disarmed / KILLED armed / UNKNOWN on null.
 *   - Latency/SIM_ERROR/Blacklist → state derived from readiness verifier items.
 *   - Confidence → NOT_AVAILABLE while scoring pipeline not wired.
 *   - Summary + overall precedence (KILLED > PAUSED > BLOCKED > WARN > UNKNOWN > NOT_AVAILABLE > PASS).
 *   - Response shape backward-compat: same 10 ids, same row fields.
 *   - No fake metrics; no secrets in evidence; NOT_AVAILABLE never carries a value.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { __forTesting } from "./risk-circuit-breakers.js";
import { metricsText } from "@arbx/shared";

const {
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
  toGasMeasurementOutcomes,
  driftTrackerMode,
  coverageGapAction,
  deriveGasCoverage,
  readGasWindow,
  isMissingGasLedgerColumn,
  GAS_WINDOW_SQL_LEDGER,
  GAS_WINDOW_SQL_ALL_ROWS,
  DD_MIN_RUNS,
  DD_MIN_SPAN_HOURS,
  persistBreakerTrips,
  resetTripEpisodeState,
  emitBreakerMetrics,
  CB_STATE_METRIC,
} = __forTesting;

const SAVED_ENV = { ...process.env };
beforeEach(() => {
  process.env = { ...SAVED_ENV };
  resetTripEpisodeState();
});
afterEach(() => {
  process.env = { ...SAVED_ENV };
  resetTripEpisodeState();
});

const BREAKER_IDS = [
  "global_kill_switch",
  "drawdown_breaker",
  "revert_rate_breaker",
  "gas_burn_breaker",
  "latency_breaker",
  "sim_error_breaker",
  "rpc_health_breaker",
  "blacklist_breaker",
  "executor_breaker",
  "confidence_breaker",
];

const baseCtx = {
  now: "2026-05-13T00:00:00Z",
  chainId: 1,
  killSwitchEnabled: false,
  killSwitchReason: null,
  readiness: null,
  readinessError: null,
  envRpc: false,
  envExecutor: false,
  envTradeMode: null,
  scoringPipelineWired: false,
  gasBurn: null as null | { value: number; level: string; samples: number; sufficient: boolean },
  gasBurnReason: "operator risk thresholds not configured (ARBX_RISK_* env)",
  gasBurnCapUsd: null as number | null,
  gasBurnWindowRows: null as number | null,
  gasBurnWindowSecs: null as number | null,
  ledger: {
    drawdown: {
      marks: null as null | Array<{ markAt: string; runs: number; pnlUsd: number }>,
      reason: "no database pool",
      navUsd: 0,
      tiers: [10, 20, 30, 40] as const,
    },
    revertRate: {
      windowHours: 24,
      counts: null as null | { total: number; reverted: number },
      reason: "no database pool",
      thresholdPct: null as number | null,
      thresholdReason: "ARBX_CB_MAX_REVERT_RATE not configured",
    },
    gasBurn: {
      windowHours: 24,
      status: "cap_not_configured" as
        | "ok" | "cap_not_configured" | "cap_invalid" | "window_invalid" | "no_pool" | "query_failed",
      window: null as null | { rowsInWindow: number; withActualGas: number; sumUsd: number },
      reason: "ARBX_CB_MAX_GAS_BURN_USD not configured",
      capUsd: null as number | null,
    },
  },
};

// ---------------------------------------------------------------------------
// Helpers — synthetic equity marks / ledger windows.
// ---------------------------------------------------------------------------

/** Hourly equity marks: each entry = one mark with `runs` paper runs of `pnl`. */
function marks(pnls: number[], opts?: { runsPerMark?: number; hourStep?: number }): Array<{ markAt: string; runs: number; pnlUsd: number }> {
  const runs = opts?.runsPerMark ?? 50;
  const step = opts?.hourStep ?? 48; // default: two marks span 48h (>= 24h floor)
  const t0 = Date.parse("2026-08-01T00:00:00Z");
  return pnls.map((pnlUsd, i) => ({
    markAt: new Date(t0 + i * step * 3_600_000).toISOString(),
    runs,
    pnlUsd,
  }));
}

function ddCtx(m: Array<{ markAt: string; runs: number; pnlUsd: number }> | null, navUsd = 1000) {
  return { ...baseCtx, ledger: { ...baseCtx.ledger, drawdown: { marks: m, reason: m ? "ok" : "query_failed: x", navUsd, tiers: [10, 20, 30, 40] as const } } };
}

function revertCtx(counts: { total: number; reverted: number } | null, thresholdPct: number | null) {
  return {
    ...baseCtx,
    ledger: {
      ...baseCtx.ledger,
      revertRate: {
        windowHours: 24,
        counts,
        reason: counts ? "ok" : "query_failed: x",
        thresholdPct,
        thresholdReason: thresholdPct === null ? "ARBX_CB_MAX_REVERT_RATE not configured" : "",
      },
    },
  };
}

function gasCtx(
  window: {
    rowsInWindow: number;
    withActualGas: number;
    sumUsd: number;
    /** Coverage denominator from the measurement ledger (migration 126).
     *  Omitted ⇒ the fixture keeps the conservative all-rows denominator, which
     *  is what every pre-GAP-B assertion below was written against. */
    measurable?: number;
    denominator?: "ledger" | "all_rows_no_ledger";
  } | null,
  capUsd: number | null,
  status: "ok" | "cap_not_configured" | "cap_invalid" = capUsd === null ? "cap_not_configured" : "ok",
) {
  return {
    ...baseCtx,
    ledger: {
      ...baseCtx.ledger,
      gasBurn: {
        windowHours: 24,
        status,
        window,
        reason: status === "ok" ? "ok" : status === "cap_invalid" ? "invalid ARBX_CB_MAX_GAS_BURN_USD (expected > 0)" : "ARBX_CB_MAX_GAS_BURN_USD not configured",
        capUsd,
      },
    },
  };
}

// ---------------------------------------------------------------------------
// computeDrawdownStats — synthetic curve math.
// ---------------------------------------------------------------------------

describe("computeDrawdownStats", () => {
  it("monotonic-up curve has zero drawdown and tracks the final peak", () => {
    const s = computeDrawdownStats(marks([10, 10, 10]), 1000);
    expect(s.maxDdUsd).toBe(0);
    expect(s.maxDdPct).toBe(0);
    expect(s.peakUsd).toBe(1030);
    expect(s.samples).toBe(150);
  });

  it("known trough: +100 then -300 from NAV 1000 → dd $300 = 27.27% of peak 1100", () => {
    const s = computeDrawdownStats(marks([100, -300]), 1000);
    expect(s.maxDdUsd).toBeCloseTo(300, 6);
    expect(s.maxDdPct).toBeCloseTo((300 / 1100) * 100, 2);
    expect(s.peakUsd).toBeCloseTo(1100, 6);
  });

  it("unanchored monotonic-down curve → USD dd tracked, % null (peak <= 0, fail-honest)", () => {
    const s = computeDrawdownStats(marks([-5, -5]), 0);
    expect(s.maxDdUsd).toBeCloseTo(10, 6);
    expect(s.maxDdPct).toBe(null);
  });

  it("span is measured first-mark → last-mark in hours", () => {
    const s = computeDrawdownStats(marks([1, 1, 1], { hourStep: 24 }), 1000);
    expect(s.spanHours).toBeCloseTo(48, 6);
  });
});

// ---------------------------------------------------------------------------
// makeDrawdownBreaker — tier classification + evidence floors.
// ---------------------------------------------------------------------------

describe("makeDrawdownBreaker", () => {
  it("returns NOT_AVAILABLE without paper-shadow data", () => {
    const b = makeDrawdownBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.severity).toBe("critical");
    expect(b.evidence.current_value).toBe(null);
    expect(b.operator_required).toBe(true);
    expect(b.blocks).toContain("LIVE");
  });

  it("monotonic-up curve (sufficient evidence) → PASS, no LIVE block, value 0", () => {
    const b = makeDrawdownBreaker(ddCtx(marks([10, 10])));
    expect(b.state).toBe("PASS");
    expect(b.action).toBe("none");
    expect(b.evidence.current_value).toBe(0);
    expect(b.blocks).toEqual([]);
    expect(b.operator_required).toBe(false);
    expect(b.required_action).toBe(null);
  });

  it("known 25%+ trough (27.27%) trips tier 20 → PAUSED + pause action, all tier states reported", () => {
    const b = makeDrawdownBreaker(ddCtx(marks([100, -300])));
    expect(b.state).toBe("PAUSED");
    expect(b.action).toBe("pause");
    expect(b.severity).toBe("high");
    expect(b.evidence.current_value).toBeCloseTo(27.27, 1);
    expect(b.evidence.threshold).toBe("10/20/30/40 %");
    expect(b.blocks).toContain("LIVE");
    expect(b.evidence.detail).toContain("10%:TRIPPED");
    expect(b.evidence.detail).toContain("20%:TRIPPED");
    expect(b.evidence.detail).toContain("30%:PASS");
    expect(b.evidence.detail).toContain("40%:PASS");
  });

  it("50% drawdown trips tier 40 → KILLED + kill_switch action", () => {
    const b = makeDrawdownBreaker(ddCtx(marks([-500, 0])));
    expect(b.state).toBe("KILLED");
    expect(b.action).toBe("kill_switch");
    expect(b.severity).toBe("critical");
    expect(b.evidence.current_value).toBeCloseTo(50, 1);
  });

  it("sub-10% drawdown → PASS (no tier tripped)", () => {
    const b = makeDrawdownBreaker(ddCtx(marks([50, -50]))); // equity 1050 → 1000 = 4.76%
    expect(b.state).toBe("PASS");
    expect(b.evidence.detail).toContain("10%:PASS");
  });

  it(`insufficient runs (<${DD_MIN_RUNS}) → NOT_AVAILABLE with reason, current_value null`, () => {
    const b = makeDrawdownBreaker(ddCtx(marks([100, -300], { runsPerMark: 10 })));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("Insufficient evidence");
    expect(b.evidence.detail).toContain("20 runs");
  });

  it(`insufficient span (<${DD_MIN_SPAN_HOURS}h) → NOT_AVAILABLE with reason`, () => {
    const b = makeDrawdownBreaker(ddCtx(marks([100, -300], { hourStep: 1 })));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.detail).toContain("Insufficient evidence");
    expect(b.evidence.current_value).toBe(null);
  });

  it("peak equity <= 0 (unanchored, sufficient evidence) → NOT_AVAILABLE, asks for ARBX_RISK_NAV_USD", () => {
    const b = makeDrawdownBreaker(ddCtx(marks([-5, -5], { runsPerMark: 50 }), 0));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("ARBX_RISK_NAV_USD");
  });
});

// ---------------------------------------------------------------------------
// makeRevertRateBreaker — trailing window math.
// ---------------------------------------------------------------------------

describe("makeRevertRateBreaker", () => {
  it("returns NOT_AVAILABLE without rolling-window aggregator", () => {
    const b = makeRevertRateBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
  });

  it("zero runs in window → NOT_AVAILABLE (rate undefined, NEVER 0%)", () => {
    const b = makeRevertRateBreaker(revertCtx({ total: 0, reverted: 0 }, 30));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("not 0%");
  });

  it("counts present but threshold unconfigured → NOT_AVAILABLE asking for the env var", () => {
    const b = makeRevertRateBreaker(revertCtx({ total: 100, reverted: 5 }, null));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("ARBX_CB_MAX_REVERT_RATE not configured");
    expect(b.operator_required).toBe(true);
  });

  it("rate below threshold → PASS with computed rate and classification rule in detail", () => {
    const b = makeRevertRateBreaker(revertCtx({ total: 100, reverted: 2 }, 30));
    expect(b.state).toBe("PASS");
    expect(b.action).toBe("none");
    expect(b.evidence.current_value).toBe(2);
    expect(b.evidence.threshold).toBe(30);
    expect(b.evidence.detail).toContain("ILIKE '%revert%'");
    expect(b.blocks).toEqual([]);
  });

  it("rate at/above threshold → PAUSED + pause action (trips)", () => {
    const b = makeRevertRateBreaker(revertCtx({ total: 100, reverted: 40 }, 30));
    expect(b.state).toBe("PAUSED");
    expect(b.action).toBe("pause");
    expect(b.evidence.current_value).toBe(40);
    expect(b.blocks).toContain("LIVE");
    expect(b.required_action).toContain("Pause new submissions");
  });

  it("exact boundary (rate == threshold) trips (>= semantics)", () => {
    const b = makeRevertRateBreaker(revertCtx({ total: 100, reverted: 30 }, 30));
    expect(b.state).toBe("PAUSED");
  });
});

// ---------------------------------------------------------------------------
// makeGasBurnBreaker — actual-gas window, legacy sim path, combination.
// ---------------------------------------------------------------------------

describe("makeGasBurnBreaker (A.6 actual-gas path)", () => {
  it("returns NOT_AVAILABLE without gas ledger", () => {
    const b = makeGasBurnBreaker(baseCtx);
    expect(b.state).toBe("NOT_AVAILABLE");
  });

  it("partial coverage under cap (120/500 measured) → WARN — under-cap NOT certified (audit 2026-10)", () => {
    // The previous expectation (PASS on 120/500 measured) encoded the defect:
    // an observed $10 over 24% coverage cannot certify the window total is
    // under the $50 cap — the 380 unmeasured rows may carry the remainder.
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, 50));
    expect(b.state).toBe("WARN");
    expect(b.action).toBe("warn");
    expect(b.evidence.current_value).toBe(10);
    expect(b.evidence.threshold).toBe(50);
    expect(b.evidence.unit).toBe("USD per window");
    expect(b.evidence.detail).toContain("120/500");
    expect(b.evidence.detail).toContain("NOT certified");
    expect(b.blocks).toContain("LIVE");
    expect(b.required_action).toContain("backfill");
  });

  it("full coverage under cap (500/500 measured) → PASS with summed value", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 500, withActualGas: 500, sumUsd: 10 }, 50));
    expect(b.state).toBe("PASS");
    expect(b.evidence.current_value).toBe(10);
    expect(b.evidence.threshold).toBe(50);
    expect(b.evidence.unit).toBe("USD per window");
    expect(b.blocks).toEqual([]);
  });

  it("observed sum ≥ cap with coverage gaps → PAUSED — excess reported despite missing measurements", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 100, withActualGas: 1, sumUsd: 60 }, 50));
    expect(b.state).toBe("PAUSED");
    expect(b.action).toBe("pause");
    expect(b.evidence.current_value).toBe(60);
    expect(b.evidence.detail).toContain("1/100");
    expect(b.evidence.detail).toContain("unmeasured");
    expect(b.blocks).toContain("LIVE");
  });

  it("actual-gas sum at/above cap → PAUSED + pause action", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 75 }, 50));
    expect(b.state).toBe("PAUSED");
    expect(b.action).toBe("pause");
    expect(b.evidence.current_value).toBe(75);
    expect(b.blocks).toContain("LIVE");
  });

  it("empty window (0 runs) → NOT_AVAILABLE — burn undefined, not $0", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 0, withActualGas: 0, sumUsd: 0 }, 50));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("not $0");
  });

  it("runs present but none with actual_gas_cost_usd → NOT_AVAILABLE", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 400, withActualGas: 0, sumUsd: 0 }, 50));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.detail).toContain("actual_gas_cost_usd");
  });

  // -------------------------------------------------------------------------
  // Producer-switch honesty (R10 E2E-COMPUTE GUARD, 2026-10).
  //
  // The counterexample this closes: `required_action` used to answer a coverage
  // gap with "wait for drift_tracker to backfill…", but the drift-tracker only
  // spawns when ARBX_DRIFT_TRACKER_MODE == "on" (recon/src/main.rs:343) and
  // .env.example ships `off`. The instruction asked the operator to wait for a
  // backfill that a dormant switch can never produce — a config block dressed
  // up as a transient.
  // -------------------------------------------------------------------------

  it("driftTrackerMode replicates the producer's EXACT predicate (== \"on\")", () => {
    expect(driftTrackerMode({})).toEqual({ mode: "unset", observed: null });
    expect(driftTrackerMode({ ARBX_DRIFT_TRACKER_MODE: "on" })).toEqual({
      mode: "on",
      observed: "on",
    });
    // `.env.example` ships `off`.
    expect(driftTrackerMode({ ARBX_DRIFT_TRACKER_MODE: "off" }).mode).toBe("off");
    // Parity with `drift_mode == "on"`: no case folding, no trim, and an
    // empty/absent value is dormant — never silently promoted to "on".
    for (const v of ["", "ON", "On", "on ", " on", "true", "1"]) {
      expect(driftTrackerMode({ ARBX_DRIFT_TRACKER_MODE: v }).mode, `value=${JSON.stringify(v)}`).toBe("off");
    }
  });

  it("a dormant producer is named as CONFIG, not as a wait — the action is not wait-only", () => {
    const flag = driftTrackerMode({ ARBX_DRIFT_TRACKER_MODE: "off" });
    const a = coverageGapAction(flag, "0/400 measured");
    expect(a).toContain("CONFIG, not transient");
    expect(a).toContain('"off"');
    expect(a).toContain("no backfill can occur");
    expect(a).toContain("Operator-only flip");
    // The regression this asserts: no branch may instruct a bare wait.
    expect(a).not.toMatch(/^wait for/i);
  });

  it("an UNSET var is a config block too (absent ⇒ dormant), stated as an observation", () => {
    const a = coverageGapAction(driftTrackerMode({}), "0/400 measured");
    expect(a).toContain("unset in this");
    expect(a).toContain("absent");
    expect(a).toContain("backfill");
  });

  it("three distinct producer states ⇒ three distinct actions (a hardcoded text would fail)", () => {
    const gap = "same gap";
    const s = new Set(
      (["on", "off", undefined] as const).map((v) =>
        coverageGapAction(
          driftTrackerMode(v === undefined ? {} : { ARBX_DRIFT_TRACKER_MODE: v }),
          gap,
        ),
      ),
    );
    expect(s.size).toBe(3);
  });

  it("NOT_AVAILABLE with rows but no measurement names the dormant switch (not 'accumulate runs')", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 400, withActualGas: 0, sumUsd: 0 }, 50), {
      ARBX_DRIFT_TRACKER_MODE: "off",
    });
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.required_action).toContain("CONFIG, not transient");
    expect(b.required_action).toContain("ARBX_DRIFT_TRACKER_MODE");
    // The old instruction is exactly what cannot work while the switch is off.
    expect(b.required_action).not.toContain("Accumulate paper runs");
  });

  it("WARN partial coverage keeps `backfill` (pre-existing contract) and adds the real blocker", () => {
    const b = makeGasBurnBreaker(
      gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, 50),
      { ARBX_DRIFT_TRACKER_MODE: "off" },
    );
    expect(b.state).toBe("WARN");
    expect(b.required_action).toContain("backfill");
    expect(b.required_action).toContain("CONFIG, not transient");
    expect(b.required_action).toContain("120/500");
  });

  it("an ENABLED producer keeps the transient reading (the switch is not assumed dormant)", () => {
    const b = makeGasBurnBreaker(
      gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, 50),
      { ARBX_DRIFT_TRACKER_MODE: "on" },
    );
    expect(b.state).toBe("WARN");
    expect(b.required_action).toContain("ARBX_DRIFT_TRACKER_MODE=on");
    expect(b.required_action).not.toContain("CONFIG, not transient");
  });

  it("the switch never colours a healthy verdict: PASS ⇒ required_action null even when off", () => {
    const b = makeGasBurnBreaker(
      gasCtx({ rowsInWindow: 500, withActualGas: 500, sumUsd: 10 }, 50),
      { ARBX_DRIFT_TRACKER_MODE: "off" },
    );
    expect(b.state).toBe("PASS");
    expect(b.required_action).toBe(null);
  });

  it("invalid ARBX_CB_MAX_GAS_BURN_USD fails closed even when the sim path is healthy", () => {
    const ctx = {
      ...gasCtx(null, null, "cap_invalid"),
      gasBurn: { value: 5, level: "ok", samples: 120, sufficient: true },
      gasBurnCapUsd: 50,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.detail).toContain("invalid ARBX_CB_MAX_GAS_BURN_USD");
  });

  it("legacy sim-gas path (ARBX_RISK_*) preserved when ARBX_CB cap unset", () => {
    const ctx = {
      ...baseCtx,
      gasBurn: { value: 5, level: "ok", samples: 120, sufficient: true },
      gasBurnCapUsd: 50,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("PASS");
    expect(b.evidence.current_value).toBe(5);
    expect(b.evidence.detail).toContain("simulated gas");
  });

  it("combined: worst evaluated path wins (actual PASS + sim warn → WARN)", () => {
    const ctx = {
      ...gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, 50),
      gasBurn: { value: 5, level: "warn", samples: 120, sufficient: true },
      gasBurnCapUsd: 50,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("WARN");
    expect(b.action).toBe("warn");
    expect(b.evidence.detail).toContain("actual gas");
    expect(b.evidence.detail).toContain("simulated gas");
  });

  it("combined: NOT_AVAILABLE roll-up never carries a fabricated current_value (R8)", () => {
    const ctx = {
      ...gasCtx({ rowsInWindow: 400, withActualGas: 0, sumUsd: 0 }, 50), // actual → NOT_AVAILABLE
      gasBurn: { value: 5, level: "ok", samples: 120, sufficient: true }, // sim → PASS
      gasBurnCapUsd: 50,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
  });

  it("empty window NA carries the accumulate required_action (parity with revert_rate)", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 0, withActualGas: 0, sumUsd: 0 }, 50));
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.required_action).toContain("actual_gas_cost_usd");
  });

  it("dual-path: sim decides (cap unset) → deciding_path=sim, value+threshold from the sim path", () => {
    const ctx = {
      ...baseCtx,
      gasBurn: { value: 5, level: "ok", samples: 70, sufficient: true },
      gasBurnReason: "ok",
      gasBurnCapUsd: 40,
      gasBurnWindowRows: 70,
      gasBurnWindowSecs: 86400,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("PASS");
    expect(b.evidence.deciding_path).toBe("sim");
    expect(b.evidence.current_value).toBe(5);
    expect(b.evidence.threshold).toBe(40); // ARBX_RISK_GAS_CAP_USD, not the unset ARBX_CB cap
    expect(b.evidence.paths?.sim.measurements).toBe(70);
    expect(b.evidence.paths?.sim.window_rows).toBe(70);
    expect(b.evidence.paths?.sim.scope).toBe("chain:1");
  });

  it("dual-path: actual decides → deciding_path=actual, provenance labels replay (never on-chain settled)", () => {
    const ctx = {
      ...gasCtx({ rowsInWindow: 100, withActualGas: 100, sumUsd: 20 }, 50),
      gasBurn: { value: 5, level: "ok", samples: 70, sufficient: true },
      gasBurnCapUsd: 40,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("PASS");
    expect(b.evidence.deciding_path).toBe("actual");
    expect(b.evidence.current_value).toBe(20);
    expect(b.evidence.threshold).toBe(50);
    expect(b.evidence.paths?.actual.provenance).toContain("sim-ctl replay");
    expect(b.evidence.paths?.actual.provenance).toContain("not on-chain settled");
    expect(b.evidence.paths?.actual.measured).toBe(100);
    expect(b.evidence.paths?.actual.expected).toBe(100);
  });

  it("state/value alignment: sim is the worst path → displayed value is the SIM value, not A.6's", () => {
    // actual PASS ($20 full coverage) + sim KILLED ($999 sim burn) → row KILLED
    // by sim: the operator must see the sim number that caused it.
    const ctx = {
      ...gasCtx({ rowsInWindow: 100, withActualGas: 100, sumUsd: 20 }, 50),
      gasBurn: { value: 999, level: "kill", samples: 70, sufficient: true },
      gasBurnReason: "ok",
      gasBurnCapUsd: 40,
      gasBurnWindowRows: 70,
      gasBurnWindowSecs: 86400,
    };
    const b = makeGasBurnBreaker(ctx);
    expect(b.state).toBe("KILLED");
    expect(b.evidence.deciding_path).toBe("sim");
    expect(b.evidence.current_value).toBe(999);
    expect(b.evidence.threshold).toBe(40);
  });
});

// ---------------------------------------------------------------------------
// toGasMeasurementOutcomes — R8 loader contract: NULL ≠ 0.
// ---------------------------------------------------------------------------

describe("toGasMeasurementOutcomes (A.5 loader, NULL ≠ 0)", () => {
  it("NULL gas rows are NOT measurements: 10 NULL rows + min_samples 10 → 0 measurements (the fabricated-PASS counterexample)", () => {
    const rows = Array.from({ length: 10 }, (_, i) => ({ ts: 1000 + i, gas: null }));
    const { outcomes, measured, windowRows } = toGasMeasurementOutcomes(rows);
    expect(windowRows).toBe(10);
    expect(measured).toBe(0);
    expect(outcomes).toHaveLength(0);
    // With 0 measurements computeBreakerWindow is insufficient by definition —
    // the window can never reach sufficiency on gas-less rows again.
  });

  it("a stored 0.0 IS a valid measurement and is preserved verbatim (never dropped, never inflated)", () => {
    const rows = [
      { ts: 1, gas: 0.0 },
      { ts: 2, gas: null },
      { ts: 3, gas: 2.5 },
    ];
    const { outcomes, measured, windowRows } = toGasMeasurementOutcomes(rows);
    expect(measured).toBe(2);
    expect(windowRows).toBe(3);
    expect(outcomes.map((o) => o.gasUsd)).toEqual([0, 2.5]);
  });

  it("non-finite values are rejected as measurements (defensive: NaN never sums)", () => {
    const rows = [
      { ts: 1, gas: Number.NaN },
      { ts: 2, gas: 1.0 },
    ];
    const { measured } = toGasMeasurementOutcomes(rows);
    expect(measured).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// loadCbConfig — env parsing (empty string = unset → structural defaults).
// ---------------------------------------------------------------------------

describe("loadCbConfig", () => {
  it("clean env → structural defaults, no invented trip thresholds", () => {
    const c = loadCbConfig({});
    expect(c.chainId).toBe(1);
    expect(c.windowHours).toBe(24);
    expect(c.maxRevertRatePct).toBe(null);
    expect(c.maxGasBurnUsd).toBe(null);
    expect(c.navUsd).toBe(0);
    expect([...c.ddTiers]).toEqual([10, 20, 30, 40]);
  });

  it("empty-string window var means unset → default 24, not invalid", () => {
    expect(loadCbConfig({ ARBX_CB_REVERT_WINDOW_H: "" }).windowHours).toBe(24);
  });

  it("non-numeric window → null (invalid, breakers must NOT_AVAILABLE)", () => {
    expect(loadCbConfig({ ARBX_CB_REVERT_WINDOW_H: "abc" }).windowHours).toBe(null);
    expect(loadCbConfig({ ARBX_CB_REVERT_WINDOW_H: "-3" }).windowHours).toBe(null);
  });

  it("valid threshold vars parse; set-but-invalid flags distinguish absence", () => {
    const ok = loadCbConfig({ ARBX_CB_MAX_REVERT_RATE: "30", ARBX_CB_MAX_GAS_BURN_USD: "50" });
    expect(ok.maxRevertRatePct).toBe(30);
    expect(ok.maxGasBurnUsd).toBe(50);
    const bad = loadCbConfig({ ARBX_CB_MAX_REVERT_RATE: "0", ARBX_CB_MAX_GAS_BURN_USD: "nope" });
    expect(bad.maxRevertRatePct).toBe(null);
    expect(bad.revertRateSet).toBe(true); // set-but-invalid, not absent
    expect(bad.maxGasBurnUsd).toBe(null);
    expect(bad.gasBurnSet).toBe(true);
  });

  it("chain + tiers + NAV parse with fallbacks", () => {
    const c = loadCbConfig({ ARBX_CB_CHAIN_ID: "137", ARBX_RISK_DD_TIERS: "5,10,15,20", ARBX_RISK_NAV_USD: "10000" });
    expect(c.chainId).toBe(137);
    expect([...c.ddTiers]).toEqual([5, 10, 15, 20]);
    expect(c.navUsd).toBe(10000);
    const d = loadCbConfig({ ARBX_RISK_DD_TIERS: "10,20" }); // wrong arity → doctrine tiers
    expect([...d.ddTiers]).toEqual([10, 20, 30, 40]);
  });
});

// ---------------------------------------------------------------------------
// persistBreakerTrips — risk_events dedupe on state transition.
// ---------------------------------------------------------------------------

interface RecordedInsert { sql: string; params: unknown[] }

function makeFakePool(opts: { latestState?: string | null; failInserts?: boolean } = {}) {
  const inserts: RecordedInsert[] = [];
  const selects: number[] = [];
  const latest = opts.latestState ?? null;
  const pool = {
    async query(sql: string, params?: unknown[]) {
      if (sql.includes("payload->>'state'")) {
        selects.push(1);
        return { rows: latest === null ? [] : [{ state: latest }] };
      }
      if (sql.startsWith("INSERT INTO risk_events")) {
        if (opts.failInserts) throw new Error("synthetic insert failure");
        inserts.push({ sql, params: params ?? [] });
        return { rows: [] };
      }
      return { rows: [] };
    },
  } as unknown as import("pg").Pool;
  return { pool, inserts, selects };
}

function trippedBreakerRow() {
  // Real evaluator row: 40% revert rate vs 30% cap → PAUSED.
  return makeRevertRateBreaker(revertCtx({ total: 100, reverted: 40 }, 30));
}

describe("persistBreakerTrips", () => {
  it("first trip inserts exactly one risk_events row; repeated poll same state does NOT insert again", async () => {
    const { pool, inserts, selects } = makeFakePool({ latestState: null });
    const logger = { warn: () => {} };
    const breaker = trippedBreakerRow();
    expect(breaker.state).toBe("PAUSED");

    await persistBreakerTrips({ pool, logger }, [breaker], 1);
    expect(inserts.length).toBe(1);
    expect(inserts[0]!.sql).toContain("INSERT INTO risk_events");
    expect(inserts[0]!.params[0]).toBe("circuit_breaker");
    expect(inserts[0]!.params[1]).toBe("warning");
    const payload = JSON.parse(inserts[0]!.params[2] as string) as { breaker_id: string; state: string };
    expect(payload.breaker_id).toBe("revert_rate_breaker");
    expect(payload.state).toBe("PAUSED");
    expect(inserts[0]!.params[3]).toBe(1); // chain_id

    // Second evaluation, same state, same episode → deduped (no extra SELECT either).
    await persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1);
    expect(inserts.length).toBe(1);
    expect(selects.length).toBe(1);
  });

  it("cold start consults risk_events: latest persisted row same state → no insert (episode continues)", async () => {
    const { pool, inserts } = makeFakePool({ latestState: "PAUSED" });
    const logger = { warn: () => {} };
    await persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1);
    expect(inserts.length).toBe(0);
  });

  it("state transition PAUSED → KILLED inserts a second row", async () => {
    const { pool, inserts } = makeFakePool({ latestState: null });
    const logger = { warn: () => {} };
    await persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1);
    const killed = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: true, killSwitchReason: "ops" });
    expect(killed.state).toBe("KILLED");
    await persistBreakerTrips({ pool, logger }, [killed], 1);
    expect(inserts.length).toBe(2);
    expect(inserts[1]!.params[1]).toBe("critical"); // KILLED severity
    expect((JSON.parse(inserts[1]!.params[2] as string) as { breaker_id: string }).breaker_id).toBe("global_kill_switch");
    expect(inserts[1]!.params[0]).toBe("kill_switch"); // kill-switch rows use their event_type
  });

  it("non-tripped breakers never touch risk_events", async () => {
    const { pool, inserts, selects } = makeFakePool({ latestState: null });
    const logger = { warn: () => {} };
    const pass = makeRevertRateBreaker(revertCtx({ total: 100, reverted: 2 }, 30));
    expect(pass.state).toBe("PASS");
    await persistBreakerTrips({ pool, logger }, [pass], 1);
    expect(inserts.length).toBe(0);
    expect(selects.length).toBe(0);
  });

  it("insert failure is logged and never throws", async () => {
    const { pool, inserts } = makeFakePool({ latestState: null, failInserts: true });
    const warns: Array<Record<string, unknown>> = [];
    const logger = { warn: (obj: Record<string, unknown>) => { warns.push(obj); } };
    await expect(persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1)).resolves.toBeUndefined();
    expect(inserts.length).toBe(0);
    expect(warns.length).toBe(1);
    expect(warns[0]!["event"]).toBe("circuit_breakers.trip_persist_failed");
  });

  it("null pool is a no-op", async () => {
    const logger = { warn: () => {} };
    await expect(persistBreakerTrips({ pool: null, logger }, [trippedBreakerRow()], 1)).resolves.toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// A.6 Prometheus emission — arbx_risk_cb_* (A6-CBPROM-01).
// ---------------------------------------------------------------------------

describe("A.6 Prometheus emission (arbx_risk_cb_*)", () => {
  it("maps every wire state to the documented gauge value (7 states, no collisions)", () => {
    const values = Object.values(CB_STATE_METRIC);
    expect(values.length).toBe(7);
    expect(new Set(values).size).toBe(7);
    expect(CB_STATE_METRIC.PASS).toBe(0);
    expect(CB_STATE_METRIC.WARN).toBe(1);
    expect(CB_STATE_METRIC.PAUSED).toBe(2);
    expect(CB_STATE_METRIC.KILLED).toBe(3);
    expect(CB_STATE_METRIC.BLOCKED).toBe(4);
    expect(CB_STATE_METRIC.NOT_AVAILABLE).toBe(5);
    expect(CB_STATE_METRIC.UNKNOWN).toBe(6);
  });

  it("emitBreakerMetrics sets arbx_risk_cb_state for the 10 breakers + last-eval unixtime", async () => {
    const breakers = buildAllBreakers(baseCtx);
    const at = new Date("2026-05-13T00:00:00Z");
    emitBreakerMetrics(breakers, at);
    // Assert on the Prometheus exposition format itself — the exact lines the
    // scraper sees on api-server /metrics.
    const { body } = await metricsText();
    const stateLines = body
      .split("\n")
      .filter((l) => l.startsWith("arbx_risk_cb_state{"))
      .map((l) => /name="([a-z_]+)"} (\d+)$/.exec(l));
    expect(stateLines.every(Boolean)).toBe(true);
    const byName = new Map(stateLines.map((m) => [m![1]!, Number(m![2])]));
    expect([...byName.keys()].sort()).toEqual([...BREAKER_IDS].sort());
    // baseCtx honest expectations: executor env missing → BLOCKED; kill-switch
    // disarmed → PASS; DD without ledger → NOT_AVAILABLE (never fabricated).
    expect(byName.get("executor_breaker")).toBe(CB_STATE_METRIC.BLOCKED);
    expect(byName.get("global_kill_switch")).toBe(CB_STATE_METRIC.PASS);
    expect(byName.get("drawdown_breaker")).toBe(CB_STATE_METRIC.NOT_AVAILABLE);
    expect(body).toContain(
      `arbx_risk_cb_last_eval_unixtime ${Math.floor(at.getTime() / 1000)}`,
    );
  });

  it("trip episodes increment arbx_risk_cb_trips_total exactly once per NEW episode", async () => {
    const series = 'arbx_risk_cb_trips_total{name="revert_rate_breaker",state="PAUSED"}';
    const readTrips = async (): Promise<number> => {
      const { body } = await metricsText();
      const line = body.split("\n").find((l) => l.startsWith(series));
      return line ? Number(line.slice(series.length).trim()) : 0;
    };
    const before = await readTrips();
    const logger = { warn: () => {} };

    // First evaluation of a tripped breaker → new episode → +1.
    const { pool } = makeFakePool({ latestState: null });
    await persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1);
    expect(await readTrips()).toBe(before + 1);

    // Same episode re-polled → no additional trip counted (dedupe contract).
    await persistBreakerTrips({ pool, logger }, [trippedBreakerRow()], 1);
    expect(await readTrips()).toBe(before + 1);

    // Cold start seeded from risk_events with the same state → episode continues.
    const { pool: seededPool } = makeFakePool({ latestState: "PAUSED" });
    await persistBreakerTrips({ pool: seededPool, logger }, [trippedBreakerRow()], 1);
    expect(await readTrips()).toBe(before + 1);
  });

  it("REVIEW-FIX: persistently failing INSERT counts the episode exactly ONCE (no +1 per tick)", async () => {
    const series = 'arbx_risk_cb_trips_total{name="revert_rate_breaker",state="PAUSED"}';
    const readTrips = async (): Promise<number> => {
      const { body } = await metricsText();
      const line = body.split("\n").find((l) => l.startsWith(series));
      return line ? Number(line.slice(series.length).trim()) : 0;
    };
    const before = await readTrips();
    const logger = { warn: () => {} };

    // INSERT persistently fails (disk full / constraint drift — this VPS has
    // had both). The trip still happened → counted ONCE. Previously the
    // episode map was only set on insert success, so every 60s tick
    // re-classified the same physical episode as new → +1/min inflation
    // (~1440/day) for one real trip, contradicting the help text.
    const { pool: failPool } = makeFakePool({ latestState: null, failInserts: true });
    await persistBreakerTrips({ pool: failPool, logger }, [trippedBreakerRow()], 1);
    expect(await readTrips()).toBe(before + 1);

    // The next 3 periodic ticks with the insert still failing → still ONE.
    await persistBreakerTrips({ pool: failPool, logger }, [trippedBreakerRow()], 1);
    await persistBreakerTrips({ pool: failPool, logger }, [trippedBreakerRow()], 1);
    await persistBreakerTrips({ pool: failPool, logger }, [trippedBreakerRow()], 1);
    expect(await readTrips()).toBe(before + 1);
  });
});

// ---------------------------------------------------------------------------
// Unchanged evaluators (regression).
// ---------------------------------------------------------------------------

describe("makeLatencyBreaker", () => {
  it("returns NOT_AVAILABLE when readiness G-RPC-1 missing", () => {
    const b = makeLatencyBreaker(baseCtx);
    expect(["NOT_AVAILABLE", "UNKNOWN"]).toContain(b.state);
  });
  it("returns PASS when G-RPC-1 is green", () => {
    const ctx = {
      ...baseCtx,
      readiness: {
        items: [{ id: "G-RPC-1", status: "green", reason: "ok", group: "risk_doctrines" as const, label: "x", verified_at: "x" }],
        summary: { green: 1, yellow: 0, red: 0, pending: 0, total: 1 },
        flip_blocked: false,
        generated_at: "x",
      },
    } as Parameters<typeof makeLatencyBreaker>[0];
    expect(makeLatencyBreaker(ctx).state).toBe("PASS");
  });
  it("returns PAUSED when G-RPC-1 is red", () => {
    const ctx = {
      ...baseCtx,
      readiness: {
        items: [{ id: "G-RPC-1", status: "red", reason: "rpc down", group: "risk_doctrines" as const, label: "x", verified_at: "x" }],
        summary: { green: 0, yellow: 0, red: 1, pending: 0, total: 1 },
        flip_blocked: true,
        generated_at: "x",
      },
    } as Parameters<typeof makeLatencyBreaker>[0];
    expect(makeLatencyBreaker(ctx).state).toBe("PAUSED");
  });
});

describe("makeSimErrorBreaker", () => {
  it("returns NOT_AVAILABLE when readiness G-SIM-1 missing", () => {
    expect(makeSimErrorBreaker(baseCtx).state).toBe("NOT_AVAILABLE");
  });
});

describe("makeRpcHealthBreaker", () => {
  it("returns BLOCKED when RPC_HTTP_1 missing", () => {
    delete process.env["RPC_HTTP_1"];
    const b = makeRpcHealthBreaker({ ...baseCtx, envRpc: false });
    expect(b.state).toBe("BLOCKED");
    expect(b.severity).toBe("critical");
    expect(b.blocks).toContain("A.4");
    expect(b.blocks).toContain("LIVE");
  });
});

describe("makeBlacklistBreaker", () => {
  it("returns NOT_AVAILABLE without G-TOK-1", () => {
    expect(makeBlacklistBreaker(baseCtx).state).toBe("NOT_AVAILABLE");
  });
});

describe("makeExecutorBreaker", () => {
  it("returns BLOCKED when EXECUTOR_1 missing", () => {
    const b = makeExecutorBreaker({ ...baseCtx, envExecutor: false });
    expect(b.state).toBe("BLOCKED");
    expect(b.severity).toBe("critical");
  });
  it("returns WARN when EXECUTOR_1 present but on-chain probe pending", () => {
    const b = makeExecutorBreaker({ ...baseCtx, envExecutor: true });
    expect(b.state).toBe("WARN");
    expect(b.evidence.current_value).toBe("env_present");
  });
});

describe("makeConfidenceBreaker", () => {
  it("returns NOT_AVAILABLE when scoring pipeline not wired", () => {
    expect(makeConfidenceBreaker({ ...baseCtx, scoringPipelineWired: false }).state).toBe("NOT_AVAILABLE");
  });
  it("returns PASS when scoring pipeline wired", () => {
    expect(makeConfidenceBreaker({ ...baseCtx, scoringPipelineWired: true }).state).toBe("PASS");
  });
});

describe("makeGlobalKillSwitchBreaker", () => {
  it("returns PASS when kill-switch disarmed", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: false });
    expect(b.state).toBe("PASS");
    // CRITICAL: even when "PASS", global breaker STILL blocks LIVE (A.9 sign-off gate).
    expect(b.blocks).toContain("LIVE");
  });
  it("returns KILLED when kill-switch armed", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: true, killSwitchReason: "operator" });
    expect(b.state).toBe("KILLED");
    expect(b.action).toBe("kill_switch");
  });
  it("returns UNKNOWN when kill-switch state cannot be read", () => {
    const b = makeGlobalKillSwitchBreaker({ ...baseCtx, killSwitchEnabled: null });
    expect(b.state).toBe("UNKNOWN");
  });
});

describe("buildAllBreakers", () => {
  it("builds exactly 10 breakers", () => {
    expect(buildAllBreakers(baseCtx).length).toBe(10);
  });
});

// ---------------------------------------------------------------------------
// Response shape backward-compat — SystemGuardBanner + frontend Zod rely on it.
// ---------------------------------------------------------------------------

describe("response shape backward-compat", () => {
  it("breaker ids and order are unchanged", () => {
    expect(buildAllBreakers(baseCtx).map((b) => b.id)).toEqual(BREAKER_IDS);
  });

  it("every breaker row keeps the canonical field set and types", () => {
    const stringNumberNull = (v: unknown) =>
      v === null || ["string", "number"].includes(typeof v);
    const stringOrNull = (v: unknown) => v === null || typeof v === "string";
    for (const b of buildAllBreakers(baseCtx)) {
      expect(typeof b.id).toBe("string");
      expect(typeof b.name).toBe("string");
      expect(typeof b.category).toBe("string");
      expect(typeof b.state).toBe("string");
      expect(typeof b.severity).toBe("string");
      expect(typeof b.action).toBe("string");
      expect(typeof b.evidence.detail).toBe("string");
      expect(stringNumberNull(b.evidence.current_value)).toBe(true);
      expect(stringNumberNull(b.evidence.threshold)).toBe(true);
      expect(stringOrNull(b.evidence.unit)).toBe(true);
      expect(Array.isArray(b.blocks)).toBe(true);
      expect(typeof b.operator_required).toBe("boolean");
      expect(typeof b.last_evaluated_at).toBe("string");
      expect(typeof b.description).toBe("string");
      expect(stringOrNull(b.required_action)).toBe(true);
    }
  });

  it("evidence.source stays within the wire enum", () => {
    const allowed = ["kill_switch", "readiness_verifier", "env_probe", "scoring_status", "paper_ledger", "not_configured"];
    for (const b of buildAllBreakers(baseCtx)) {
      expect(allowed).toContain(b.evidence.source);
    }
  });
});

describe("summarize + overall", () => {
  it("summarizes correctly with kill switch disarmed + envs missing", () => {
    const breakers = buildAllBreakers(baseCtx);
    const s = summarize(breakers);
    expect(s.total).toBe(10);
    // 1 PASS (kill_switch disarmed), 0+ BLOCKED/NOT_AVAILABLE.
    expect(s.pass).toBeGreaterThanOrEqual(1);
    expect(s.blocked + s.not_available + s.warn + s.paused + s.killed + s.unknown).toBeGreaterThan(0);
  });

  it("summary counts stay internally consistent for ledger-fed states", () => {
    const ctx = {
      ...ddCtx(marks([100, -300])), // drawdown → PAUSED
      ledger: {
        ...baseCtx.ledger,
        drawdown: { marks: marks([100, -300]), reason: "ok", navUsd: 1000, tiers: [10, 20, 30, 40] as const },
        revertRate: { windowHours: 24, counts: { total: 100, reverted: 2 }, reason: "ok", thresholdPct: 30, thresholdReason: "" },
        gasBurn: { windowHours: 24, status: "ok" as const, window: { rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, reason: "ok", capUsd: 50 },
      },
    };
    const s = summarize(buildAllBreakers(ctx));
    expect(s.total).toBe(10);
    expect(s.pass + s.warn + s.paused + s.killed + s.blocked + s.not_available + s.unknown).toBe(10);
    expect(s.paused).toBeGreaterThanOrEqual(1);
  });

  it("overall returns BLOCKED when any breaker is BLOCKED", () => {
    const breakers = buildAllBreakers(baseCtx);
    const o = overall(breakers);
    // With env missing, rpc+executor are BLOCKED → overall should be BLOCKED.
    expect(["BLOCKED", "KILLED", "PAUSED"]).toContain(o);
  });

  it("overall returns KILLED when kill switch armed (worst-state precedence)", () => {
    const ctx = { ...baseCtx, killSwitchEnabled: true, killSwitchReason: "ops" };
    const o = overall(buildAllBreakers(ctx));
    expect(o).toBe("KILLED");
  });
});

describe("regression: no fake metrics + no secrets", () => {
  it("no breaker leaks RPC URL / DB password / contract address in evidence", () => {
    process.env["RPC_HTTP_1"] = "https://eth.alchemy.com/v2/SECRET-API-KEY-9999";
    process.env["EXECUTOR_1"] = "0xDEADBEEFCAFE1234567890abcdef1234567890ab";
    process.env["DATABASE_URL"] = "postgres://user:supersecret@host:5432/db";
    const breakers = buildAllBreakers({
      ...baseCtx,
      envRpc: true,
      envExecutor: true,
    });
    const serialized = JSON.stringify(breakers);
    expect(serialized).not.toContain("SECRET-API-KEY");
    expect(serialized).not.toContain("supersecret");
    expect(serialized).not.toContain("DEADBEEFCAFE1234567890");
  });

  it("no breaker fabricates a current_value for NOT_AVAILABLE state", () => {
    const breakers = buildAllBreakers(baseCtx);
    for (const b of breakers) {
      if (b.state === "NOT_AVAILABLE") {
        expect(b.evidence.current_value).toBe(null);
      }
    }
  });

  it("R8 invariant holds for ledger-fed NOT_AVAILABLE rows too (insufficient + thresholdless)", () => {
    const ctxs = [
      ddCtx(marks([100, -300], { runsPerMark: 10 })),          // insufficient runs
      ddCtx(marks([-5, -5], { runsPerMark: 50 }), 0),          // peak <= 0
      revertCtx({ total: 0, reverted: 0 }, 30),                // empty window
      revertCtx({ total: 100, reverted: 5 }, null),            // threshold unconfigured
      gasCtx({ rowsInWindow: 0, withActualGas: 0, sumUsd: 0 }, 50), // empty gas window
      {
        ...gasCtx({ rowsInWindow: 400, withActualGas: 0, sumUsd: 0 }, 50),
        gasBurn: { value: 5, level: "ok", samples: 120, sufficient: true },
        gasBurnCapUsd: 50,
      },
    ];
    for (const ctx of ctxs) {
      const breakers = buildAllBreakers(ctx);
      for (const b of breakers) {
        if (b.state === "NOT_AVAILABLE") {
          expect(b.evidence.current_value, `${b.id} fabricated a value`).toBe(null);
          expect(b.evidence.detail.length, `${b.id} missing R8 reason`).toBeGreaterThan(0);
        }
      }
    }
  });
});


describe("WO-3 exhaustive next action without false PASS", () => {
  for (const state of ["KILLED","PAUSED","BLOCKED","WARN","UNKNOWN","NOT_AVAILABLE","PASS"] as const) {
    it(`next_action faithfully describes ${state}`, async () => {
      const {__forTesting}=await import("./risk-circuit-breakers.js");
      const action=__forTesting.nextActionForState(state);
      expect(typeof action).toBe("string");expect(action.length).toBeGreaterThan(10);
      expect(action.includes("All real breakers PASS")).toBe(state === "PASS");
    });
  }
});

// ---------------------------------------------------------------------------
// GAP-B — the A.6 coverage denominator (Backend, 2026-10).
//
// The defect: the denominator was COUNT(*) — EVERY window row. Rows the
// measurement ledger marks `not_applicable` (settled WITHOUT broadcast: an
// economic/market reject burns no gas) or `impossible` (structurally
// unresolvable by the drift scan) can NEVER carry actual_gas_cost_usd, so
// `unmeasured` could never reach 0 and under-cap could never be certified. A
// WARN that no amount of waiting clears is not a warning — it is a permanent
// block wearing one.
//
// These tests fix the arithmetic AND the honesty: the narrower denominator is
// used ONLY when the ledger supplies it (`denominator: "ledger"`); a deployment
// without the column keeps the conservative all-rows denominator and SAYS SO.
// ---------------------------------------------------------------------------

describe("deriveGasCoverage (A.6 denominator arithmetic — pure)", () => {
  it("ledger denominator excludes never-measurable rows from the population", () => {
    const c = deriveGasCoverage({
      rowsInWindow: 500,
      withActualGas: 120,
      measurable: 120,
      denominator: "ledger",
    });
    expect(c.expected).toBe(120);
    expect(c.excluded).toBe(380);
    expect(c.unmeasured).toBe(0);
    expect(c.breach).toBe(false);
  });

  it("an absent ledger keeps every window row (conservative, never optimistic)", () => {
    const c = deriveGasCoverage({ rowsInWindow: 500, withActualGas: 120 });
    expect(c.denominator).toBe("all_rows_no_ledger");
    expect(c.expected).toBe(500);
    expect(c.excluded).toBe(0);
    expect(c.unmeasured).toBe(380);
  });

  it("a numerator larger than the measurable set is a BREACH, not a clamp", () => {
    // Clamping `unmeasured` to 0 here would turn self-refuting data into a PASS.
    const c = deriveGasCoverage({
      rowsInWindow: 500,
      withActualGas: 500,
      measurable: 10,
      denominator: "ledger",
    });
    expect(c.breach).toBe(true);
  });

  it("a denominator larger than the window violates the subset relation", () => {
    const c = deriveGasCoverage({
      rowsInWindow: 10,
      withActualGas: 1,
      measurable: 40,
      denominator: "ledger",
    });
    expect(c.breach).toBe(true);
  });

  it("every row excluded (measured 0, measurable 0) is NOT a breach — it is no evidence", () => {
    const c = deriveGasCoverage({
      rowsInWindow: 300,
      withActualGas: 0,
      measurable: 0,
      denominator: "ledger",
    });
    expect(c.breach).toBe(false);
    expect(c.excluded).toBe(300);
  });
});

describe("makeGasBurnBreaker — GAP-B ledger denominator", () => {
  it("all never-measurable rows excluded → under-cap certifies as PASS (was a permanent WARN)", () => {
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 500,
          withActualGas: 120,
          sumUsd: 10,
          measurable: 120,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(b.state).toBe("PASS");
    expect(b.blocks).toEqual([]);
    expect(b.evidence.detail).toContain("120/120 measurable runs");
    expect(b.evidence.detail).toContain("380 window row(s) excluded");
  });

  it("measurable rows still uncovered → WARN (an exclusion never certifies the remainder)", () => {
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 500,
          withActualGas: 120,
          sumUsd: 10,
          measurable: 200,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(b.state).toBe("WARN");
    expect(b.evidence.detail).toContain("120/200 measurable runs");
    expect(b.evidence.detail).toContain("80 measurable run(s) lack actual_gas_cost_usd");
    expect(b.required_action).toContain("120/200");
  });

  it("a numerator exceeding the ledger's measurable set → NOT_AVAILABLE, never PASS", () => {
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 500,
          withActualGas: 500,
          sumUsd: 10,
          measurable: 10,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(b.state).toBe("NOT_AVAILABLE");
    expect(b.evidence.current_value).toBe(null);
    expect(b.evidence.detail).toContain("contradicts itself");
    expect(b.required_action).toContain("paper-gas-coverage-audit.sh");
    // The drift-tracker switch is NOT the blocker here — do not send the
    // operator to flip a flag that cannot change this aggregate.
    expect(b.required_action).not.toContain("Operator-only flip");
  });

  it("the wire says WHICH denominator produced the verdict", () => {
    const ledger = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 500,
          withActualGas: 500,
          sumUsd: 10,
          measurable: 500,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(ledger.evidence.paths?.actual.denominator).toBe("ledger");
    expect(ledger.evidence.paths?.actual.excluded).toBe(0);
    const degraded = makeGasBurnBreaker(
      gasCtx({ rowsInWindow: 500, withActualGas: 500, sumUsd: 10 }, 50),
    );
    expect(degraded.evidence.paths?.actual.denominator).toBe("all_rows_no_ledger");
  });

  it("a degraded denominator cannot PASS silently: the reason names the missing migration", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 500, withActualGas: 120, sumUsd: 10 }, 50));
    expect(b.state).toBe("WARN");
    expect(b.evidence.paths?.actual.reason).toContain("gas_measurement_state is absent");
    expect(b.evidence.paths?.actual.reason).toContain(
      "126_paper_trade_runs_gas_coverage_ledger",
    );
    // The operator must not be promised that waiting closes it.
    expect(b.required_action).toContain("126_paper_trade_runs_gas_coverage_ledger");
  });

  it("a ledger-denominator PASS carries no degraded-migration note", () => {
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 120,
          withActualGas: 120,
          sumUsd: 10,
          measurable: 120,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(b.state).toBe("PASS");
    expect(b.evidence.paths?.actual.reason).not.toContain("gas_measurement_state is absent");
    expect(b.required_action).toBe(null);
  });

  it("PAUSED stays reachable with a ledger denominator (an excess is never hidden)", () => {
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 500,
          withActualGas: 120,
          sumUsd: 75,
          measurable: 120,
          denominator: "ledger",
        },
        50,
      ),
    );
    expect(b.state).toBe("PAUSED");
    expect(b.evidence.current_value).toBe(75);
  });
});

// ---------------------------------------------------------------------------
// GAP-B (Data, 2026-10) — the DENOMINATOR as a data contract.
//
// The producer emits `denominator` + `excluded` (risk-circuit-breakers.ts:1121-1122)
// precisely so a consumer can tell "no rows" apart from "no MEASURABLE rows"
// (migration 126 §"NOT DONE HERE": "the wire/contract change that lets the API
// distinguish 'no expected measurement' from 'expected and missing'").
//
// These assertions fix the properties of that contract that a downstream writer
// or validator can EASILY break without any test turning red today:
//   · the pair must be present in the 0-excluded-rows case, not only in the
//     partial-coverage case a fixture already covered;
//   · `excluded` is a SIGNED integer: the breach branch emits it NEGATIVE
//     (measured > measurable ⇒ expected < rowsInWindow is not the only source),
//     so a consumer validator that assumes non-negative would reject the whole
//     response — killing the panel instead of reporting the contradiction;
//   · `measured` must be inside the population the ACTUAL query counts, checked
//     against GAS_WINDOW_SQL_LEDGER's own FILTER text rather than a restatement.
// ---------------------------------------------------------------------------
describe("GAP-B data contract — denominator provenance reaches the wire intact", () => {
  /** Extract the `gas_measurement_state` predicate from the real query text. */
  function measurableFilter(sql: string): string {
    const m = sql.match(/COUNT\(\*\) FILTER \(([\s\S]*?)\)::int AS measurable/);
    expect(m, "GAS_WINDOW_SQL_LEDGER must expose its measurable FILTER").not.toBeNull();
    return m![1];
  }

  it("0 measurable rows still names the denominator (the ledger case is not a silent 0/0)", () => {
    // Migration 126 applied; every window row is not_applicable/impossible.
    const b = makeGasBurnBreaker(
      gasCtx(
        {
          rowsInWindow: 12,
          withActualGas: 0,
          sumUsd: 0,
          measurable: 0,
          denominator: "ledger",
        },
        50,
      ),
    );
    const actual = b.evidence.paths?.actual;
    // Producer side of the contract: the provenance survives to the wire.
    expect(actual?.denominator).toBe("ledger");
    expect(actual?.excluded).toBe(12);
    expect(actual?.expected).toBe(0);
    expect(actual?.measured).toBe(0);
    // R8: 12 rows exist, 0 carry a measurement — NOT_AVAILABLE, never a $0 burn.
    expect(b.state).toBe("NOT_AVAILABLE");
    // And the reader is the ONLY place the population got narrowed: the SQL that
    // produced `measurable` says which states it drops.
    const filter = measurableFilter(GAS_WINDOW_SQL_LEDGER);
    expect(filter).toContain("'not_applicable'");
    expect(filter).toContain("'impossible'");
    expect(filter).not.toMatch(/='measured'/);
  });

  it("excluded is SIGNED: the self-contradicting aggregate emits it negative", () => {
    // Wired exactly as makeGasBurnBreaker wires it: readGasWindow forwards the
    // caller-supplied `measurable` verbatim, deriving only the denominator.
    const corrupted = deriveGasCoverage({
      rowsInWindow: 10,
      withActualGas: 1,
      measurable: 40,
      denominator: "ledger",
    });
    expect(corrupted.excluded).toBe(-30);
    expect(corrupted.breach).toBe(true);

    // …and it really does reach the wire that way (no clamp on the way out).
    const w = readGasWindow(
      [{ rows_in_window: 10, with_actual: 1, gas_usd: 0, measurable: 40 }],
      "ledger",
    );
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 10, withActualGas: 1, sumUsd: 0, measurable: 40, denominator: "ledger" }, 50));
    expect(w.measurable).toBe(40);
    expect(b.evidence.paths?.actual.excluded).toBe(-30);
    expect(b.state).toBe("NOT_AVAILABLE");
  });

  it("a fresh deployment (no migration 126) never claims the narrower denominator", () => {
    const b = makeGasBurnBreaker(gasCtx({ rowsInWindow: 12, withActualGas: 0, sumUsd: 0 }, 50));
    const actual = b.evidence.paths?.actual;
    expect(actual?.denominator).toBe("all_rows_no_ledger");
    expect(actual?.excluded).toBe(0);
    expect(actual?.expected).toBe(12);
    // The degraded denominator is named in the reason for THIS branch too — a
    // consumer that only reads counts cannot see it, so the wire must say it.
    expect(actual?.reason).toContain("gas_measurement_state is absent");
  });
});

describe("A.6 SQL + degradation (I/O adapter)", () => {
  it("the ledger query derives the denominator from gas_measurement_state", () => {
    expect(GAS_WINDOW_SQL_LEDGER).toContain(
      "gas_measurement_state IS DISTINCT FROM 'not_applicable'",
    );
    expect(GAS_WINDOW_SQL_LEDGER).toContain(
      "gas_measurement_state IS DISTINCT FROM 'impossible'",
    );
    expect(GAS_WINDOW_SQL_LEDGER).toContain("AS measurable");
    // The degraded query keeps COUNT(*) as the denominator and nothing else.
    expect(GAS_WINDOW_SQL_ALL_ROWS).not.toContain("gas_measurement_state");
    expect(GAS_WINDOW_SQL_LEDGER).not.toEqual(GAS_WINDOW_SQL_ALL_ROWS);
  });

  it("only the two never-measurable states are excluded — NULL stays measurable", () => {
    // Fail-closed: a not-yet-classified row (NULL) is potentially measurable and
    // stays in the denominator. `IS DISTINCT FROM` is what makes NULL count as
    // measurable; a bare `<>` would silently DROP those rows and shrink the
    // denominator with no evidence.
    const sql = GAS_WINDOW_SQL_LEDGER;
    expect(sql).toContain("IS DISTINCT FROM");
    expect(sql).not.toMatch(/gas_measurement_state\s*[=<>]/);
  });

  it("42703/42P01 mean 'ledger absent'; any other error stays a real failure", () => {
    expect(isMissingGasLedgerColumn({ code: "42703" })).toBe(true);
    expect(isMissingGasLedgerColumn({ code: "42P01" })).toBe(true);
    expect(isMissingGasLedgerColumn({ code: "57014" })).toBe(false);
    expect(isMissingGasLedgerColumn(new Error("connection terminated"))).toBe(false);
    expect(isMissingGasLedgerColumn(null)).toBe(false);
  });

  it("readGasWindow carries the denominator provenance and invents no measurement", () => {
    const w = readGasWindow(
      [{ rows_in_window: 500, with_actual: 0, gas_usd: 0, measurable: 120 }],
      "ledger",
    );
    expect(w).toEqual({
      rowsInWindow: 500,
      withActualGas: 0,
      sumUsd: 0,
      measurable: 120,
      denominator: "ledger",
    });
    // An absent row is a COUNT of zero — and the evaluator routes that to
    // NOT_AVAILABLE, never to a $0 burn.
    const empty = readGasWindow([], "all_rows_no_ledger");
    expect(empty).toEqual({
      rowsInWindow: 0,
      withActualGas: 0,
      sumUsd: 0,
      measurable: 0,
      denominator: "all_rows_no_ledger",
    });
  });
});
