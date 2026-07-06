import { describe, it, expect } from "vitest";
import { OpportunityListItemSchema, TokenInfoSchema } from "../api-contracts.js";

describe("TokenInfoSchema", () => {
  it("accepts fully resolved token", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: "WETH", decimals: 18,
      logo_url: "https://raw.githubusercontent.com/.../logo.png",
      resolved_via: "onchain_full",
    })).not.toThrow();
  });

  it("accepts all-null TokenInfo (resolved_via=failed)", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: null, decimals: null, logo_url: null, resolved_via: "failed",
    })).not.toThrow();
  });

  it("rejects invalid resolved_via", () => {
    expect(() => TokenInfoSchema.parse({
      symbol: "X", decimals: 18, logo_url: null, resolved_via: "guessed",
    })).toThrow();
  });
});

describe("OpportunityListItemSchema", () => {
  const base = {
    id: "11111111-1111-1111-1111-111111111111",
    chain_id: 1, strategy_kind: "dex_arb", dex_a: "uniswap-v2", dex_b: null,
    pair_symbol: "x/y",
    token_in: "0x" + "a".repeat(40),  token_in_info: null,
    token_out: "0x" + "b".repeat(40), token_out_info: null,
    amount_in_wei: "1000",
    expected_profit_usd: null, roi_pct: null, risk_score: null,
    block_number: null, rejection_reason: null, status: "detected" as const,
    detected_at: "2026-05-06T00:00:00Z", trace_id: "22222222-2222-2222-2222-222222222222",
    chain_id_out: null, bridge: null, bridge_fee_usd: null,
  };

  it("accepts a fail-honest item with all NULL profit", () => {
    expect(() => OpportunityListItemSchema.parse(base)).not.toThrow();
  });

  it("accepts a simulated item with profit=0 (real value, not pending)", () => {
    expect(() => OpportunityListItemSchema.parse({
      ...base, status: "simulated", expected_profit_usd: 0,
    })).not.toThrow();
  });

  it("accepts cross-chain item with chain_id_out + bridge filled", () => {
    expect(() => OpportunityListItemSchema.parse({
      ...base, chain_id_out: 42161, bridge: "across", bridge_fee_usd: 0.50,
    })).not.toThrow();
  });

  it("rejects invalid status", () => {
    expect(() => OpportunityListItemSchema.parse({ ...base, status: "magic" })).toThrow();
  });
});
