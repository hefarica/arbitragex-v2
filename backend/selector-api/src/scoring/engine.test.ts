import { describe, it, expect } from "vitest";
import { scoreOpportunity, weightsFromConfig, type ScoringWeights } from "./engine.js";
import { liquidityFactor, depthFactor, safetyFactor, slippageFactor, gasFactor, riskFactor } from "./factors.js";
import type { Opportunity, SimulationResult } from "@arbx/shared";

const baseOpp: Opportunity = {
  id: "00000000-0000-0000-0000-000000000001",
  chain_id: 1,
  strategy_kind: "dex_arb",
  dex_a: "uniswap-v3",
  dex_b: "curve",
  pair_symbol: "WETH/USDC",
  token_in: "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2",
  token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
  amount_in_wei: "1000000000000000000",
  expected_profit_usd: 30,
  roi_pct: null, risk_score: null, block_number: null,
  detected_at: "2026-04-21T00:00:00.000Z",
  trace_id: "00000000-0000-0000-0000-0000000000aa",
};

const goodSim: SimulationResult = {
  opportunity_id: baseOpp.id,
  passed: true,
  gas_estimate_wei: "21000",
  gas_price_wei: "20000000000",
  slippage_pct: 0.2,
  revert_risk_pct: 1,
  simulated_profit_usd: 28,
  simulator: "anvil",
  fail_reason: null,
  simulated_at: "2026-04-21T00:00:01.000Z",
  trace_id: baseOpp.trace_id,
};

const defaultWeights: ScoringWeights = {
  min_accept_score: 55,
  liquidity: 0.20, depth: 0.15, safety: 0.20,
  slippage: 0.15, gas: 0.15, risk: 0.15,
};

describe("factors", () => {
  it("liquidity scales with profit (log)", () => {
    expect(liquidityFactor({ ...baseOpp, expected_profit_usd: 0 })).toBeGreaterThanOrEqual(0);
    const small = liquidityFactor({ ...baseOpp, expected_profit_usd: 1 });
    const big = liquidityFactor({ ...baseOpp, expected_profit_usd: 10000 });
    expect(big).toBeGreaterThan(small);
    expect(big).toBeLessThanOrEqual(100);
  });
  it("depth favors two-dex paths", () => {
    expect(depthFactor(baseOpp)).toBe(60);
    expect(depthFactor({ ...baseOpp, dex_b: null })).toBe(40);
  });
  it("safety passes through, clamped", () => {
    expect(safetyFactor(80)).toBe(80);
    expect(safetyFactor(-5)).toBe(0);
    expect(safetyFactor(9999)).toBe(100);
  });
  it("slippage null → 50 neutral", () => {
    expect(slippageFactor(null)).toBe(50);
    expect(slippageFactor({ ...goodSim, slippage_pct: 0 })).toBe(100);
    expect(slippageFactor({ ...goodSim, slippage_pct: 10 })).toBeLessThan(80);
  });
  it("gas null → 40 conservative", () => {
    expect(gasFactor(null, 200)).toBe(40);
  });
  it("risk null → 50 neutral", () => {
    expect(riskFactor(null)).toBe(50);
    expect(riskFactor({ ...goodSim, revert_risk_pct: 0 })).toBe(100);
    expect(riskFactor({ ...goodSim, revert_risk_pct: 80 })).toBeLessThan(30);
  });
});

describe("scoreOpportunity", () => {
  it("high-quality opp above threshold", () => {
    const out = scoreOpportunity(baseOpp, goodSim, 85, defaultWeights, 200);
    expect(out.score).toBeGreaterThanOrEqual(defaultWeights.min_accept_score);
    expect(out.factors.safety).toBe(85);
  });
  it("low safety drags the score", () => {
    const a = scoreOpportunity(baseOpp, goodSim, 85, defaultWeights, 200);
    const b = scoreOpportunity(baseOpp, goodSim, 10, defaultWeights, 200);
    expect(a.score).toBeGreaterThan(b.score);
  });
  it("null sim yields neutral factors, not a crash", () => {
    const out = scoreOpportunity(baseOpp, null, 70, defaultWeights, 200);
    expect(out.factors.slippage).toBe(50);
    expect(out.factors.risk).toBe(50);
    expect(out.factors.gas).toBe(40);
    expect(Number.isFinite(out.score)).toBe(true);
  });
  it("weightsFromConfig roundtrips", () => {
    const cfg = {
      scoring: {
        min_accept_score: 60,
        weight_liquidity: 0.1, weight_depth: 0.1, weight_safety: 0.4,
        weight_slippage: 0.1, weight_gas: 0.1, weight_risk: 0.2,
      },
    } as unknown as Parameters<typeof weightsFromConfig>[0];
    const w = weightsFromConfig(cfg);
    expect(w.min_accept_score).toBe(60);
    expect(w.safety).toBe(0.4);
  });
});
