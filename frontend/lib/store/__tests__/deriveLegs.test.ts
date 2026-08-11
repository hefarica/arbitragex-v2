// frontend/lib/store/__tests__/deriveLegs.test.ts
//
// Regression tests for the multi-leg route ViewModel logic. These lock the
// fidelity contract: the route_metadata round-trip (Rust RouteMetadata → JSONB
// → API → TS RouteMetadataWire → deriveLegs) must preserve the exact token
// traversal order, and the legacy null path must fall back honestly (R8).
//
// RULE 00: addresses used here are canonical mainnet protocol constants
// (WETH/USDC/DAI), NOT fabricated operator data. No mocks of the unit under test.
import { describe, it, expect } from "vitest";
import {
  parseRouteMetadata,
  deriveLegs,
  mapToOmniOpportunity,
} from "@/lib/store/types";

// Canonical mainnet tokens (protocol constants, RULE 00 exception).
const WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const DAI = "0x6b175474e89094c44da98b954eedeac495271d0f";

describe("parseRouteMetadata", () => {
  it("returns null for absent / non-object / empty {}", () => {
    expect(parseRouteMetadata(null)).toBeNull();
    expect(parseRouteMetadata(undefined)).toBeNull();
    expect(parseRouteMetadata("nope")).toBeNull();
    expect(parseRouteMetadata({})).toBeNull();
  });

  it("returns null when dex_adapters empty or token_addresses < 2", () => {
    expect(
      parseRouteMetadata({ dex_adapters: [], token_addresses: [WETH], pool_addresses: [] }),
    ).toBeNull();
    expect(
      parseRouteMetadata({ dex_adapters: ["uniswap_v2_router"], token_addresses: [WETH], pool_addresses: ["0x1"] }),
    ).toBeNull();
  });

  it("parses a 3-leg triangular topology verbatim (token order preserved)", () => {
    const rm = parseRouteMetadata({
      token_addresses: [WETH, USDC, DAI, WETH],
      pool_addresses: ["0xp1", "0xp2", "0xp3"],
      dex_adapters: ["uniswap_v2_router", "uniswap_v2_router", "uniswap_v2_router"],
    });
    expect(rm).not.toBeNull();
    expect(rm!.token_addresses).toEqual([WETH, USDC, DAI, WETH]);
    expect(rm!.dex_adapters).toHaveLength(3);
    expect(rm!.pool_addresses).toHaveLength(3);
  });
});

describe("deriveLegs", () => {
  it("derives N legs from route_metadata in traversal order (2..N)", () => {
    const opp = mapToOmniOpportunity({
      id: "a",
      chain_id: 1,
      strategy_kind: "triangular",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      dex_b: null,
      token_in: WETH,
      token_out: WETH,
      route_metadata: {
        token_addresses: [WETH, USDC, DAI, WETH],
        pool_addresses: ["0xp1", "0xp2", "0xp3"],
        dex_adapters: ["uniswap_v2_router", "uniswap_v2_router", "uniswap_v2_router"],
      },
    } as Record<string, unknown>);
    const legs = deriveLegs(opp);
    expect(legs).toHaveLength(3);
    const [leg0, , leg2] = legs;
    expect(leg0!.token_in).toBe(WETH);
    expect(leg0!.token_out).toBe(USDC);
    expect(leg2!.token_out).toBe(WETH); // closes the cycle
  });

  it("falls back to a synthetic 2-leg BUY/SELL when route_metadata is null (R8 honest)", () => {
    const opp = mapToOmniOpportunity({
      id: "b",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      dex_b: "sushiswap",
      token_in: WETH,
      token_out: USDC,
    } as Record<string, unknown>);
    const legs = deriveLegs(opp);
    expect(legs).toHaveLength(2);
    const [leg0, leg1] = legs;
    expect(leg0!.dex).toBe("uniswap-v2");
    expect(leg1!.dex).toBe("sushiswap");
  });

  it("returns [] only when there is genuinely no route (no dex_a, no dex_b)", () => {
    const opp = mapToOmniOpportunity({
      id: "c",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "",
      dex_b: null,
      token_in: WETH,
      token_out: USDC,
    } as Record<string, unknown>);
    expect(deriveLegs(opp)).toEqual([]);
  });
});
