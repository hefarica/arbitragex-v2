/**
 * Factor calculators — pure functions. No I/O, no async.
 *
 * Each returns a number in [0, 100] where higher = better for the system.
 * Inputs that would produce NaN/Inf are clamped. Missing inputs get a
 * documented neutral or conservative value. No fabrication beyond neutrals.
 */

import type { Opportunity, SimulationResult } from "@arbx/shared";

export type Factors = {
  liquidity: number;
  depth: number;
  safety: number;
  slippage: number;
  gas: number;
  risk: number;
};

const clamp = (x: number, lo: number, hi: number): number =>
  Number.isFinite(x) ? Math.max(lo, Math.min(hi, x)) : lo;

/** Liquidity proxy: log-scaled from expected_profit_usd. */
export function liquidityFactor(opp: Opportunity): number {
  const ep = Math.max(1, opp.expected_profit_usd);
  return clamp(Math.log10(ep * 20) * 20, 0, 100);
}

/** Depth: rough proxy until on-chain pool state is available (S6). */
export function depthFactor(opp: Opportunity): number {
  if (opp.dex_b && opp.dex_b !== opp.dex_a) return 60;
  return 40;
}

/** Safety: external provider or internal heuristic score, passed straight through. */
export function safetyFactor(safety_score: number): number {
  return clamp(safety_score, 0, 100);
}

/** Slippage: inverse of observed slippage_pct. Null → 50 neutral. */
export function slippageFactor(sim: SimulationResult | null): number {
  if (!sim || sim.slippage_pct == null) return 50;
  return clamp(100 - sim.slippage_pct * 30, 0, 100);
}

/** Gas: rudimentary. Without a sim, conservative 40; with sim, penalize high gas_price. */
export function gasFactor(sim: SimulationResult | null, maxGasPriceGwei: number): number {
  if (!sim) return 40;
  const gasWei = sim.gas_price_wei ? Number(sim.gas_price_wei) : 0;
  if (gasWei <= 0) return 50;
  const gasGwei = gasWei / 1e9;
  const ratio = Math.min(1, gasGwei / Math.max(1, maxGasPriceGwei));
  return clamp(100 * (1 - ratio), 0, 100);
}

/** Risk: inverse of revert_risk_pct. Null → 50 neutral. */
export function riskFactor(sim: SimulationResult | null): number {
  if (!sim || sim.revert_risk_pct == null) return 50;
  return clamp(100 - sim.revert_risk_pct, 0, 100);
}

export function computeFactors(
  opp: Opportunity,
  sim: SimulationResult | null,
  safety_score: number,
  maxGasPriceGwei: number,
): Factors {
  return {
    liquidity: liquidityFactor(opp),
    depth:     depthFactor(opp),
    safety:    safetyFactor(safety_score),
    slippage:  slippageFactor(sim),
    gas:       gasFactor(sim, maxGasPriceGwei),
    risk:      riskFactor(sim),
  };
}
