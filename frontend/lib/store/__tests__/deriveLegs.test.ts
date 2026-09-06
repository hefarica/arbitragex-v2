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
  deriveHopCount,
  deriveLegLedger,
  mapToOmniOpportunity,
  SYNTHETIC_LEGACY_VIEW_LABEL,
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

  // ─── HOPS-LEDGER-04: the optional per-leg wei arrays ───
  it("projects the leg ledger arrays when present; undefined (NOT []) when absent (R8)", () => {
    const withLedger = parseRouteMetadata({
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
      leg_amounts_in: ["1000", "990"],
      leg_amounts_out: ["990", "1010"],
      leg_zero_for_one: [true, false],
    });
    expect(withLedger!.leg_amounts_in).toEqual(["1000", "990"]);
    expect(withLedger!.leg_amounts_out).toEqual(["990", "1010"]);
    expect(withLedger!.leg_zero_for_one).toEqual([true, false]);

    const withoutLedger = parseRouteMetadata({
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
    });
    // Absence is the STATE "not computed" — never an empty array (R8).
    expect(withoutLedger!.leg_amounts_in).toBeUndefined();
    expect(withoutLedger!.leg_amounts_out).toBeUndefined();
    expect(withoutLedger!.leg_zero_for_one).toBeUndefined();
  });

  it("filters non-string/non-boolean junk out of the ledger arrays (type-gate)", () => {
    const rm = parseRouteMetadata({
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
      leg_amounts_in: ["1000", 42, null],
      leg_amounts_out: ["990", "1010"],
      leg_zero_for_one: [true, "yes"],
    });
    expect(rm!.leg_amounts_in).toEqual(["1000"]);
    expect(rm!.leg_amounts_out).toEqual(["990", "1010"]);
    expect(rm!.leg_zero_for_one).toEqual([true]);
  });

  it("unwraps the REAL DecimalsMap wire shape {\"map\":{…}} (flat tolerated, junk dropped)", () => {
    // The Rust side serializes DecimalsMap as a newtype — {"map": {...}} is
    // what actually arrives on the wire (see prod fixtures). A flat record is
    // tolerated for legacy/tests; non-number values drop, never coerce.
    const nested = parseRouteMetadata({
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
      decimals: { map: { [WETH]: 18, [USDC]: "6" } },
    });
    expect(nested!.decimals).toEqual({ [WETH]: 18 });

    const flat = parseRouteMetadata({
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
      decimals: { [WETH]: 18 },
    });
    expect(flat!.decimals).toEqual({ [WETH]: 18 });
  });
});

// ─── HOPS-LEDGER-04 — per-leg wei ledger derivation ──────────────────────────

describe("deriveLegLedger (HOPS-LEDGER-04)", () => {
  const sizedWire = {
    id: "led",
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-08-11T00:00:00Z",
    trace_id: "t",
    dex_a: "uniswap-v2",
    dex_b: "sushiswap",
    token_in: WETH,
    token_out: WETH,
    route_metadata: {
      token_addresses: [WETH, USDC, WETH],
      pool_addresses: ["0xp1", "0xp2"],
      dex_adapters: ["uniswap_v2_router", "sushiswap"],
      leg_amounts_in: ["1000000000000000000", "990000000000000000"],
      leg_amounts_out: ["990000000000000000", "1010000000000000000"],
      leg_zero_for_one: [true, false],
    },
  } as Record<string, unknown>;

  it("chains legs (leg1 in === leg0 out); delta ONLY on the closing leg", () => {
    const opp = mapToOmniOpportunity(sizedWire);
    const ledger = deriveLegLedger(opp);
    expect(ledger).not.toBeNull();
    expect(ledger).toHaveLength(2);
    const [l0, l1] = ledger!;
    expect(l0!.amount_in_wei).toBe("1000000000000000000");
    expect(l0!.amount_out_wei).toBe("990000000000000000");
    expect(l0!.cycle_delta_wei).toBeNull(); // intermediate hop: no delta (R8)
    expect(l1!.amount_in_wei).toBe(l0!.amount_out_wei); // the ledger chains
    expect(l1!.zero_for_one).toBe(false);
    // closed cycle (WETH→USDC→WETH): final − initial = +10e15 wei, exact
    expect(l1!.cycle_delta_wei).toBe("10000000000000000");
  });

  it("negative closed-cycle delta keeps its sign (loss cycle)", () => {
    const rm = sizedWire.route_metadata as Record<string, unknown>;
    const losing = {
      ...rm,
      leg_amounts_out: ["990000000000000000", "900000000000000000"],
    };
    const opp = mapToOmniOpportunity({ ...sizedWire, route_metadata: losing });
    const ledger = deriveLegLedger(opp);
    expect(ledger![1]!.cycle_delta_wei).toBe("-100000000000000000");
  });

  it("returns null when any ledger array is absent (not-Sized / triangular — R8)", () => {
    const rm = { ...(sizedWire.route_metadata as Record<string, unknown>) };
    delete rm.leg_amounts_in;
    const opp = mapToOmniOpportunity({ ...sizedWire, route_metadata: rm });
    expect(deriveLegLedger(opp)).toBeNull();
  });

  it("returns null on length mismatch vs hops (all-or-nothing, mirrors Rust attach)", () => {
    const mismatched = {
      ...(sizedWire.route_metadata as Record<string, unknown>),
      leg_zero_for_one: [true], // 1 entry vs 2 hops
    };
    const opp = mapToOmniOpportunity({ ...sizedWire, route_metadata: mismatched });
    expect(deriveLegLedger(opp)).toBeNull();
  });

  it("no cycle delta when the topology doesn't close (A→B→C)", () => {
    const open = {
      ...(sizedWire.route_metadata as Record<string, unknown>),
      token_addresses: [WETH, USDC, DAI],
    };
    const opp = mapToOmniOpportunity({ ...sizedWire, route_metadata: open });
    const ledger = deriveLegLedger(opp);
    expect(ledger).not.toBeNull();
    expect(ledger!.every((e) => e.cycle_delta_wei === null)).toBe(true);
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

// ─── FE-0030 — §29 SYNTHETIC LEGACY VIEW marking ─────────────────────────────

describe("deriveLegs — synthetic legacy marking (FE-0030 §29)", () => {
  it("wire legs carry NO synthetic flag (persisted topology = ROUTE-grade)", () => {
    const opp = mapToOmniOpportunity({
      id: "a",
      chain_id: 1,
      strategy_kind: "triangular",
      detected_at: "2026-08-11T00:00:00Z",
      trace_id: "t",
      dex_a: "uniswap-v2",
      token_in: WETH,
      token_out: WETH,
      route_metadata: {
        token_addresses: [WETH, USDC, DAI, WETH],
        pool_addresses: ["0xp1", "0xp2", "0xp3"],
        dex_adapters: ["uniswap_v2_router", "sushiswap", "uniswap_v2_router"],
      },
    } as Record<string, unknown>);
    for (const leg of deriveLegs(opp)) {
      expect(leg.synthetic).toBeUndefined();
    }
  });

  it("synthetic fallback legs are marked synthetic: true (every leg)", () => {
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
    for (const leg of legs) {
      expect(leg.synthetic).toBe(true);
    }
  });

  it("a synthetic view NEVER implies operational hops: hop_count stays null (§29)", () => {
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
    // 2 view legs, but the wire hop_count is null — the synthetic legs are
    // display shape, not an operational HOPS=2 claim.
    expect(deriveLegs(opp)).toHaveLength(2);
    expect(opp.hop_count).toBeNull();
    expect(deriveHopCount(opp.route_metadata)).toBeNull();
  });

  it("the §29 marker is the canonical exported string", () => {
    expect(SYNTHETIC_LEGACY_VIEW_LABEL).toBe("SYNTHETIC LEGACY VIEW");
  });
});
