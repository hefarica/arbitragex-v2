/**
 * Sprint 3 Task 3.1 — pmiCalculator unit tests.
 *
 * Pure function. No I/O. Inputs and expected outputs verifiable from
 * the PMI/EVM canonical formulas (Project Management Institute,
 * Earned Value Management) translated to crypto-MEV semantics:
 *
 *   CPI = profit_realized / capital_deployed
 *   SPI = ops_completed / (ops_target × elapsed_day_fraction)
 *   EAC = target_pnl × CPI
 *   ETC = capital × CPI × remaining_day_fraction
 *   TCPI = (target_pnl − profit_realized) / (capital − profit_realized)
 *   VAC = target_pnl − EAC
 *   CV  = profit_realized − planned_to_date
 *
 * NaN-safety: when any divisor is zero, return safeDiv fallback (0 by
 * default, 1 for TCPI to avoid suggesting "infinite required efficiency").
 */
import { describe, it, expect } from "vitest";

import { computeKpis, type PmiInput } from "./pmiCalculator.js";

const baseInput: PmiInput = {
  capitalDeployedUsd: 1000,
  profitRealizedUsd: 50,
  opsCompleted: 30,
  opsTargetPerDay: 100,
  minProfitUsdTarget: 5,
  elapsedDayFraction: 0.3, // 30% of UTC day elapsed
};

describe("pmiCalculator.computeKpis", () => {
  it("CPI = profit / capital_deployed", () => {
    const k = computeKpis(baseInput);
    expect(k.cpi).toBeCloseTo(50 / 1000, 6); // 0.05
  });

  it("SPI = ops_completed / (ops_target × elapsed_fraction)", () => {
    const k = computeKpis(baseInput);
    // 30 / (100 × 0.3) = 1.0 — exactly on track
    expect(k.spi).toBeCloseTo(1.0, 6);
  });

  it("EAC forecasts pnl scaled by CPI", () => {
    const k = computeKpis(baseInput);
    // target_pnl = 100 × 5 = 500; forecast = 500 × 0.05 = 25
    expect(k.eacUsd).toBeCloseTo(25, 6);
  });

  it("CV = profit_realized − (target_pnl × elapsed_fraction)", () => {
    const k = computeKpis(baseInput);
    // planned-to-date at 30% = 500 × 0.3 = 150; CV = 50 − 150 = −100
    expect(k.cvUsd).toBeCloseTo(-100, 6);
  });

  it("returns NaN-safe zeros when capital is zero", () => {
    const k = computeKpis({ ...baseInput, capitalDeployedUsd: 0 });
    expect(k.cpi).toBe(0);
    expect(Number.isFinite(k.eacUsd)).toBe(true);
    expect(Number.isFinite(k.etcUsd)).toBe(true);
    expect(Number.isFinite(k.tcpi)).toBe(true);
    expect(Number.isFinite(k.vacUsd)).toBe(true);
    expect(Number.isFinite(k.cvUsd)).toBe(true);
  });
});
