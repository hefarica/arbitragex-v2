import { describe, expect, it } from "vitest";
import { evaluateRugRules, RUG_RULES, type RugRuleId } from "./token-top.js";

const ALL = new Set<RugRuleId>(RUG_RULES.map((r) => r.id));

function agg(overrides: Partial<Parameters<typeof evaluateRugRules>[0]> = {}) {
  const twoDaysAgo = new Date(Date.now() - 48 * 3_600_000);
  return {
    symbol: "ABC",
    address: "0xabc",
    risk_level: "UNKNOWN",
    is_stablecoin: false,
    pool_count: 3,
    oldest_pool_at: twoDaysAgo,
    tvl_current: 1_000_000,
    tvl_24h: 900_000,
    val: {
      address: "0xabc",
      liquidity_usd: "500000",
      volume_24h_usd: "1000000",
      volume_1h_usd: "100000",
      volume_5m_usd: "10000",
      registry_verified: true,
      final_status: "VERIFIED",
      score: 75,
    },
    safety: 80,
    ...overrides,
  } as Parameters<typeof evaluateRugRules>[0];
}

describe("evaluateRugRules (PC-07)", () => {
  it("clean token passes all 10 rules", () => {
    const { failed, unverified } = evaluateRugRules(agg(), ALL);
    expect(failed).toEqual([]);
    expect(unverified).toEqual([]);
  });

  it("flags thin liquidity, illiquid status, low safety, unverified registry", () => {
    const a = agg({
      safety: null,
      val: {
        address: "0xabc", liquidity_usd: "1000", volume_24h_usd: "500",
        volume_1h_usd: "100", volume_5m_usd: "1",
        registry_verified: false, final_status: "ILLIQUID", score: 10,
      },
    });
    const { failed } = evaluateRugRules(a, ALL);
    expect(failed).toEqual(expect.arrayContaining([
      "min_safety_score", "min_liquidity_usd", "registry_verified", "validated_status",
    ]));
  });

  it("flags wash-trade churn and 5m pump spike", () => {
    const a = agg({
      val: {
        address: "0xabc", liquidity_usd: "10000", volume_24h_usd: "500000",
        volume_1h_usd: "1000", volume_5m_usd: "500",
        registry_verified: true, final_status: "VIABLE", score: 60,
      },
    });
    const { failed } = evaluateRugRules(a, ALL);
    expect(failed).toContain("vol_liq_ratio_cap");
    expect(failed).toContain("pump_5m_spike");
  });

  it("flags high risk level, single pool, fresh pool, and memecoin symbol", () => {
    const oneHourAgo = new Date(Date.now() - 1 * 3_600_000);
    const a = agg({ symbol: "BABYDOGE", risk_level: "HIGH", pool_count: 1, oldest_pool_at: oneHourAgo });
    const { failed } = evaluateRugRules(a, ALL);
    expect(failed).toEqual(expect.arrayContaining(["risk_level_ok", "multi_pool", "pool_age_24h", "exclude_memecoins"]));
  });

  it("R8: NULL metrics are unverified, never a fabricated pass or fail", () => {
    const a = agg({ safety: null, val: null, risk_level: null });
    const { failed, unverified } = evaluateRugRules(a, ALL);
    expect(failed).toEqual([]);
    expect(unverified).toEqual(expect.arrayContaining([
      "min_safety_score", "validated_status", "min_liquidity_usd",
      "vol_liq_ratio_cap", "pump_5m_spike", "registry_verified", "risk_level_ok",
    ]));
  });

  it("disabled rules are not evaluated", () => {
    const a = agg({ symbol: "PEPE", pool_count: 1 });
    const { failed } = evaluateRugRules(a, new Set());
    expect(failed).toEqual([]);
  });
});
