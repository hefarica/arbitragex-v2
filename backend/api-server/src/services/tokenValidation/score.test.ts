/**
 * Unit tests for the FinalTokenScore composer.
 *
 * Pins the status taxonomy and scoring weights against concrete scenarios
 * drawn from the live feed on 2026-05-11:
 *
 *   - WETH ground truth: registry-verified, deep liquidity → VERIFIED.
 *   - SCFN/IVQ pattern: in DEX Screener but $0.01 liquidity + wash-trade
 *     volume → ILLIQUID with wash-trade penalty.
 *   - 69420 pattern: not in registry, no DEX Screener data → NO_DATA.
 *   - Impersonation: address-not-in-registry but symbol claims to be USDC
 *     → unverified + low score.
 *   - Non-ERC20: bytecode missing or doesn't respond to ABI → INVALID.
 *   - PENDING: both validators failed to run.
 */

import { describe, it, expect } from "vitest";
import { composeFinalScore } from "./score.js";
import type { OnChainTruthResult } from "./onChainTruth.js";
import type { LiquidityRealityResult } from "./liquidityReality.js";
import type { VerificationResult } from "../tokenRegistry.js";

function onChainOk(over: Partial<OnChainTruthResult> = {}): OnChainTruthResult {
  return {
    ran: true,
    error: false,
    error_message: null,
    exists_onchain: true,
    is_contract: true,
    bytecode_size: 24576,
    standard: "ERC20",
    onchain_name: "Mock Token",
    onchain_symbol: "MOCK",
    onchain_decimals: 18,
    total_supply_str: "1000000000000000000000000",
    ...over,
  };
}

function liquidityWeth(over: Partial<LiquidityRealityResult> = {}): LiquidityRealityResult {
  return {
    ran: true,
    error: false,
    error_message: null,
    no_data: false,
    pair_count: 30,
    liquidity_usd: 2_500_000,
    volume_24h_usd: 4_000_000,
    volume_1h_usd: 200_000,
    volume_5m_usd: 15_000,
    price_usd: 2350,
    primary_dex: "uniswap-v3",
    primary_pair_address: "0x" + "0".repeat(40),
    ...over,
  };
}

function liquidityScam(over: Partial<LiquidityRealityResult> = {}): LiquidityRealityResult {
  // The SCFN/IVQ pattern: pair exists, $0.01 liquidity, $4k wash volume.
  return {
    ran: true,
    error: false,
    error_message: null,
    no_data: false,
    pair_count: 1,
    liquidity_usd: 0.01,
    volume_24h_usd: 4446,
    volume_1h_usd: 200,
    volume_5m_usd: 5,
    price_usd: 0.000000000004,
    primary_dex: "uniswap-v2",
    primary_pair_address: "0x" + "1".repeat(40),
    ...over,
  };
}

function liquidityNoData(): LiquidityRealityResult {
  return {
    ran: true,
    error: false,
    error_message: null,
    no_data: true,
    pair_count: 0,
    liquidity_usd: null,
    volume_24h_usd: null,
    volume_1h_usd: null,
    volume_5m_usd: null,
    price_usd: null,
    primary_dex: null,
    primary_pair_address: null,
  };
}

function liquiditySkipped(): LiquidityRealityResult {
  return {
    ran: false,
    error: true,
    error_message: "unsupported chain",
    no_data: false,
    pair_count: 0,
    liquidity_usd: null,
    volume_24h_usd: null,
    volume_1h_usd: null,
    volume_5m_usd: null,
    price_usd: null,
    primary_dex: null,
    primary_pair_address: null,
  };
}

function registryVerified(symbol = "WETH"): VerificationResult {
  return {
    verified: true,
    registry_symbol: symbol,
    registry_name: "Wrapped Ether",
    registry_logo_uri: null,
    source: "uniswap",
    notes: ["in-registry", "via-uniswap"],
  };
}

function registryUnknown(): VerificationResult {
  return {
    verified: false,
    registry_symbol: null,
    registry_name: null,
    registry_logo_uri: null,
    source: null,
    notes: ["address-not-in-registry"],
  };
}

function registryMismatch(): VerificationResult {
  return {
    verified: false,
    registry_symbol: "USDC",
    registry_name: "USD Coin",
    registry_logo_uri: null,
    source: "uniswap",
    notes: ["in-registry", "via-uniswap", "symbol-mismatch"],
  };
}

// ── Tests ───────────────────────────────────────────────────────────────────

describe("composeFinalScore", () => {
  it("VERIFIED + high score for registry-verified, deep-liquidity tokens (WETH)", () => {
    const r = composeFinalScore(onChainOk(), liquidityWeth(), registryVerified("WETH"));
    expect(r.status).toBe("VERIFIED");
    expect(r.score).toBeGreaterThanOrEqual(85);
    // Breakdown should mention key contributing signals.
    const keys = r.reasons.map((x) => x.key);
    expect(keys).toContain("exists-onchain");
    expect(keys).toContain("erc20-abi");
    expect(keys).toContain("registry-verified");
    expect(keys).toContain("liquidity-viable");
  });

  it("ILLIQUID + wash-trade penalty for SCFN/IVQ pattern", () => {
    const r = composeFinalScore(onChainOk(), liquidityScam(), registryUnknown());
    expect(r.status).toBe("ILLIQUID");
    expect(r.score).toBeLessThanOrEqual(35);
    const keys = r.reasons.map((x) => x.key);
    expect(keys).toContain("liquidity-illiquid");
    expect(keys).toContain("wash-trade-signature");
    // No registry verification, no volume bonus.
    expect(keys).not.toContain("registry-verified");
  });

  it("VIABLE for non-registry tokens with real liquidity and volume", () => {
    const liq = liquidityWeth({ pair_count: 4, liquidity_usd: 50_000, volume_24h_usd: 5_000 });
    const r = composeFinalScore(onChainOk(), liq, registryUnknown());
    expect(r.status).toBe("VIABLE");
    expect(r.score).toBeGreaterThanOrEqual(45);
    expect(r.score).toBeLessThanOrEqual(70);
    const keys = r.reasons.map((x) => x.key);
    expect(keys).toContain("liquidity-viable");
    expect(keys).toContain("volume-active");
    expect(keys).not.toContain("registry-verified");
  });

  it("LOW_LIQUIDITY for thin-but-positive liquidity", () => {
    const liq = liquidityWeth({ pair_count: 1, liquidity_usd: 1_000, volume_24h_usd: 50 });
    const r = composeFinalScore(onChainOk(), liq, registryUnknown());
    expect(r.status).toBe("LOW_LIQUIDITY");
    expect(r.score).toBeLessThanOrEqual(50);
    expect(r.reasons.map((x) => x.key)).toContain("liquidity-low");
  });

  it("NO_DATA when DEX Screener returns no pairs but contract is valid ERC-20", () => {
    const r = composeFinalScore(onChainOk(), liquidityNoData(), registryUnknown());
    expect(r.status).toBe("NO_DATA");
    expect(r.reasons.map((x) => x.key)).toContain("no-dex-data");
  });

  it("INVALID — bytecode missing means it's an EOA, not a contract", () => {
    const oc = onChainOk({
      exists_onchain: false,
      is_contract: false,
      bytecode_size: 0,
      standard: "unknown",
      onchain_name: null,
      onchain_symbol: null,
      onchain_decimals: null,
      total_supply_str: null,
    });
    const r = composeFinalScore(oc, liquidityNoData(), registryUnknown());
    expect(r.status).toBe("INVALID");
    expect(r.score).toBe(0);
    expect(r.reasons[0]!.key).toBe("not-a-contract");
  });

  it("INVALID — bytecode exists but doesn't implement ERC-20 ABI", () => {
    const oc = onChainOk({
      standard: "unknown",
      onchain_name: null,        // missing name() → not ERC-20
    });
    const r = composeFinalScore(oc, liquidityWeth(), registryUnknown());
    expect(r.status).toBe("INVALID");
    expect(r.score).toBe(0);
    expect(r.reasons[0]!.key).toBe("not-erc20");
  });

  it("Impersonation penalty: symbol-mismatch flagged with -25 delta", () => {
    // Random address that CLAIMS to be USDC. Registry has the real USDC at
    // a different address; registry returns verified=false with mismatch.
    const r = composeFinalScore(
      onChainOk({ onchain_symbol: "USDC" }),
      liquidityWeth(),
      registryMismatch(),
    );
    const symbolMismatchReason = r.reasons.find((x) => x.key === "symbol-mismatch");
    expect(symbolMismatchReason).toBeDefined();
    expect(symbolMismatchReason!.delta).toBe(-25);
    // Status is NOT VERIFIED — impersonation never gets the green badge.
    expect(r.status).not.toBe("VERIFIED");
  });

  it("PENDING when both validators failed to run", () => {
    const oc: OnChainTruthResult = {
      ran: false,
      error: true,
      error_message: "no RPC",
      exists_onchain: false,
      is_contract: false,
      bytecode_size: null,
      standard: "unknown",
      onchain_name: null,
      onchain_symbol: null,
      onchain_decimals: null,
      total_supply_str: null,
    };
    const r = composeFinalScore(oc, liquiditySkipped(), registryUnknown());
    expect(r.status).toBe("PENDING");
    expect(r.score).toBe(0);
  });

  it("Score clamped to [0, 100] under any combination", () => {
    // Maximum-everything scenario.
    const r = composeFinalScore(
      onChainOk(),
      liquidityWeth({ pair_count: 100, liquidity_usd: 1e9, volume_24h_usd: 1e9 }),
      registryVerified(),
    );
    expect(r.score).toBeGreaterThanOrEqual(0);
    expect(r.score).toBeLessThanOrEqual(100);
  });

  it("Wash-trade penalty scales with ratio severity", () => {
    // ratio = 100k → very high penalty.
    const liq = liquidityScam({ liquidity_usd: 0.01, volume_24h_usd: 1000 });
    const r1 = composeFinalScore(onChainOk(), liq, registryUnknown());
    const washReason1 = r1.reasons.find((x) => x.key === "wash-trade-signature");
    expect(washReason1).toBeDefined();
    expect(washReason1!.delta).toBeLessThan(0);

    // Healthy ratio: $1k volume / $1k liquidity = 1× → no penalty, +5 bonus.
    const liq2 = liquidityScam({ liquidity_usd: 1000, volume_24h_usd: 1000, pair_count: 1 });
    const r2 = composeFinalScore(onChainOk(), liq2, registryUnknown());
    const healthy = r2.reasons.find((x) => x.key === "healthy-vol-liq-ratio");
    expect(healthy).toBeDefined();
    expect(healthy!.delta).toBeGreaterThan(0);
  });
});
