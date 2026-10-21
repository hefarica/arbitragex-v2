import { describe, expect, it } from "vitest";
import {
  computeSaveGate, evaluateChainGate, evaluateRugRules, evaluateVolumeGate,
  RUG_RULES, rankNote, rankValueOf, screenAndRank, selectionNote,
  tokenPricesKey, IMPLAUSIBLE_TVL_USD,
  type RankBy, type RugRuleId, type TopTokenRow,
} from "./token-top.js";

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

  it("flags high risk level, single pool and memecoin symbol", () => {
    const oneHourAgo = new Date(Date.now() - 1 * 3_600_000);
    const a = agg({ symbol: "BABYDOGE", risk_level: "HIGH", pool_count: 1, oldest_pool_at: oneHourAgo });
    const { failed } = evaluateRugRules(a, ALL);
    expect(failed).toEqual(expect.arrayContaining(["risk_level_ok", "multi_pool", "exclude_memecoins"]));
  });

  it("decision 6: pool_age_24h is NOT MEASURED and never rejects (it reads insertion time)", () => {
    const oneHourAgo = new Date(Date.now() - 1 * 3_600_000);
    const a = agg({ pool_count: 3, oldest_pool_at: oneHourAgo });
    const { failed, notMeasured } = evaluateRugRules(a, ALL);
    expect(failed).not.toContain("pool_age_24h");
    expect(notMeasured).toEqual(["pool_age_24h"]);
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

function row(over: Partial<TopTokenRow> & { address: string }): TopTokenRow {
  return {
    symbol: "ABC",
    address: over.address,
    tvl_usd: "1000000.00",
    tvl_24h_ago_usd: "900000.00",
    pool_count: 3,
    cache_score: 80,
    validation_score: 75,
    final_status: "VERIFIED",
    liquidity_usd: "500000.00",
    volume_24h_usd: "1000000.00",
    vol_liq_ratio: 2,
    registry_verified: true,
    oldest_pool_age_hours: 48,
    risk_level: "UNKNOWN",
    is_stablecoin: false,
    memecoin_heuristic: false,
    failed_rules: [],
    unverified_rules: [],
    ...over,
  };
}

describe("screenAndRank — basis de ranking (D5, 2026-10-21)", () => {
  it("orders by measured 24h volume, not by TVL", () => {
    const rows = [
      row({ address: "0xa", symbol: "BIG_TVL", tvl_usd: "900000000.00", volume_24h_usd: "1000.00" }),
      row({ address: "0xb", symbol: "BIG_VOL", tvl_usd: "1000.00", volume_24h_usd: "50000000.00" }),
      row({ address: "0xc", symbol: "MID", tvl_usd: "5000.00", volume_24h_usd: "2000000.00" }),
    ];
    const { ranked, excluded } = screenAndRank(rows, "volume_24h", 100);
    expect(ranked.map((r) => r.symbol)).toEqual(["BIG_VOL", "MID", "BIG_TVL"]);
    expect(excluded).toEqual({});
  });

  it("R8: a token with NULL 24h volume is excluded and counted, never ranked as 0", () => {
    const rows = [
      row({ address: "0xa", volume_24h_usd: null }),
      row({ address: "0xb", volume_24h_usd: "0.00" }),
      row({ address: "0xc", volume_24h_usd: "10.00" }),
    ];
    const { ranked, excluded } = screenAndRank(rows, "volume_24h", 100);
    expect(ranked.map((r) => r.address)).toEqual(["0xc"]);
    expect(excluded["no_volume_24h"]).toBe(2);
  });

  it("excludes an implausible pool valuation (the ADI 6.55e32 row) from every basis", () => {
    const rows = [
      row({ address: "0xadi", tvl_usd: "6.559901993815026e+32", volume_24h_usd: "999999999.00" }),
      row({ address: "0xok", tvl_usd: "1000000.00", volume_24h_usd: "5.00" }),
    ];
    for (const basis of ["tvl", "volume_24h"] as RankBy[]) {
      const { ranked, excluded } = screenAndRank(rows, basis, 100);
      expect(ranked.map((r) => r.address)).toEqual(["0xok"]);
      expect(excluded["implausible_tvl"]).toBe(1);
    }
    expect(Number("6.559901993815026e+32")).toBeGreaterThan(IMPLAUSIBLE_TVL_USD);
  });

  it("tvl basis keeps the 24h window ordering (capital flow) and the current ordering apart", () => {
    const rows = [
      row({ address: "0xa", tvl_usd: "100.00", tvl_24h_ago_usd: "900.00" }),
      row({ address: "0xb", tvl_usd: "500.00", tvl_24h_ago_usd: "100.00" }),
    ];
    expect(screenAndRank(rows, "tvl", 100, "current").ranked.map((r) => r.address)).toEqual(["0xb", "0xa"]);
    expect(screenAndRank(rows, "tvl", 100, "24h").ranked.map((r) => r.address)).toEqual(["0xa", "0xb"]);
    expect(rankValueOf(rows[0], "volume_24h")).toBe(1000000);
  });

  it("respects the limit and keeps a deterministic tie-break", () => {
    const rows = [
      row({ address: "0xbb", volume_24h_usd: "7.00", tvl_usd: "10.00" }),
      row({ address: "0xaa", volume_24h_usd: "7.00", tvl_usd: "10.00" }),
      row({ address: "0xcc", volume_24h_usd: "9.00", tvl_usd: "1.00" }),
    ];
    const { ranked } = screenAndRank(rows, "volume_24h", 2);
    expect(ranked.map((r) => r.address)).toEqual(["0xcc", "0xaa"]);
  });

  it("D-SEL-2: a null TVL is excluded and counted, never read as 0", () => {
    const rows = [
      row({ address: "0xnull", tvl_usd: null }),
      row({ address: "0xok", tvl_usd: "10.00" }),
    ];
    const { ranked, excluded } = screenAndRank(rows, "tvl", 100);
    expect(ranked.map((r) => r.address)).toEqual(["0xok"]);
    expect(excluded["no_reserve"]).toBe(1);
  });

  it("D-SEL-1: a null 24h snapshot stays null and is counted separately", () => {
    const rows = [row({ address: "0xnew", tvl_24h_ago_usd: null })];
    const { excluded } = screenAndRank(rows, "tvl", 100);
    expect(excluded["no_24h_snapshot"]).toBe(1);
    expect(excluded["no_reserve"]).toBeUndefined();
  });

  it("names its own basis in the served note", () => {
    expect(rankNote("volume_24h")).toContain("volumen 24h");
    expect(rankNote("tvl")).toContain("valor en pools");
  });
});

describe("evaluateVolumeGate — §2(i)(ii) volume freshness (2026-10-21)", () => {
  it("volume basis without the column is NOT COMPUTED with its own cause", () => {
    const g = evaluateVolumeGate({ rankBy: "volume_24h", columnPresent: false, probeFailed: false, orderable: 0 });
    expect(g.selection_state).toBe("NOT COMPUTED");
    expect(g.selection_reason).toBe("volume_as_of_absent");
    expect(selectionNote("volume_24h", g)).toContain("volume_as_of no existe");
  });

  it("volume basis with the column but zero fresh rows is NOT COMPUTED, not 0/0 green", () => {
    const g = evaluateVolumeGate({ rankBy: "volume_24h", columnPresent: true, probeFailed: false, orderable: 0 });
    expect(g.selection_state).toBe("NOT COMPUTED");
    expect(g.selection_reason).toBe("no_fresh_volume_snapshot");
  });

  it("a probe failure is its own cause, never a silent pass", () => {
    const g = evaluateVolumeGate({ rankBy: "volume_24h", columnPresent: true, probeFailed: true, orderable: 9 });
    expect(g.selection_state).toBe("NOT COMPUTED");
    expect(g.selection_reason).toBe("volume_as_of_probe_failed");
  });

  it("the tvl basis is out of scope of this gate", () => {
    const g = evaluateVolumeGate({ rankBy: "tvl", columnPresent: false, probeFailed: false, orderable: 0 });
    expect(g.selection_state).toBe("MEASURED");
  });

  it("a fresh population makes the volume basis MEASURED and reports its size", () => {
    const g = evaluateVolumeGate({ rankBy: "volume_24h", columnPresent: true, probeFailed: false, orderable: 42 });
    expect(g).toEqual({ selection_state: "MEASURED", selection_reason: null, volume_orderable: 42 });
  });
});

describe("min_safety_score — one instrument, one threshold (D-SEL-6, decision 7)", () => {
  it("the threshold applies to validations.score, not to the cache score", () => {
    const onlyCache = agg({ safety: 95, val: { ...agg().val!, score: 30 } });
    expect(evaluateRugRules(onlyCache, new Set<RugRuleId>(["min_safety_score"])).failed)
      .toEqual(["min_safety_score"]);

    const onlyValidations = agg({ safety: 10, val: { ...agg().val!, score: 90 } });
    expect(evaluateRugRules(onlyValidations, new Set<RugRuleId>(["min_safety_score"])).failed)
      .toEqual([]);
  });
});

describe("computeSaveGate — write lock (decision 3, 2026-10-21)", () => {
  const pop = (evaluated: number, unverified = 0, diagnostic = false) => ({ evaluated, unverified, diagnostic });

  it("allows the write only with a computed selection and a verified rule", () => {
    expect(
      computeSaveGate({
        selectionState: "MEASURED",
        rulePopulation: { min_liquidity_usd: pop(10) },
        rulesApplied: ["min_liquidity_usd"],
      }),
    ).toEqual({ allowed: true, reasons: [] });
  });

  it("blocks the write when the selection is not computed", () => {
    const g = computeSaveGate({
      selectionState: "NOT COMPUTED",
      rulePopulation: { min_liquidity_usd: pop(10) },
      rulesApplied: ["min_liquidity_usd"],
    });
    expect(g.allowed).toBe(false);
    expect(g.reasons).toContain("selection_not_computed");
  });

  it("blocks when every applied rule is unverified — 0 population is not a pass", () => {
    const g = computeSaveGate({
      selectionState: "MEASURED",
      rulePopulation: { min_liquidity_usd: pop(0, 12) },
      rulesApplied: ["min_liquidity_usd"],
    });
    expect(g.allowed).toBe(false);
    expect(g.reasons).toEqual(["no_verified_rule_population"]);
  });

  it("blocks when no rule is applied at all", () => {
    const g = computeSaveGate({ selectionState: "MEASURED", rulePopulation: {}, rulesApplied: [] });
    expect(g.reasons).toEqual(["no_rules_applied"]);
  });

  it("a diagnostic rule never counts as the verified population", () => {
    const g = computeSaveGate({
      selectionState: "MEASURED",
      rulePopulation: { pool_age_24h: pop(10, 0, true) },
      rulesApplied: ["pool_age_24h"],
    });
    expect(g.allowed).toBe(false);
    expect(g.reasons).toEqual(["no_verified_rule_population"]);
  });
});

describe("price tower per chain — §6 (decision 5, 2026-10-21)", () => {
  it("builds the key from the chain, never a literal chain-1 key", () => {
    expect(tokenPricesKey(1)).toBe("arbx:token_prices:1");
    expect(tokenPricesKey(42161)).toBe("arbx:token_prices:42161");
  });

  it("only a proven chain is ranked; the rest are NOT COMPUTED", () => {
    expect(evaluateChainGate(1)).toEqual({ selection_state: "MEASURED", selection_reason: null });
    expect(evaluateChainGate(8453)).toEqual({
      selection_state: "NOT COMPUTED",
      selection_reason: "price_tower_not_parameterized",
    });
  });
});
