import { describe, it, expect } from "vitest";
import { decide } from "./engine.js";
import type { AppConfig, SimulationResult } from "@arbx/shared";
import type { TokenSafetyRecord } from "../token_safety/cache.js";

const cfg = {
  scoring: { min_accept_score: 55 },
  token_safety: { min_acceptable_score: 70 },
  risk: { max_revert_rate_pct: 5 },
} as unknown as AppConfig;

const safeOk: TokenSafetyRecord = {
  chain_id: 1, token_address: "0x" + "c".repeat(40), safety_score: 85, flags: {},
  source: "internal", ttl_expires_at: new Date(), updated_at: new Date(),
};
const safeBad: TokenSafetyRecord = { ...safeOk, safety_score: 40 };

describe("decide", () => {
  it("accepts when score high + safety ok + sim ok", () => {
    const d = decide({
      scored: { opportunity_id: "x", score: 80, factors: {} as any },
      safety: safeOk, sim: null, cfg,
    });
    expect(d.kind).toBe("accept");
    expect(d.score).toBe(80);
  });
  it("rejects when safety below threshold", () => {
    const d = decide({
      scored: { opportunity_id: "x", score: 80, factors: {} as any },
      safety: safeBad, sim: null, cfg,
    });
    expect(d.kind).toBe("reject");
    expect((d as any).reason).toBe("safety_below_threshold");
  });
  it("rejects when simulation_failed", () => {
    const sim: SimulationResult = {
      opportunity_id: "x", passed: false,
      gas_estimate_wei: null, gas_price_wei: null,
      slippage_pct: null, revert_risk_pct: null, simulated_profit_usd: null,
      simulator: "anvil", fail_reason: "revert",
      simulated_at: "2026-04-21T00:00:00.000Z",
      trace_id: "00000000-0000-0000-0000-000000000000",
    };
    const d = decide({
      scored: { opportunity_id: "x", score: 80, factors: {} as any },
      safety: safeOk, sim, cfg,
    });
    expect(d.kind).toBe("reject");
    expect((d as any).reason).toBe("simulation_failed");
  });
  it("rejects when revert_risk > 2× max", () => {
    const sim: SimulationResult = {
      opportunity_id: "x", passed: true,
      gas_estimate_wei: null, gas_price_wei: null,
      slippage_pct: null, revert_risk_pct: 20, simulated_profit_usd: null,
      simulator: "anvil", fail_reason: null,
      simulated_at: "2026-04-21T00:00:00.000Z",
      trace_id: "00000000-0000-0000-0000-000000000000",
    };
    const d = decide({
      scored: { opportunity_id: "x", score: 80, factors: {} as any },
      safety: safeOk, sim, cfg,
    });
    expect(d.kind).toBe("reject");
    expect((d as any).reason).toBe("revert_risk_too_high");
  });
  it("rejects when score below min", () => {
    const d = decide({
      scored: { opportunity_id: "x", score: 40, factors: {} as any },
      safety: safeOk, sim: null, cfg,
    });
    expect(d.kind).toBe("reject");
    expect((d as any).reason).toBe("score_below_min");
  });
});
