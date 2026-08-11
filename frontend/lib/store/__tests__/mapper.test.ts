// frontend/lib/store/__tests__/mapper.test.ts
//
// Regression tests for mapToOmniOpportunity: number coercion is exact (never
// NaN/fabricated) and route_metadata parses when present, null when empty (R8).
import { describe, it, expect } from "vitest";
import { mapToOmniOpportunity } from "@/lib/store/types";

describe("mapToOmniOpportunity — R8 fail-honest + route_metadata", () => {
  it("coerces numbers and leaves unknowns null (never NaN, never fabricated)", () => {
    const o = mapToOmniOpportunity({
      id: "x",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      dex_b: null,
      token_in: "0xa",
      token_out: "0xb",
      expected_profit_usd: "12.5",
      net_expected_profit_usd: null,
      roi_pct: 2.3,
    } as Record<string, unknown>);
    expect(o.expected_profit_usd).toBe(12.5); // string → number
    expect(o.net_expected_profit_usd).toBeNull();
    expect(o.roi_pct).toBe(2.3);
    expect(o.route_metadata).toBeNull();
  });

  it("parses route_metadata when present and well-formed", () => {
    const o = mapToOmniOpportunity({
      id: "y",
      chain_id: 1,
      strategy_kind: "triangular",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      dex_b: null,
      token_in: "0xa",
      token_out: "0xa",
      route_metadata: {
        token_addresses: ["0xa", "0xb", "0xc", "0xa"],
        pool_addresses: ["0xp1", "0xp2", "0xp3"],
        dex_adapters: ["uniswap_v2_router", "uniswap_v2_router", "uniswap_v2_router"],
      },
    } as Record<string, unknown>);
    expect(o.route_metadata).not.toBeNull();
    expect(o.route_metadata!.token_addresses).toHaveLength(4);
  });

  it("route_metadata null when API sends empty {}", () => {
    const o = mapToOmniOpportunity({
      id: "z",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      dex_b: null,
      token_in: "0xa",
      token_out: "0xb",
      route_metadata: {},
    } as Record<string, unknown>);
    expect(o.route_metadata).toBeNull();
  });
});
