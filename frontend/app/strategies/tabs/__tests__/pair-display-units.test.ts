/**
 * FE-0043 (§73 unit tier) — PairIndex display units: `pairKey` (§12 deep-link
 * identity) and `feeList` (fee-tier presentation). These two pure functions
 * were previously exercised only through PairIntelligencePanel renders; this
 * suite pins their contracts directly (exported for test the same way
 * QuoteBasePanel exports `quoteWeightsKey`/`knobsToQuoteWeights`).
 *
 * Doctrine: payload values only — no math, no fabrication (RULE 00); empty
 * input renders the honest dash, never a zero (R8).
 */
import { describe, expect, it } from "vitest";

import type { PairView, PoolRef } from "@/lib/apex/schemas";

import { feeList, pairKey } from "../PairIntelligencePanel";

const A = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

function mkPool(fee_bps: number, venue = "uniswap_v2"): PoolRef {
  return {
    pool_address: "0x" + "c".repeat(40),
    venue,
    fee_bps,
    reserves_a: "1000",
    reserves_b: "2000",
  };
}

function mkPair(pools: PoolRef[], a = A, b = B): PairView {
  return {
    chain_id: 1,
    token_a: { chain_id: 1, address: a, symbol: "TKA", decimals: 18 },
    token_b: { chain_id: 1, address: b, symbol: "TKB", decimals: 18 },
    pools,
    venue_count: new Set(pools.map((p) => p.venue)).size,
    alpha_forward: null,
    alpha_reverse: null,
    dirty: false,
    last_reserve_update: null,
  };
}

describe("pairKey — §12 deep-link identity", () => {
  it("is the canonical `aAddr-bAddr` template, verbatim payload addresses", () => {
    expect(pairKey(mkPair([]))).toBe(`${A}-${B}`);
  });

  it("is stable across renders for the same pair (pure — no Date/random)", () => {
    const p = mkPair([mkPool(30)]);
    expect(pairKey(p)).toBe(pairKey(p));
  });

  it("two distinct pairs get distinct keys — identity is the pair, not a symbol", () => {
    const other = mkPair([], B, A); // legs swapped ⇒ a DIFFERENT canonical pair key
    expect(pairKey(mkPair([]))).not.toBe(pairKey(other));
  });
});

describe("feeList — fee-tier presentation (payload values, no math)", () => {
  it("dedupes fee_bps across pools and joins ascending with ' · '", () => {
    const p = mkPair([mkPool(30), mkPool(5), mkPool(30, "uniswap_v3"), mkPool(100)]);
    expect(feeList(p)).toBe("5 · 30 · 100");
  });

  it("single tier renders with no separator", () => {
    expect(feeList(mkPair([mkPool(30), mkPool(30, "curve")]))).toBe("30");
  });

  it("zero pools (or all-missing) renders the honest dash, never a 0 (R8)", () => {
    expect(feeList(mkPair([]))).toBe("—");
  });

  it("fractional on-chain fees pass through verbatim (doctrina: never hardcoded)", () => {
    expect(feeList(mkPair([mkPool(0.5), mkPool(1)]))).toBe("0.5 · 1");
  });
});
