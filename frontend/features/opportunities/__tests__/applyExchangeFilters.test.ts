// frontend/features/opportunities/__tests__/applyExchangeFilters.test.ts
//
// Coverage for the exchange filter pure function: family (via familyOf), chain,
// cartridge substring, viable status, min-yield floor. DEFAULT_FILTERS passes
// all (no narrowing).
import { describe, it, expect } from "vitest";
import {
  applyExchangeFilters,
  DEFAULT_FILTERS,
} from "@/components/opportunities/exchange/ExchangeFilterBar";
import {
  mapToOmniOpportunity,
  type OmniOpportunity,
} from "@/lib/store/types";

const mk = (over: Record<string, unknown>): OmniOpportunity =>
  mapToOmniOpportunity({
    id: "id",
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-08-11T00:00:00Z",
    trace_id: "t",
    dex_a: "uniswap-v2",
    dex_b: "sushiswap",
    token_in: "0xa",
    token_out: "0xb",
    ...over,
  });

describe("applyExchangeFilters", () => {
  const opps = [
    mk({ id: "1", strategy_kind: "dex_arb", chain_id: 1, net_expected_profit_usd: 5 }),
    mk({
      id: "2",
      strategy_kind: "mev_01_001_dex_dex_arbitrage",
      chain_id: 42161,
      net_expected_profit_usd: 1,
      status: "rejected",
    }),
    mk({ id: "3", strategy_kind: "triangular", chain_id: 1, net_expected_profit_usd: 20 }),
  ];

  it("DEFAULT_FILTERS returns all (no narrowing)", () => {
    expect(applyExchangeFilters(opps, DEFAULT_FILTERS)).toHaveLength(3);
  });

  it("min-yield floor filters out low-net opps", () => {
    expect(
      applyExchangeFilters(opps, { ...DEFAULT_FILTERS, minYieldUsd: 10 }),
    ).toHaveLength(1);
  });

  it("viable-only excludes rejected", () => {
    expect(
      applyExchangeFilters(opps, { ...DEFAULT_FILTERS, viableOnly: true }),
    ).toHaveLength(2);
  });

  it("chain filter narrows to one chain", () => {
    expect(
      applyExchangeFilters(opps, { ...DEFAULT_FILTERS, chainId: 42161 }),
    ).toHaveLength(1);
  });

  it("cartridge search matches substring of strategy_kind", () => {
    expect(
      applyExchangeFilters(opps, { ...DEFAULT_FILTERS, search: "mev_01" }),
    ).toHaveLength(1);
  });
});
