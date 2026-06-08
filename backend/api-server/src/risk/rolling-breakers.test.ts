// Pins the shared semantics with backend/shared-rs/src/risk_ledger.rs.
// If these diverge from the Rust unit tests, the two implementations have drifted.
import { describe, it, expect } from "vitest";
import {
  computeBreakerWindow,
  loadThresholdsFromEnv,
  type BreakerThresholds,
  type TradeOutcome,
} from "./rolling-breakers.js";

const T: BreakerThresholds = {
  windowSecs: 86_400,
  minSamples: 2,
  navBaselineUsd: 1_000,
  ddWarnPct: 10,
  ddPausePct: 20,
  ddHardPausePct: 30,
  ddKillPct: 40,
  maxGasBurnUsd: 100,
};

const o = (tsUnix: number, pnlUsd: number, gasUsd: number): TradeOutcome => ({ tsUnix, pnlUsd, gasUsd });

describe("computeBreakerWindow (mirror of risk_ledger::compute_breakers)", () => {
  it("empty input is insufficient and ok", () => {
    const s = computeBreakerWindow(1_000, [], { ...T, minSamples: 5 });
    expect(s.windowSamples).toBe(0);
    expect(s.insufficientEvidence).toBe(true);
    expect(s.drawdownPct.sufficient).toBe(false);
    expect(s.equityUsd).toBe(1_000);
  });

  it("below min_samples never trips", () => {
    const s = computeBreakerWindow(1_000, [o(900, -900, 500)], { ...T, minSamples: 5 });
    expect(s.insufficientEvidence).toBe(true);
    expect(s.gasBurnUsd.level).toBe("ok");
    expect(s.drawdownPct.level).toBe("ok");
  });

  it("peak then drop computes the drawdown tier (25% -> pause)", () => {
    // NAV 1000 -> +200=1200 (peak) -> -300=900. dd = 300/1200 = 25% -> pause.
    const s = computeBreakerWindow(1_000, [o(100, 200, 1), o(200, -300, 1)], T);
    expect(s.drawdownPct.value).toBeCloseTo(25, 9);
    expect(s.drawdownPct.level).toBe("pause");
  });

  it("drawdown kill tier at 40%", () => {
    const s = computeBreakerWindow(1_000, [o(100, 0, 0), o(200, -400, 0)], T);
    expect(s.drawdownPct.value).toBeCloseTo(40, 9);
    expect(s.drawdownPct.level).toBe("kill");
  });

  it("monotonic-up equity has zero drawdown", () => {
    const s = computeBreakerWindow(1_000, [o(100, 10, 1), o(200, 20, 1), o(300, 30, 1)], { ...T, minSamples: 3 });
    expect(s.drawdownPct.value).toBeCloseTo(0, 9);
    expect(s.drawdownPct.level).toBe("ok");
    expect(s.equityUsd).toBe(1_060);
  });

  it("gas burn trips pause at the cap", () => {
    const s = computeBreakerWindow(1_000, [o(100, 1, 60), o(200, 1, 50), o(300, 1, 0)], { ...T, minSamples: 3 });
    expect(s.gasBurnUsd.value).toBeCloseTo(110, 9);
    expect(s.gasBurnUsd.level).toBe("pause");
  });

  it("window filter excludes old and future samples", () => {
    const s = computeBreakerWindow(
      1_000_000,
      [o(100, -500, 999), o(913_600, -100, 10), o(1_000_000, -50, 5), o(2_000_000, -999, 999)],
      T,
    );
    expect(s.windowSamples).toBe(2);
    expect(s.gasBurnUsd.value).toBeCloseTo(15, 9);
  });

  it("non-positive NAV does not divide-by-zero", () => {
    const s = computeBreakerWindow(1_000, [o(100, -10, 1)], { ...T, minSamples: 1, navBaselineUsd: 0 });
    expect(s.drawdownPct.value).toBe(0);
    expect(s.drawdownPct.level).toBe("ok");
  });
});

describe("loadThresholdsFromEnv (no-hardcode: null unless fully configured)", () => {
  it("returns null when unset", () => {
    expect(loadThresholdsFromEnv({})).toBeNull();
  });

  it("returns null on a partial/invalid tier list", () => {
    expect(
      loadThresholdsFromEnv({
        ARBX_RISK_NAV_USD: "1000",
        ARBX_RISK_WINDOW_SECS: "86400",
        ARBX_RISK_MIN_SAMPLES: "10",
        ARBX_RISK_GAS_CAP_USD: "100",
        ARBX_RISK_DD_TIERS: "10,20,30", // only 3 — invalid
      }),
    ).toBeNull();
  });

  it("parses a fully-configured env", () => {
    const t = loadThresholdsFromEnv({
      ARBX_RISK_NAV_USD: "5000",
      ARBX_RISK_WINDOW_SECS: "86400",
      ARBX_RISK_MIN_SAMPLES: "20",
      ARBX_RISK_GAS_CAP_USD: "250",
      ARBX_RISK_DD_TIERS: "10,20,30,40",
    });
    expect(t).not.toBeNull();
    expect(t?.navBaselineUsd).toBe(5000);
    expect(t?.ddKillPct).toBe(40);
    expect(t?.maxGasBurnUsd).toBe(250);
  });
});
