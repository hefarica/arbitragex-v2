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
  // FE-0029 (§28): fixtures declare status explicitly — the mapper no longer
  // fabricates "detected" for payloads that omit it.
  const opps = [
    mk({ id: "1", strategy_kind: "dex_arb", chain_id: 1, net_expected_profit_usd: 5, status: "detected" }),
    mk({
      id: "2",
      strategy_kind: "mev_01_001_dex_dex_arbitrage",
      chain_id: 42161,
      net_expected_profit_usd: 1,
      status: "rejected",
    }),
    mk({ id: "3", strategy_kind: "triangular", chain_id: 1, net_expected_profit_usd: 20, status: "detected" }),
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

  it("FE-0029 fail-safe: viableOnly cannot assert viability for an unstatused row", () => {
    const unstatused = [mk({ id: "4", status: undefined })]; // malformed payload → status null
    expect(unstatused[0]!.status).toBeNull();
    expect(
      applyExchangeFilters(unstatused, { ...DEFAULT_FILTERS, viableOnly: true }),
    ).toHaveLength(0);
    // But it stays visible without the viability assertion (honest, not hidden).
    expect(
      applyExchangeFilters(unstatused, DEFAULT_FILTERS),
    ).toHaveLength(1);
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

  // Hops filter (operator order 2026-09-20). hop_count null = legacy row
  // without route_metadata — it belongs to NO hop bucket (R8 fail-honest).
  // hop_count derives from route_metadata (deriveHopCount), so fixtures carry
  // topology instead of a raw hop_count field.
  const rm2 = {
    token_addresses: ["0xa", "0xb", "0xc"],
    pool_addresses: ["0xp1", "0xp2"],
    dex_adapters: ["uniswap_v2", "sushiswap"],
  };
  const rm3 = {
    token_addresses: ["0xa", "0xb", "0xc", "0xd"],
    pool_addresses: ["0xp1", "0xp2", "0xp3"],
    dex_adapters: ["uniswap_v2", "sushiswap", "curve"],
  };
  const hopOpps = [
    mk({ id: "h1", route_metadata: rm2, status: "detected" }),
    mk({ id: "h2", route_metadata: rm3, status: "detected" }),
    mk({ id: "h3", route_metadata: null, status: "detected" }), // legacy row
  ];

  it("hops=2 keeps only hop_count 2", () => {
    const out = applyExchangeFilters(hopOpps, { ...DEFAULT_FILTERS, hops: 2 });
    expect(out.map((o) => o.id)).toEqual(["h1"]);
  });

  it("hops filter drops mismatched and null hop_count rows", () => {
    const out = applyExchangeFilters(hopOpps, { ...DEFAULT_FILTERS, hops: 3 });
    expect(out.map((o) => o.id)).toEqual(["h2"]);
  });

  it('hops="all" keeps every row including null hop_count (legacy)', () => {
    expect(
      applyExchangeFilters(hopOpps, { ...DEFAULT_FILTERS, hops: "all" }),
    ).toHaveLength(3);
    expect(DEFAULT_FILTERS.hops).toBe("all");
  });
});
