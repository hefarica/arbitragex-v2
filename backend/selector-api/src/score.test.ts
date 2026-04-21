import { describe, it, expect } from "vitest";
import { scoreOpportunity } from "./score.js";
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
  detected_at: "2026-04-20T00:00:00.000Z",
  trace_id: "00000000-0000-0000-0000-0000000000aa",
};

const passingSim: SimulationResult = {
  opportunity_id: baseOpp.id,
  passed: true,
  gas_estimate_wei: "21000",
  gas_price_wei: "20000000000",
  slippage_pct: 0.2,
  revert_risk_pct: 1,
  simulated_profit_usd: 28,
  simulator: "anvil",
  fail_reason: null,
  simulated_at: "2026-04-20T00:00:01.000Z",
  trace_id: baseOpp.trace_id,
};

describe("scoreOpportunity", () => {
  it("accepts a good, passing opportunity with decent safety", () => {
    const out = scoreOpportunity(baseOpp, passingSim, 80);
    expect(out.decision).toBe("accept");
    expect(out.score).toBeGreaterThanOrEqual(55);
  });

  it("rejects when safety_score is below the 50 floor", () => {
    const out = scoreOpportunity(baseOpp, passingSim, 30);
    expect(out.decision).toBe("reject");
    expect(out.reason).toBe("safety_below_threshold");
  });

  it("rejects when simulation_failed", () => {
    const failingSim: SimulationResult = { ...passingSim, passed: false, fail_reason: "revert" };
    const out = scoreOpportunity(baseOpp, failingSim, 80);
    expect(out.decision).toBe("reject");
    expect(out.reason).toBe("simulation_failed");
  });

  it("produces stable factor set", () => {
    const out = scoreOpportunity(baseOpp, passingSim, 80);
    expect(Object.keys(out.factors).sort()).toEqual(["depth","gas","liquidity","risk","safety","slippage"]);
  });
});
