/**
 * Sprint 3 Task 3.1 — PMI / Earned Value Management metrics for crypto MEV ops.
 *
 * Pure functions. No I/O. Inputs come from SQL aggregations elsewhere
 * (see backend/api-server/src/routes/operations.ts).
 *
 * Translation reference: docs/superpowers/specs/2026-05-03-sop-evm-integration-design.md §6.
 */

export interface PmiInput {
  /** Operator-supplied capital allocation for this chain (`trading_config.capital_usd`). */
  capitalDeployedUsd: number;
  /** Sum of `executions.actual_profit_usd` since UTC midnight. */
  profitRealizedUsd: number;
  /** Count of executions since UTC midnight. */
  opsCompleted: number;
  /** Operator's daily ops target. Until UI seeds it, defaulted by caller. */
  opsTargetPerDay: number;
  /** Operator's `trading_config.min_profit_usd` (per-op profit target). */
  minProfitUsdTarget: number;
  /** Fraction of the UTC day elapsed, in [0,1]. */
  elapsedDayFraction: number;
}

export interface PmiKpis {
  /** Capital efficiency: profit_realized / capital_deployed. */
  cpi: number;
  /** Velocity index: ops_completed / (ops_target × elapsed_fraction). */
  spi: number;
  /** Forecast daily PnL: target_pnl × CPI. */
  eacUsd: number;
  /** Remaining capital × CPI × remaining_day_fraction. */
  etcUsd: number;
  /** Required efficiency to hit target: (target − realized) / (capital − realized). */
  tcpi: number;
  /** Projected variance: target_pnl − EAC. */
  vacUsd: number;
  /** Realized variance: profit_realized − planned_to_date. */
  cvUsd: number;
}

const safeDiv = (a: number, b: number, fallback = 0): number => {
  if (!Number.isFinite(a) || !Number.isFinite(b) || b === 0) return fallback;
  return a / b;
};

export function computeKpis(input: PmiInput): PmiKpis {
  const {
    capitalDeployedUsd,
    profitRealizedUsd,
    opsCompleted,
    opsTargetPerDay,
    minProfitUsdTarget,
    elapsedDayFraction,
  } = input;

  const cpi = safeDiv(profitRealizedUsd, capitalDeployedUsd);
  // Floor elapsed_fraction at 1% to avoid pathological SPI early in the day.
  const spi = safeDiv(opsCompleted, opsTargetPerDay * Math.max(elapsedDayFraction, 0.01));
  const targetPnlUsd = opsTargetPerDay * minProfitUsdTarget;
  const plannedToDateUsd = targetPnlUsd * elapsedDayFraction;
  const eacUsd = targetPnlUsd * cpi;
  const remainingDayFraction = Math.max(0, 1 - elapsedDayFraction);
  const etcUsd = capitalDeployedUsd * cpi * remainingDayFraction;
  // TCPI fallback = 1 → "current efficiency is sufficient" when divisor degenerate.
  const tcpi = safeDiv(targetPnlUsd - profitRealizedUsd, capitalDeployedUsd - profitRealizedUsd, 1);
  const vacUsd = targetPnlUsd - eacUsd;
  const cvUsd = profitRealizedUsd - plannedToDateUsd;

  return { cpi, spi, eacUsd, etcUsd, tcpi, vacUsd, cvUsd };
}
