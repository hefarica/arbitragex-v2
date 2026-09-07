/**
 * Unit tests for the target-driven simulation pipeline.
 *
 * The TS formula mirrors `backend/math-engine/src/roi_engine.rs:149-212`.
 * The first "canonical-scenario" test pins the TS output to a hand-
 * calculated reference so drift between Rust and TS is detectable in CI.
 */

import { describe, expect, it } from "vitest";
import type { Redis } from "ioredis";
import {
  forwardSimulate,
  inverseSize,
  resolveTarget,
  type SimulatorRow,
} from "./computeSimulatedNet.js";
import {
  getTradingConfigForChain,
  _clearSnapshotCacheForTests,
  type TradingConfigSnapshot,
} from "./tradingConfigSnapshot.js";

// ── Fixtures ────────────────────────────────────────────────────────────────

function baseCfg(): TradingConfigSnapshot {
  return {
    capital_usd: 100_000,
    base_token_symbol: "WETH",
    base_token_price_usd: 2350,
    token_prices_usd: { WETH: 2350, USDC: 1, USDT: 1, WBTC: 81324 },

    gas_estimate_units: 250_000,
    gas_price_strategy: "fixed",
    fixed_gas_price_gwei: 0.3,
    max_slippage_pct: 0.005,         // 0.5%
    failure_risk_buffer_pct: 0.001,  // 0.1%
    flashloan_fee_pct: 0.0009,       // 9bps Aave V3-ish
    lp_fee_default_pct: 0.003,       // WO-04 (2026-09-06): V2 30bps tier default

    capital_cost_rate_annual_pct: 0,
    ops_overhead_usd_per_attempt: 0.01,
    p_copied_max: 0,                 // disable copied-buffer for clean math
    p_copied_volume_threshold_usd: 1_000_000,
    spread_sanity_mult: 3.0,

    enabled_strategies: ["dex_arb"],
    strategy_configs: {},

    simulation_capital_usd: null,
    simulation_per_token_amounts_usd: {},
    simulation_per_strategy_caps_usd: {},
    simulation_target_profit_usd: null,
    simulation_target_roi_pct: null,
  };
}

function baseRow(over: Partial<SimulatorRow> = {}): SimulatorRow {
  return {
    chain_id: 1,
    strategy_kind: "dex_arb",
    // 1 WETH at 1e18 wei
    amount_in_wei: "1000000000000000000",
    expected_profit_usd: 100,
    token_in: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    token_in_symbol: "WETH",
    token_in_decimals: 18,
    ...over,
  };
}

// ── forwardSimulate ─────────────────────────────────────────────────────────

describe("forwardSimulate", () => {
  it("computes net within $1 of hand-calculated value for canonical scenario", () => {
    const cfg = baseCfg();
    const row = baseRow();
    const r = forwardSimulate(row, cfg);
    expect(r).not.toBeNull();
    expect(r!.amount_in_usd).toBeCloseTo(2350, 5);
    expect(r!.gross_usd).toBe(100);

    // Hand calculation, gross $100, amount_in $2350:
    //   gas:           250_000 × 0.3 × 1e9 / 1e18 × 2350 = $0.17625
    //   flashloan:     2350 × 0.0009 = $2.115
    //   lp_fees:       2350 × 0.003 = $7.05
    //   slippage:      2350 × 0.005 = $11.75
    //   failure_buf:   2350 × 0.001 = $2.35
    //   copied:        100 × 0 = 0
    //   capital_cost:  0 (rate=0)
    //   ops:           $0.01
    //   relay:         max(100 × 0.05, 0.5) = $5.0  (mainnet)
    //   net = 100 - 0.17625 - 2.115 - 7.05 - 11.75 - 2.35 - 0 - 0 - 0.01 - 5.0 = 71.55375
    expect(r!.cost_breakdown.gas_usd).toBeCloseTo(0.17625, 5);
    expect(r!.cost_breakdown.flashloan_fee_usd).toBeCloseTo(2.115, 5);
    expect(r!.cost_breakdown.lp_fees_usd).toBeCloseTo(7.05, 5);
    expect(r!.cost_breakdown.slippage_usd).toBeCloseTo(11.75, 5);
    expect(r!.cost_breakdown.failure_buffer_usd).toBeCloseTo(2.35, 5);
    expect(r!.cost_breakdown.copied_buffer_usd).toBeCloseTo(0, 5);
    expect(r!.cost_breakdown.capital_cost_usd).toBeCloseTo(0, 5);
    expect(r!.cost_breakdown.ops_overhead_usd).toBe(0.01);
    expect(r!.cost_breakdown.relay_fee_usd).toBeCloseTo(5.0, 5);
    expect(r!.net_usd).toBeCloseTo(71.55375, 2);
    expect(r!.source).toBe("simulation");
  });

  it("relay floor of $0.50 binds on tiny gross", () => {
    const cfg = baseCfg();
    const row = baseRow({ expected_profit_usd: 1 });
    const r = forwardSimulate(row, cfg)!;
    // gross × 5% = $0.05, floor of $0.50 binds.
    expect(r.cost_breakdown.relay_fee_usd).toBeCloseTo(0.5, 5);
  });

  it("returns null when expected_profit_usd is null", () => {
    const cfg = baseCfg();
    const row = baseRow({ expected_profit_usd: null });
    expect(forwardSimulate(row, cfg)).toBeNull();
  });

  it("returns null when token symbol cannot be priced", () => {
    const cfg = baseCfg();
    const row = baseRow({ token_in_symbol: "UNKNOWNTKN" });
    expect(forwardSimulate(row, cfg)).toBeNull();
  });

  it("returns null when token_in_decimals is missing", () => {
    const cfg = baseCfg();
    const row = baseRow({ token_in_decimals: null });
    expect(forwardSimulate(row, cfg)).toBeNull();
  });

  it("relay_fee is 0 on L2 chains (non-mainnet)", () => {
    const cfg = baseCfg();
    const row = baseRow({ chain_id: 42161 }); // Arbitrum
    const r = forwardSimulate(row, cfg)!;
    expect(r.cost_breakdown.relay_fee_usd).toBe(0);
  });

  it("scales gas with fixed_gas_price_gwei", () => {
    const cfg = baseCfg();
    cfg.fixed_gas_price_gwei = 3.0;  // 10x more expensive
    const row = baseRow();
    const r = forwardSimulate(row, cfg)!;
    expect(r.cost_breakdown.gas_usd).toBeCloseTo(0.17625 * 10, 4);
  });

  it("p_copied_max scales copied_buffer with gross", () => {
    const cfg = baseCfg();
    cfg.p_copied_max = 0.1;  // 10%
    const row = baseRow();
    const r = forwardSimulate(row, cfg)!;
    expect(r.cost_breakdown.copied_buffer_usd).toBeCloseTo(100 * 0.1, 5);
    // Notes should mention it.
    expect(r.notes.some((n) => n.includes("p-copied-max"))).toBe(true);
  });
});

// ── WO-04 (2026-09-06): lp_fee_default_pct parameterization ────────────────
//
// The 30 bps LP-fee literal (was LP_FEE_FRACTION_DEFAULT = 0.003 at
// computeSimulatedNet.ts:139) is now the trading_config knob
// `lp_fee_default_pct` (migration 119; PUT admin zod + snapshot default).
// Case 1 pins the FIRST-DEPLOY INVARIANT: a Redis blob persisted BEFORE
// migration 119 (field absent) parses to 0.003 → behavior byte-identical to
// the hardcoded era. Case 2 proves the knob actually turns the math.

describe("WO-04 (2026-09-06): lp_fee_default_pct", () => {
  it("Case 1 — first-deploy invariant: blob without the field ⇒ 0.003, note lp-fee=30bps-proxy", async () => {
    _clearSnapshotCacheForTests();
    // Hand-built config blob exactly as a pre-migration-119 PUT would have
    // persisted it — deliberately WITHOUT lp_fee_default_pct.
    const blob = JSON.stringify({
      capital_usd: 100_000,
      base_token_symbol: "WETH",
      base_token_price_usd: 2350,
      token_prices_usd: { WETH: 2350 },
      gas_estimate_units: 250_000,
      gas_price_strategy: "fixed",
      fixed_gas_price_gwei: 0.3,
      max_slippage_pct: 0.005,
      failure_risk_buffer_pct: 0.001,
      flashloan_fee_pct: 0.0009,
      capital_cost_rate_annual_pct: 0,
      ops_overhead_usd_per_attempt: 0.01,
      p_copied_max: 0,
    });
    // Redis client double at the infrastructure boundary — the DATA is the
    // hand-built blob above (fixture, not a mock of results under test).
    const stubRedis = { get: async (_key: string) => blob } as unknown as Redis;
    const snapshot = await getTradingConfigForChain(stubRedis, 1);
    expect(snapshot).not.toBeNull();
    // Doctrinal default from parseSnapshot — mirrors the Rust serde default.
    expect(snapshot!.lp_fee_default_pct).toBe(0.003);

    const r = forwardSimulate(baseRow(), snapshot!)!;
    // 1 WETH = $2350 amount_in ⇒ lp_fees = 2350 × 0.003 = $7.05 (unchanged).
    expect(r.cost_breakdown.lp_fees_usd).toBeCloseTo(7.05, 5);
    // Exact note string of the hardcoded era (Math.round(0.003 * 10_000) === 30).
    expect(r.notes).toContain("lp-fee=30bps-proxy");
  });

  it("Case 2 — override 0.001 (10 bps): lp fees + dynamic note + varCostRate reflect the knob", () => {
    const cfg = baseCfg();
    cfg.lp_fee_default_pct = 0.001;
    const r = forwardSimulate(baseRow(), cfg)!;
    expect(r.cost_breakdown.lp_fees_usd).toBeCloseTo(2350 * 0.001, 5); // $2.35
    expect(r.notes).toContain("lp-fee=10bps-proxy");
    expect(r.notes).not.toContain("lp-fee=30bps-proxy");

    // varCostRateFromCfg is private — observe its delta through inverseSize:
    // varCostRate 0.0099 → 0.0079 ⇒ netPerUsd rises ⇒ the amount required to
    // reach a $50 net floor DROPS (default ≈ $1661, override ≈ $1557).
    const defCfg = baseCfg();
    const defInv = inverseSize(
      baseRow(), defCfg,
      { net_usd: 50, roi_pct: null, source: "strategy_config" },
      forwardSimulate(baseRow(), defCfg)!,
    )!;
    const ovrInv = inverseSize(
      baseRow(), cfg,
      { net_usd: 50, roi_pct: null, source: "strategy_config" },
      r,
    )!;
    expect(ovrInv.required_amount_in_usd).toBeLessThan(defInv.required_amount_in_usd);
    expect(ovrInv.required_amount_in_usd).toBeGreaterThan(1500);
    expect(ovrInv.required_amount_in_usd).toBeLessThan(1650);
  });
});

// ── resolveTarget ───────────────────────────────────────────────────────────

describe("resolveTarget", () => {
  it("priority 1: strategy_configs.min_profit_usd wins when set > 0", () => {
    const cfg = baseCfg();
    cfg.strategy_configs = {
      dex_arb: { enabled: true, min_profit_usd: 50, min_roi_pct: null },
    };
    cfg.simulation_target_profit_usd = 200;  // would lose
    const t = resolveTarget(cfg, "dex_arb");
    expect(t).toEqual({ net_usd: 50, roi_pct: null, source: "strategy_config" });
  });

  it("priority 1: surfaces both USD and ROI floors when both set", () => {
    const cfg = baseCfg();
    cfg.strategy_configs = {
      dex_arb: { enabled: true, min_profit_usd: 10, min_roi_pct: 2 },
    };
    const t = resolveTarget(cfg, "dex_arb");
    expect(t).toEqual({ net_usd: 10, roi_pct: 2, source: "strategy_config" });
  });

  it("priority 1: ROI-only floor (no USD) surfaces correctly", () => {
    const cfg = baseCfg();
    cfg.strategy_configs = {
      dex_arb: { enabled: true, min_profit_usd: null, min_roi_pct: 2 },
    };
    const t = resolveTarget(cfg, "dex_arb");
    expect(t).toEqual({ net_usd: null, roi_pct: 2, source: "strategy_config" });
  });

  it("priority 2: simulation tab when strategy override is null/zero on both", () => {
    const cfg = baseCfg();
    cfg.strategy_configs = {
      dex_arb: { enabled: true, min_profit_usd: 0, min_roi_pct: null },
    };
    cfg.simulation_target_profit_usd = 200;
    cfg.simulation_target_roi_pct = 3;
    const t = resolveTarget(cfg, "dex_arb");
    expect(t).toEqual({ net_usd: 200, roi_pct: 3, source: "simulation_tab" });
  });

  it("returns null when neither floor is set anywhere", () => {
    const cfg = baseCfg();
    expect(resolveTarget(cfg, "dex_arb")).toBeNull();
  });

  it("matches strategy_kind case-insensitively", () => {
    const cfg = baseCfg();
    cfg.strategy_configs = {
      DEX_ARB: { enabled: true, min_profit_usd: 99, min_roi_pct: null },
    };
    const t = resolveTarget(cfg, "dex_arb");
    expect(t).toEqual({ net_usd: 99, roi_pct: null, source: "strategy_config" });
  });
});

// ── inverseSize ─────────────────────────────────────────────────────────────

describe("inverseSize", () => {
  it("USD-only floor: solves target $50 net within cap, suggests scaled amount", () => {
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    const row = baseRow();  // 1 WETH = $2350 amount_in, gross $100
    const fwd = forwardSimulate(row, cfg)!;
    // grossPerUsd ≈ 100/2350 = 0.04255
    // varCostRate = 0.0009 + 0.005 + 0.001 + 0.003 + 0 = 0.0099
    // p_copied=0, relayRate=0.05 (mainnet)
    // effectiveGross = 0.04255 × (1 − 0 − 0.05) = 0.0404
    // netPerUsd = 0.0404 − 0.0099 = 0.0305
    // fixedCosts = gas 0.17625 + ops 0.01 = 0.18625; +0.5 relay floor = 0.68625
    // required = (50 + 0.686) / 0.0305 ≈ $1661
    const inv = inverseSize(row, cfg, { net_usd: 50, roi_pct: null, source: "strategy_config" }, fwd)!;
    expect(inv).not.toBeNull();
    expect(inv.target_net_usd).toBe(50);
    expect(inv.target_roi_pct).toBeNull();
    expect(inv.target_source).toBe("strategy_config");
    expect(inv.binding_floor).toBe("usd-floor");
    expect(inv.estimation_basis).toBe("observed-gross");
    expect(inv.required_amount_in_usd).toBeGreaterThan(1500);
    expect(inv.required_amount_in_usd).toBeLessThan(1800);
    expect(inv.meets_target_at_cap).toBe(true);
    expect(inv.suggested_amount_in_usd).toBe(inv.required_amount_in_usd);
    expect(inv.notes).toContain("linear-extrap");
  });

  it("AND semantics: USD floor binds when route is highly profitable (r ≫ T_roi)", () => {
    // High r=3.05% net-per-usd from the canonical scenario, T_roi=2%.
    // req_usd ≈ 1661, req_roi = 0.686 / (0.0305 - 0.02) ≈ 65. USD floor binds (1661 > 65).
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    const inv = inverseSize(row, cfg, { net_usd: 50, roi_pct: 2, source: "strategy_config" }, fwd)!;
    expect(inv.binding_floor).toBe("usd-floor");
    expect(inv.required_amount_in_usd).toBeGreaterThan(1500);
    expect(inv.required_amount_in_usd).toBeLessThan(1800);
  });

  it("AND semantics: ROI floor binds when route barely clears T_roi", () => {
    // Force a route with thin margins so ROI floor matters. Bump slippage so
    // effective net rate is small (~0.5%) and T_roi=0.4% sits just below it.
    const cfg = baseCfg();
    cfg.capital_usd = 100_000_000;
    cfg.max_slippage_pct = 0.034;  // raises var cost rate to ~3.7%
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    // grossPerUsd 0.04255 × 0.95 = 0.0404. netPerUsd = 0.0404 - 0.0379 = 0.0025
    // T_roi 0.001 (0.1%) → req_roi = 0.686 / (0.0025 - 0.001) ≈ $457
    // T_usd $0.50 → req_usd = (0.50 + 0.686) / 0.0025 ≈ $474 → USD just edges out
    // Make T_usd tiny so ROI dominates:
    const inv = inverseSize(row, cfg, { net_usd: 0.1, roi_pct: 0.1, source: "strategy_config" }, fwd)!;
    expect(inv.binding_floor).toBe("roi-floor");
    expect(inv.required_amount_in_usd).toBeGreaterThan(400);
  });

  it("AND semantics: roi-unreachable when r ≤ T_roi", () => {
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    // netPerUsd ≈ 3.05%, ask for 10% ROI floor → unreachable.
    const inv = inverseSize(row, cfg, { net_usd: 10, roi_pct: 10, source: "strategy_config" }, fwd)!;
    expect(inv.binding_floor).toBe("roi-unreachable");
    expect(inv.required_amount_in_usd).toBe(Infinity);
    expect(inv.suggested_amount_in_usd).toBe(0);
    expect(inv.meets_target_at_cap).toBe(false);
    expect(inv.notes).toContain("roi-unreachable");
  });

  it("flags cap-bound when required > effective capital", () => {
    const cfg = baseCfg();
    cfg.capital_usd = 1_000;  // small cap
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    const inv = inverseSize(row, cfg, { net_usd: 50, roi_pct: null, source: "strategy_config" }, fwd)!;
    // required ≈ $1661 > cap $1000 → cap binds
    expect(inv.meets_target_at_cap).toBe(false);
    expect(inv.suggested_amount_in_usd).toBe(1000);
    expect(inv.notes).toContain("cap-bound");
  });

  it("honours per-strategy cap from simulation tab", () => {
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    cfg.simulation_per_strategy_caps_usd = { dex_arb: 500 };
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    const inv = inverseSize(row, cfg, { net_usd: 50, roi_pct: null, source: "simulation_tab" }, fwd)!;
    expect(inv.cap_amount_in_usd).toBe(500);
    expect(inv.suggested_amount_in_usd).toBe(500);
    expect(inv.meets_target_at_cap).toBe(false);
  });

  it("returns net-per-usd-nonpositive when costs exceed gross", () => {
    const cfg = baseCfg();
    cfg.max_slippage_pct = 0.5;  // 50% slippage — destroys the spread
    const row = baseRow();
    const fwd = forwardSimulate(row, cfg)!;
    const inv = inverseSize(row, cfg, { net_usd: 50, roi_pct: null, source: "strategy_config" }, fwd)!;
    expect(inv.binding_floor).toBe("net-per-usd-nonpositive");
    expect(inv.notes).toContain("net-per-usd-nonpositive");
    expect(inv.required_amount_in_usd).toBe(Infinity);
    expect(inv.meets_target_at_cap).toBe(false);
    expect(inv.suggested_amount_in_usd).toBe(0);
  });

  it("ROI-assumed (Path B): no forward → uses operator ROI as assumed rate, USD floor binds", () => {
    // No forward (worker rejected before gross math). Operator config has
    // min_profit_usd=10 + min_roi_pct=2%. Estimation_basis must be
    // "roi-assumed". Path B only enforces the USD floor (ROI floor assumed
    // by the spine's gate that surfaced the opp).
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    const row = baseRow();
    const inv = inverseSize(row, cfg, { net_usd: 10, roi_pct: 2, source: "strategy_config" }, null)!;
    expect(inv).not.toBeNull();
    expect(inv.estimation_basis).toBe("roi-assumed");
    expect(inv.notes).toContain("roi-based-estimate");
    expect(inv.notes).toContain("roi-floor-assumed-by-spine-gate");
    // Math: assumedGross=0.02, effective×0.95=0.019, var=0.0099 → netPerUsd=0.0091.
    // fixed=gas $0.17625 + ops $0.01 + relay floor $0.50 = $0.68625.
    // req_usd = (10 + 0.686)/0.0091 ≈ $1175. USD floor binds (only floor enforced).
    expect(inv.binding_floor).toBe("usd-floor");
    expect(inv.required_amount_in_usd).toBeGreaterThan(1100);
    expect(inv.required_amount_in_usd).toBeLessThan(1250);
  });

  it("ROI-assumed (Path B): net-per-usd-nonpositive when assumed ROI doesn't cover var costs", () => {
    // assumedGross = 0.5% (0.005), effective×0.95=0.00475, var=0.0099
    // → netPerUsd = -0.00515. Honest: route at this assumed ROI loses money.
    const cfg = baseCfg();
    cfg.capital_usd = 100_000;
    const row = baseRow();
    const inv = inverseSize(row, cfg, { net_usd: 10, roi_pct: 0.5, source: "strategy_config" }, null)!;
    expect(inv.estimation_basis).toBe("roi-assumed");
    expect(inv.binding_floor).toBe("net-per-usd-nonpositive");
    expect(inv.suggested_amount_in_usd).toBe(0);
  });

  it("ROI-assumed (Path B): returns null when no forward AND no roi_pct (cannot extrapolate)", () => {
    const cfg = baseCfg();
    const row = baseRow();
    // USD-only target with no forward → no way to assume a rate → null per R8.
    const inv = inverseSize(row, cfg, { net_usd: 10, roi_pct: null, source: "strategy_config" }, null);
    expect(inv).toBeNull();
  });
});
