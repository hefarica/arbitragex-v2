import { describe, it, expect } from "vitest";
import {
  applyPriceEvent,
  EMPTY_PRICES_STATE,
  type PricesState,
} from "./usePricesStream";

const baseState: PricesState = { ...EMPTY_PRICES_STATE };

describe("applyPriceEvent (G-PRICE-1 pure transitions)", () => {
  it("applies a valid full-map frame and rotates prevPrices", () => {
    const s1 = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { WETH: 2500, USDC: 1 },
      count: 2,
      ttl_secs: 60,
      ts: "2026-08-19T17:00:00Z",
      seq: 1,
    });
    expect(s1.prices).toEqual({ WETH: 2500, USDC: 1 });
    expect(s1.prevPrices).toEqual({});
    expect(s1.seq).toBe(1);
    expect(s1.ttlSecs).toBe(60);

    const s2 = applyPriceEvent(s1, {
      chain_id: 1,
      prices: { WETH: 2600, USDC: 1 },
      count: 2,
      ttl_secs: 58,
      ts: "2026-08-19T17:00:30Z",
      seq: 2,
    });
    // Full-map replace + previous map preserved for direction tracking.
    expect(s2.prices.WETH).toBe(2600);
    expect(s2.prevPrices.WETH).toBe(2500);
    expect(s2.seq).toBe(2);
  });

  it("uppercases symbols and drops non-finite/non-positive prices (R8)", () => {
    const s = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { weth: 2500, bad_nan: NaN, bad_neg: -1, bad_zero: 0, bad_inf: Infinity },
      count: 5,
      ttl_secs: 60,
      ts: "t",
      seq: 1,
    });
    expect(Object.keys(s.prices)).toEqual(["WETH"]);
  });

  it("rejects non-object payloads and returns the previous state", () => {
    expect(applyPriceEvent(baseState, null)).toBe(baseState);
    expect(applyPriceEvent(baseState, "junk")).toBe(baseState);
    expect(applyPriceEvent(baseState, 42)).toBe(baseState);
  });

  it("rejects frames without a numeric chain_id / prices object", () => {
    expect(applyPriceEvent(baseState, { prices: { A: 1 } })).toBe(baseState);
    expect(applyPriceEvent(baseState, { chain_id: 1, prices: null })).toBe(baseState);
    expect(applyPriceEvent(baseState, { chain_id: "1", prices: {} })).toBe(baseState);
  });

  it("falls back to previous ts/seq when the frame omits them", () => {
    const s1 = applyPriceEvent(baseState, {
      chain_id: 1,
      prices: { A: 1 },
      count: 1,
      ttl_secs: 30,
      ts: "2026-08-19T17:01:00Z",
      seq: 7,
    });
    const s2 = applyPriceEvent(s1, { chain_id: 1, prices: { A: 2 } });
    expect(s2.ts).toBe("2026-08-19T17:01:00Z");
    expect(s2.seq).toBe(7);
    expect(s2.ttlSecs).toBeNull();
  });
});
